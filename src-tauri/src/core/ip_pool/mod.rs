pub mod cache;
pub mod config;
pub mod history;
pub mod preheat;

use crate::core::config::model::AppConfig;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::{path::Path, sync::Arc};

pub use cache::{IpCacheKey, IpCacheSlot, IpCandidate, IpScoreCache, IpStat};
pub use config::{
    EffectiveIpPoolConfig, IpPoolFileConfig, IpPoolRuntimeConfig, IpPoolSourceToggle,
    PreheatDomain, UserStaticIp,
};
pub use history::{IpHistoryRecord, IpHistoryStore};

use self::preheat::PreheatService;

/// IP 候选来源分类，贯穿配置、缓存与事件输出。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum IpSource {
    Builtin,
    Dns,
    History,
    UserStatic,
    Fallback,
}

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

/// IP 池管理器：负责维持配置、缓存与后续阶段的评分逻辑。
#[derive(Debug)]
pub struct IpPool {
    config: Arc<EffectiveIpPoolConfig>,
    cache: Arc<IpScoreCache>,
    history: Arc<IpHistoryStore>,
    preheater: Option<PreheatService>,
}

impl IpPool {
    pub fn new(config: EffectiveIpPoolConfig) -> Self {
        let history = IpHistoryStore::load_default()
            .map(Arc::new)
            .unwrap_or_else(|err| {
                tracing::warn!(
                    target = "ip_pool",
                    error = %err,
                    "failed to load ip history; using in-memory store"
                );
                Arc::new(IpHistoryStore::in_memory())
            });
        let mut pool = Self {
            config: Arc::new(config),
            cache: Arc::new(IpScoreCache::new()),
            history,
            preheater: None,
        };
        pool.rebuild_preheater();
        pool
    }

    pub fn from_app_config(app_cfg: &AppConfig) -> Result<Self> {
        let effective = load_effective_config(app_cfg)?;
        Ok(Self::new(effective))
    }

