use std::{
    collections::HashMap,
    fmt,
    net::IpAddr,
    sync::{
        atomic::{AtomicI64, Ordering},
        Arc, Mutex, OnceLock,
    },
};

use anyhow::Result;
use tokio::{
    runtime::{Builder as RuntimeBuilder, Handle, Runtime},
    sync::{Mutex as AsyncMutex, Notify},
};

use crate::core::config::model::AppConfig;

#[cfg(test)]
use super::cache::IpCandidate;
use super::{
    builder,
    cache::{IpCacheKey, IpCacheSlot, IpScoreCache, IpSource, IpStat},
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
    best: Option<IpStat>,
    alternatives: Vec<IpStat>,
    strategy: IpSelectionStrategy,
}

impl IpSelection {
    pub fn system_default<S: Into<String>>(host: S, port: u16) -> Self {
        Self {
            key: IpCacheKey::new(host, port),
            best: None,
            alternatives: Vec::new(),
            strategy: IpSelectionStrategy::SystemDefault,
        }
    }

    pub fn from_slot<S: Into<String>>(host: S, port: u16, slot: IpCacheSlot) -> Self {
        Self {
            key: IpCacheKey::new(host, port),
            best: slot.best,
            alternatives: slot.alternatives,
            strategy: IpSelectionStrategy::Cached,
        }
    }

