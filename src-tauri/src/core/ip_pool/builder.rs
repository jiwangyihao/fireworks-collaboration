use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Result;

use crate::core::config::model::AppConfig;

use super::{
    config::{self, EffectiveIpPoolConfig},
    history::IpHistoryStore,
};

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

pub(super) fn init_history_store(config: &EffectiveIpPoolConfig) -> Arc<IpHistoryStore> {
    if let Some(raw_path) = config
        .runtime
        .history_path
        .as_deref()
        .map(str::trim)
        .filter(|path| !path.is_empty())
    {
        let resolved = resolve_history_path(raw_path);
        match IpHistoryStore::load_or_init_from_file(&resolved) {
            Ok(store) => Arc::new(store),
            Err(err) => {
                tracing::warn!(
                    target = "ip_pool",
                    path = %resolved.display(),
                    error = %err,
                    "failed to load custom ip history; using in-memory store"
                );
                Arc::new(IpHistoryStore::in_memory())
            }
        }
    } else {
        IpHistoryStore::load_default()
            .map(Arc::new)
            .unwrap_or_else(|err| {
                tracing::warn!(
                    target = "ip_pool",
                    error = %err,
                    "failed to load ip history; using in-memory store"
                );
                Arc::new(IpHistoryStore::in_memory())
            })
    }
}

fn resolve_history_path(path: &str) -> PathBuf {
    let candidate = PathBuf::from(path);
    if candidate.is_absolute() {
        candidate
    } else {
        let mut base = crate::core::config::loader::base_dir();
        base.push(path);
        base
    }
}