    pub fn with_cache(config: EffectiveIpPoolConfig, cache: IpScoreCache) -> Self {
        let mut pool = Self {
            config: Arc::new(config),
            cache: Arc::new(cache),
            history: Arc::new(IpHistoryStore::in_memory()),
            preheater: None,
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
        self.config = Arc::new(config);
        self.rebuild_preheater();
    }

    pub fn pick_best(&self, host: &str, port: u16) -> IpSelection {
        if !self.is_enabled() {
            return IpSelection::system_default(host.to_string(), port);
        }

        if let Some(slot) = self.cache.get(host, port) {
            if let Some(best) = slot.best {
                return IpSelection::from_cached(host.to_string(), port, best);
            }
        }

        IpSelection::system_default(host.to_string(), port)
    }

    pub fn report_outcome(&self, selection: &IpSelection, outcome: IpOutcome) {
        let strategy = match selection.strategy {
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
        // P4.0 阶段仅记录日志，真实反馈逻辑将在后续阶段引入。
    }

    fn rebuild_preheater(&mut self) {
        // Drop any existing preheater before creating a new one so background threads exit.
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

}

impl Default for IpPool {
    fn default() -> Self {
        Self::new(EffectiveIpPoolConfig::default())
    }
}

pub fn load_effective_config(app_cfg: &AppConfig) -> Result<EffectiveIpPoolConfig> {
    let runtime = app_cfg.ip_pool.clone();
    let file = config::load_or_init_file()?;
    Ok(EffectiveIpPoolConfig::from_parts(runtime, file))
}

pub fn load_effective_config_at(
    app_cfg: &AppConfig,
    base_dir: &Path,
) -> Result<EffectiveIpPoolConfig> {
    let runtime = app_cfg.ip_pool.clone();
    let file = config::load_or_init_file_at(base_dir)?;
    Ok(EffectiveIpPoolConfig::from_parts(runtime, file))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{
        config::loader as cfg_loader,
        ip_pool::config::{save_file_at, IpPoolFileConfig},
    };
    use std::{
        fs,
        net::{IpAddr, Ipv4Addr},
    };
    use uuid::Uuid;

    #[test]
    fn pick_best_falls_back_when_disabled() {
        let pool = IpPool::default();
        let selection = pool.pick_best("github.com", 443);
        assert!(selection.is_system_default());
        assert!(selection.selected().is_none());
    }

    #[test]
    fn pick_best_uses_cache_when_enabled() {
        let mut cfg = EffectiveIpPoolConfig::default();
        cfg.runtime.enabled = true;
        let cache = IpScoreCache::new();
        cache.insert(
            IpCacheKey::new("github.com", 443),
            IpCacheSlot::with_best(IpStat::with_latency(
                IpCandidate::new(
                    IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1)),
                    443,
                    IpSource::Builtin,
                ),
                15,
            )),
        );
        let pool = IpPool::with_cache(cfg, cache);
        let selection = pool.pick_best("github.com", 443);
        assert!(!selection.is_system_default());
        assert_eq!(
            selection.selected().unwrap().candidate.address,
            IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1))
        );
    }

    #[test]
    fn update_config_applies_runtime_changes() {
        let mut pool = IpPool::default();
        pool.cache().insert(
            IpCacheKey::new("github.com", 443),
            IpCacheSlot::with_best(IpStat::with_latency(
                IpCandidate::new(
                    IpAddr::V4(Ipv4Addr::new(9, 9, 9, 9)),
                    443,
                    IpSource::Builtin,
                ),
                7,
            )),
        );
        let selection = pool.pick_best("github.com", 443);
        assert!(selection.is_system_default());
        let mut new_config = EffectiveIpPoolConfig::default();
        new_config.runtime.enabled = true;
        pool.update_config(new_config);
        let updated = pool.pick_best("github.com", 443);
        assert!(!updated.is_system_default());
        assert_eq!(
            updated.selected().map(|stat| stat.candidate.address),
            Some(IpAddr::V4(Ipv4Addr::new(9, 9, 9, 9))),
        );
    }

    #[test]
    fn load_effective_config_with_custom_base_dir() {
        let base = std::env::temp_dir().join(format!("fwc-ip-pool-effective-{}", Uuid::new_v4()));
        fs::create_dir_all(&base).unwrap();
        let mut file_cfg = IpPoolFileConfig::default();
        file_cfg
            .preheat_domains
            .push(PreheatDomain::new("github.com"));
        file_cfg.score_ttl_seconds = 600;
        save_file_at(&file_cfg, &base).expect("save ip-config.json");
        let mut app_cfg = AppConfig::default();
        app_cfg.ip_pool.enabled = true;
        app_cfg.ip_pool.max_parallel_probes = 16;
        let effective = load_effective_config_at(&app_cfg, &base).expect("load effective config");
        assert!(effective.runtime.enabled);
        assert_eq!(effective.runtime.max_parallel_probes, 16);
        assert_eq!(effective.file.preheat_domains.len(), 1);
        assert_eq!(effective.file.score_ttl_seconds, 600);
        fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn from_app_config_uses_runtime_defaults() {
        let temp_dir =
            std::env::temp_dir().join(format!("fwc-ip-pool-from-app-{}", Uuid::new_v4()));
        fs::create_dir_all(&temp_dir).unwrap();
        cfg_loader::test_override_global_base_dir(&temp_dir);
        let mut app_cfg = AppConfig::default();
        app_cfg.ip_pool.enabled = true;
        let pool = IpPool::from_app_config(&app_cfg).expect("ip pool from app config");
        assert!(pool.is_enabled());
        assert!(pool.file_config().preheat_domains.is_empty());
        fs::remove_dir_all(&temp_dir).ok();
        cfg_loader::test_clear_global_base_dir();
    }

    #[test]
    fn load_effective_config_at_errors_on_invalid_file() {
        let base = std::env::temp_dir().join(format!("fwc-ip-pool-invalid-{}", Uuid::new_v4()));
        let config_dir = base.join("config");
        fs::create_dir_all(&config_dir).unwrap();
        let config_path = config_dir.join("ip-config.json");
        fs::write(&config_path, b"not-json").unwrap();
        let app_cfg = AppConfig::default();
        let result = load_effective_config_at(&app_cfg, &base);
        assert!(result.is_err());
        fs::remove_dir_all(&base).ok();
    }
}
