use std::{
    collections::HashMap,
    fmt,
    sync::{
        atomic::{AtomicI64, Ordering},
        Arc, Mutex,
    },
};

use anyhow::Result;
use tokio::sync::{Mutex as AsyncMutex, Notify};

use crate::core::config::model::AppConfig;

use super::{
    builder,
    cache::{IpCacheKey, IpScoreCache, IpStat},
    config::{EffectiveIpPoolConfig, IpPoolFileConfig, IpPoolRuntimeConfig},
    history::IpHistoryStore,
    maintenance,
    preheat::{self, PreheatService},
    sampling,
};

/// IP 选择策略：使用系统 DNS 或缓存的评分结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IpSelectionStrategy {
    SystemDefault,
    Cached,
}

/// IP 选择结果，封装域名、端口与可选评分条目。
#[derive(Debug, Clone)]
pub struct IpSelection {
    key: IpCacheKey,
    chosen: Option<IpStat>,
    strategy: IpSelectionStrategy,
}

impl IpSelection {
    pub fn system_default<S: Into<String>>(host: S, port: u16) -> Self {
        Self {
            key: IpCacheKey::new(host, port),
            chosen: None,
            strategy: IpSelectionStrategy::SystemDefault,
        }
    }

    pub fn from_cached<S: Into<String>>(host: S, port: u16, stat: IpStat) -> Self {
        Self {
            key: IpCacheKey::new(host, port),
            chosen: Some(stat),
            strategy: IpSelectionStrategy::Cached,
        }
    }

    pub fn host(&self) -> &str {
        &self.key.host
    }

    pub fn port(&self) -> u16 {
        self.key.port
    }

    pub fn strategy(&self) -> IpSelectionStrategy {
        self.strategy
    }

    pub fn selected(&self) -> Option<&IpStat> {
        self.chosen.as_ref()
    }

    pub fn into_stat(self) -> Option<IpStat> {
        self.chosen
    }

    pub fn is_system_default(&self) -> bool {
        matches!(self.strategy, IpSelectionStrategy::SystemDefault)
    }
}

/// IP 连接结果，用于反馈缓存。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IpOutcome {
    Success,
    Failure,
}

#[derive(Debug, Default, Clone)]
struct OutcomeStats {
    success: u32,
    failure: u32,
    last_outcome_ms: i64,
}

impl OutcomeStats {
    fn as_metrics(&self) -> OutcomeMetrics {
        OutcomeMetrics {
            success: self.success,
            failure: self.failure,
            last_outcome_ms: self.last_outcome_ms,
        }
    }
}

/// 用于测试与可观测性场景的只读统计数据。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct OutcomeMetrics {
    /// 成功次数。
    pub success: u32,
    /// 失败次数。
    pub failure: u32,
    /// 最近一次结果发生的时间戳（毫秒）。
    pub last_outcome_ms: i64,
}

/// IP 池管理器：负责维持配置、缓存与后续阶段的评分逻辑。
pub struct IpPool {
    pub(super) config: Arc<EffectiveIpPoolConfig>,
    pub(super) cache: Arc<IpScoreCache>,
    pub(super) history: Arc<IpHistoryStore>,
    pub(super) preheater: Option<PreheatService>,
    pub(super) pending: Arc<AsyncMutex<HashMap<IpCacheKey, Arc<Notify>>>>,
    pub(super) last_prune_at_ms: AtomicI64,
    outcomes: Arc<Mutex<HashMap<IpCacheKey, OutcomeStats>>>,
}

impl fmt::Debug for IpPool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let enabled = self.is_enabled();
        let cache_len = self.cache.snapshot().len();
        f.debug_struct("IpPool")
            .field("enabled", &enabled)
            .field("cache_entries", &cache_len)
            .finish()
    }
}

impl IpPool {
    pub fn new(config: EffectiveIpPoolConfig) -> Self {
        let history = builder::init_history_store(&config);
        let mut pool = Self {
            config: Arc::new(config),
            cache: Arc::new(IpScoreCache::new()),
            history,
            preheater: None,
            pending: Arc::new(AsyncMutex::new(HashMap::new())),
            last_prune_at_ms: AtomicI64::new(0),
            outcomes: Arc::new(Mutex::new(HashMap::new())),
        };
        pool.rebuild_preheater();
        pool
    }

    pub fn from_app_config(app_cfg: &AppConfig) -> Result<Self> {
        let effective = builder::load_effective_config(app_cfg)?;
        Ok(Self::new(effective))
    }

    pub fn with_cache(config: EffectiveIpPoolConfig, cache: IpScoreCache) -> Self {
        let mut pool = Self {
            config: Arc::new(config),
            cache: Arc::new(cache),
            history: Arc::new(IpHistoryStore::in_memory()),
            preheater: None,
            pending: Arc::new(AsyncMutex::new(HashMap::new())),
            last_prune_at_ms: AtomicI64::new(0),
            outcomes: Arc::new(Mutex::new(HashMap::new())),
        };
        pool.rebuild_preheater();
        pool
    }

    pub fn config(&self) -> &EffectiveIpPoolConfig {
        &self.config
    }

    pub fn runtime_config(&self) -> &IpPoolRuntimeConfig {
        &self.config.runtime
    }