    pub fn from_cached<S: Into<String>>(host: S, port: u16, stat: IpStat) -> Self {
        Self {
            key: IpCacheKey::new(host, port),
            best: Some(stat),
            alternatives: Vec::new(),
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
        self.best.as_ref()
    }

    pub fn into_stat(self) -> Option<IpStat> {
        self.best
    }

    pub fn is_system_default(&self) -> bool {
        matches!(self.strategy, IpSelectionStrategy::SystemDefault)
    }

    pub fn alternatives(&self) -> &[IpStat] {
        &self.alternatives
    }

    pub fn iter_candidates(&self) -> impl Iterator<Item = &IpStat> {
        self.best.iter().chain(self.alternatives.iter())
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
    per_candidate: HashMap<IpAddr, CandidateOutcomeStats>,
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

#[derive(Debug, Default, Clone)]
struct CandidateOutcomeStats {
    success: u32,
    failure: u32,
    last_outcome_ms: i64,
    last_sources: Vec<IpSource>,
}

impl CandidateOutcomeStats {
    fn record(&mut self, outcome: IpOutcome, sources: &[IpSource]) {
        match outcome {
            IpOutcome::Success => self.success = self.success.saturating_add(1),
            IpOutcome::Failure => self.failure = self.failure.saturating_add(1),
        }
        self.last_outcome_ms = preheat::current_epoch_ms();
        if !sources.is_empty() {
            self.last_sources = sources.to_vec();
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

/// 单个候选 IP 的统计视图。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CandidateOutcomeMetrics {
    /// 成功次数。
    pub success: u32,
    /// 失败次数。
    pub failure: u32,
    /// 最近一次结果发生的时间戳（毫秒）。
    pub last_outcome_ms: i64,
    /// 最近一次记录时的来源信息。
    pub last_sources: Vec<IpSource>,
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

        if let Some(slot) = self.get_fresh_cached(host, port, now_ms) {
            return IpSelection::from_slot(host.to_string(), port, slot);
        }

        match sampling::ensure_sampled(self, host, port).await {
            Ok(Some(slot)) => IpSelection::from_slot(host.to_string(), port, slot),
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

    pub fn pick_best_blocking(&self, host: &str, port: u16) -> IpSelection {
        if let Ok(handle) = Handle::try_current() {
            return handle.block_on(self.pick_best(host, port));
        }
        match blocking_runtime() {
            Some(rt) => rt.block_on(self.pick_best(host, port)),
            None => IpSelection::system_default(host.to_string(), port),
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

    /// 上报单个候选 IP 的尝试结果，供后续熔断与调试使用。
    pub fn report_candidate_outcome(
        &self,
        host: &str,
        port: u16,
        candidate: &IpStat,
        outcome: IpOutcome,
    ) {
        if candidate.sources.is_empty() {
            tracing::debug!(
                target = "ip_pool",
                host,
                port,
                ip = %candidate.candidate.address,
                "candidate outcome reported without sources"
            );
        }
        let key = IpCacheKey::new(host.to_string(), port);
        match self.outcomes.lock() {
            Ok(mut guard) => {
                let entry = guard.entry(key).or_default();
                let record = entry
                    .per_candidate
                    .entry(candidate.candidate.address)
                    .or_insert_with(CandidateOutcomeStats::default);
                record.record(outcome, &candidate.sources);
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

    /// 返回指定候选 IP 的统计信息。
    pub fn candidate_outcome_metrics(
        &self,
        host: &str,
        port: u16,
        ip: IpAddr,
    ) -> Option<CandidateOutcomeMetrics> {
        let key = IpCacheKey::new(host, port);
        let guard = match self.outcomes.lock() {
            Ok(guard) => guard,
            Err(_) => return None,
        };
        guard
            .get(&key)
            .and_then(|entry| entry.per_candidate.get(&ip))
            .map(|stats| CandidateOutcomeMetrics {
                success: stats.success,
                failure: stats.failure,
                last_outcome_ms: stats.last_outcome_ms,
                last_sources: stats.last_sources.clone(),
            })
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

    fn get_fresh_cached(&self, host: &str, port: u16, now_ms: i64) -> Option<IpCacheSlot> {
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

fn blocking_runtime() -> Option<&'static Runtime> {
    static RUNTIME: OnceLock<std::result::Result<Runtime, String>> = OnceLock::new();
    let entry = RUNTIME.get_or_init(|| {
        RuntimeBuilder::new_multi_thread()
            .worker_threads(2)
            .thread_name("ip-pool-blocking")
            .enable_all()
            .build()
            .map_err(|err| err.to_string())
    });
    match entry {
        Ok(rt) => Some(rt),
        Err(err) => {
            tracing::error!(
                target = "ip_pool",
                error = %err,
                "failed to initialize ip pool blocking runtime"
            );
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr};

    fn make_stat(addr: [u8; 4], port: u16) -> IpStat {
        let candidate = IpCandidate::new(
            IpAddr::from(Ipv4Addr::from(addr)),
            port,
            IpSource::UserStatic,
        );
        let mut stat = IpStat::with_latency(candidate, 12);
        stat.measured_at_epoch_ms = Some(1);
        stat.expires_at_epoch_ms = Some(i64::MAX - 1);
        stat.sources = vec![IpSource::UserStatic];
        stat
    }

    #[test]
    fn report_candidate_outcome_tracks_per_ip() {
        let mut cfg = EffectiveIpPoolConfig::default();
        cfg.runtime.enabled = true;
        let pool = IpPool::new(cfg);
        let stat = make_stat([127, 0, 0, 2], 443);
        pool.report_candidate_outcome("example.com", 443, &stat, IpOutcome::Failure);
        pool.report_candidate_outcome("example.com", 443, &stat, IpOutcome::Success);
        let metrics = pool
            .candidate_outcome_metrics("example.com", 443, stat.candidate.address)
            .expect("candidate metrics present");
        assert_eq!(metrics.success, 1);
        assert_eq!(metrics.failure, 1);
        assert_eq!(metrics.last_sources, vec![IpSource::UserStatic]);
    }

    #[test]
    fn report_outcome_updates_aggregate_counts() {
        let mut cfg = EffectiveIpPoolConfig::default();
        cfg.runtime.enabled = true;
        let pool = IpPool::new(cfg);
        let stat = make_stat([127, 0, 0, 3], 443);
        let selection = IpSelection::from_cached("example.com", 443, stat.clone());
        pool.report_outcome(&selection, IpOutcome::Success);
        let aggregate = pool
            .outcome_metrics("example.com", 443)
            .expect("aggregate metrics present");
        assert_eq!(aggregate.success, 1);
        assert_eq!(aggregate.failure, 0);
        assert!(aggregate.last_outcome_ms > 0);
    }
}