    pub fn file_config(&self) -> &IpPoolFileConfig {
        &self.config.file
    }

    pub fn cache(&self) -> &IpScoreCache {
        &self.cache
    }

    pub fn history(&self) -> &IpHistoryStore {
        &self.history
    }

    pub fn is_enabled(&self) -> bool {
        self.config.runtime.enabled
    }

    pub fn update_config(&mut self, config: EffectiveIpPoolConfig) {
        let history_path_changed = self.config.runtime.history_path != config.runtime.history_path;
        self.config = Arc::new(config);
        if history_path_changed {
            self.history = builder::init_history_store(&self.config);
        }
        self.last_prune_at_ms.store(0, Ordering::Relaxed);
        if let Ok(mut guard) = self.pending.try_lock() {
            guard.clear();
        }
        self.rebuild_preheater();
    }

    pub async fn pick_best(&self, host: &str, port: u16) -> IpSelection {
        if !self.is_enabled() {
            return IpSelection::system_default(host.to_string(), port);
        }

        let now_ms = preheat::current_epoch_ms();
        self.maybe_prune_cache(now_ms);

        if let Some(stat) = self.get_fresh_cached(host, port, now_ms) {
            return IpSelection::from_cached(host.to_string(), port, stat);
        }

        match sampling::ensure_sampled(self, host, port).await {
            Ok(Some(stat)) => IpSelection::from_cached(host.to_string(), port, stat),
            Ok(None) => IpSelection::system_default(host.to_string(), port),
            Err(err) => {
                tracing::warn!(
                    target = "ip_pool",
                    host,
                    port,
                    error = %err,
                    "on-demand sampling failed; using system DNS"
                );
                IpSelection::system_default(host.to_string(), port)
            }
        }
    }

    pub fn report_outcome(&self, selection: &IpSelection, outcome: IpOutcome) {
        let strategy = match selection.strategy() {
            IpSelectionStrategy::SystemDefault => "system",
            IpSelectionStrategy::Cached => "cached",
        };
        tracing::debug!(
            target = "ip_pool",
            host = selection.host(),
            port = selection.port(),
            strategy,
            outcome = ?outcome,
            "ip pool outcome recorded"
        );
        if selection.selected().is_none() {
            return;
        }

        let key = IpCacheKey::new(selection.host().to_string(), selection.port());
        match self.outcomes.lock() {
            Ok(mut guard) => {
                let entry = guard.entry(key).or_default();
                match outcome {
                    IpOutcome::Success => entry.success = entry.success.saturating_add(1),
                    IpOutcome::Failure => entry.failure = entry.failure.saturating_add(1),
                }
                entry.last_outcome_ms = preheat::current_epoch_ms();
            }
            Err(_) => {
                tracing::warn!(target = "ip_pool", "ip pool outcome stats mutex poisoned");
            }
        }
    }

    /// 返回指定 host:port 的只读 outcome 统计信息。
    pub fn outcome_metrics(&self, host: &str, port: u16) -> Option<OutcomeMetrics> {
        let key = IpCacheKey::new(host, port);
        self.outcomes
            .lock()
            .ok()
            .and_then(|guard| guard.get(&key).map(OutcomeStats::as_metrics))
    }

    fn rebuild_preheater(&mut self) {
        self.preheater = None;
        if !self.is_enabled() {
            return;
        }
        if self.config.file.preheat_domains.is_empty() {
            return;
        }
        match PreheatService::spawn(
            self.config.clone(),
            self.cache.clone(),
            self.history.clone(),
        ) {
            Ok(service) => {
                service.request_refresh();
                self.preheater = Some(service);
            }
            Err(err) => {
                tracing::error!(
                    target = "ip_pool",
                    error = %err,
                    "failed to spawn ip pool preheat service"
                );
            }
        }
    }

    fn get_fresh_cached(&self, host: &str, port: u16, now_ms: i64) -> Option<IpStat> {
        sampling::get_fresh_cached(self, host, port, now_ms)
    }

    pub(crate) fn maybe_prune_cache(&self, now_ms: i64) {
        maintenance::maybe_prune_cache(self, now_ms);
    }

    pub(crate) fn enforce_cache_capacity(&self) {
        maintenance::enforce_cache_capacity(self);
    }

    /// 触发一次维护周期，使用当前时间判断过期与容量限制。
    pub fn maintenance_tick(&self) {
        let now_ms = preheat::current_epoch_ms();
        self.maintenance_tick_at(now_ms);
    }

    /// 触发一次维护周期，并允许指定时间戳，便于测试。
    pub fn maintenance_tick_at(&self, now_ms: i64) {
        self.maybe_prune_cache(now_ms);
    }

    fn expire_entry(&self, host: &str, port: u16) {
        maintenance::expire_entry(self, host, port);
    }

    pub(super) fn is_preheat_target(&self, host: &str, port: u16) -> bool {
        self.config
            .file
            .preheat_domains
            .iter()
            .any(|domain| domain.host.eq_ignore_ascii_case(host) && domain.ports.contains(&port))
    }
}

impl Default for IpPool {
    fn default() -> Self {
        Self::new(EffectiveIpPoolConfig::default())
    }
}
