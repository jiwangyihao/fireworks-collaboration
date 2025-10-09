use std::collections::HashSet;

use serde::Serialize;
use tauri::State;

use crate::app::types::{ConfigBaseDir, SharedConfig, SharedIpPool};
use crate::core::config::loader as cfg_loader;
use crate::core::ip_pool::config as ip_pool_cfg;
use crate::core::ip_pool::{
    self,
    config::{IpPoolFileConfig, IpPoolRuntimeConfig},
    manager::{IpPool, IpSelectionStrategy, OutcomeMetrics},
    preheat, IpStat,
};

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct OutcomeMetricsDto {
    pub success: u32,
    pub failure: u32,
    pub last_outcome_ms: Option<i64>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct IpPoolCacheEntryDto {
    pub host: String,
    pub port: u16,
    pub best: Option<IpStat>,
    pub alternatives: Vec<IpStat>,
    pub outcome: Option<OutcomeMetricsDto>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct IpPoolSnapshotDto {
    pub runtime: IpPoolRuntimeConfig,
    pub file: IpPoolFileConfig,
    pub enabled: bool,
    pub preheat_enabled: bool,
    pub preheater_active: bool,
    pub preheat_targets: usize,
    pub preheated_targets: usize,
    pub auto_disabled_until: Option<i64>,
    pub cache_entries: Vec<IpPoolCacheEntryDto>,
    pub tripped_ips: Vec<String>,
    pub timestamp_ms: i64,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct IpPoolPreheatActivationDto {
    pub enabled: bool,
    pub preheat_enabled: bool,
    pub preheater_active: bool,
    pub activation_changed: bool,
    pub preheat_targets: usize,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct IpSelectionDto {
    pub host: String,
    pub port: u16,
    pub strategy: String,
    pub cache_hit: bool,
    pub selected: Option<IpStat>,
    pub alternatives: Vec<IpStat>,
    pub outcome: Option<OutcomeMetricsDto>,
}

#[tauri::command]
pub async fn ip_pool_get_snapshot(
    pool: State<'_, SharedIpPool>,
) -> Result<IpPoolSnapshotDto, String> {
    let pool_arc = pool.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let guard = pool_arc.lock().map_err(|e| e.to_string())?;
        Ok(build_snapshot(&*guard))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn ip_pool_update_config(
    runtime: IpPoolRuntimeConfig,
    file: IpPoolFileConfig,
    cfg: State<'_, SharedConfig>,
    base: State<'_, ConfigBaseDir>,
    pool: State<'_, SharedIpPool>,
) -> Result<IpPoolSnapshotDto, String> {
    let runtime_clone_for_pool = runtime.clone();
    let file_clone_for_pool = file.clone();

    let cfg_snapshot = {
        let mut guard = cfg.lock().map_err(|e| e.to_string())?;
        guard.ip_pool = runtime;
        guard.clone()
    };

    cfg_loader::save_at(&cfg_snapshot, base.as_path()).map_err(|e| e.to_string())?;
    ip_pool_cfg::save_file_at(&file, base.as_path()).map_err(|e| e.to_string())?;

    let pool_arc = pool.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let mut guard = pool_arc.lock().map_err(|e| e.to_string())?;
        let effective =
            ip_pool::EffectiveIpPoolConfig::from_parts(runtime_clone_for_pool, file_clone_for_pool);
        guard.update_config(effective);
        Ok(build_snapshot(&*guard))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn ip_pool_request_refresh(pool: State<'_, SharedIpPool>) -> Result<bool, String> {
    let pool_arc = pool.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let guard = pool_arc.lock().map_err(|e| e.to_string())?;
        Ok(guard.request_preheat_refresh())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn ip_pool_start_preheater(
    pool: State<'_, SharedIpPool>,
) -> Result<IpPoolPreheatActivationDto, String> {
    let pool_arc = pool.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let mut guard = pool_arc.lock().map_err(|e| e.to_string())?;
        let enabled = guard.is_enabled();
        let preheat_targets = guard.preheat_target_count();
        let (activation_changed, _) = guard.enable_preheater();
        let preheater_active = guard.has_preheater();
        Ok(IpPoolPreheatActivationDto {
            enabled,
            preheat_enabled: guard.preheat_enabled(),
            preheater_active,
            activation_changed,
            preheat_targets,
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn ip_pool_clear_auto_disabled(pool: State<'_, SharedIpPool>) -> Result<bool, String> {
    let pool_arc = pool.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let guard = pool_arc.lock().map_err(|e| e.to_string())?;
        Ok(guard.clear_auto_disabled())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn ip_pool_pick_best(
    host: String,
    port: u16,
    pool: State<'_, SharedIpPool>,
) -> Result<IpSelectionDto, String> {
    let pool_arc = pool.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let mut guard = pool_arc.lock().map_err(|e| e.to_string())?;
        let selection = guard.pick_best_blocking(&host, port);
        let outcome = metrics_to_dto(guard.outcome_metrics(&host, port));
        let strategy = selection.strategy();
        let selected = selection.selected().cloned();
        let alternatives = selection.alternatives().to_vec();
        let cache_hit = matches!(strategy, IpSelectionStrategy::Cached);
        Ok(IpSelectionDto {
            host,
            port,
            strategy: strategy_to_string(strategy),
            cache_hit,
            selected,
            alternatives,
            outcome,
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

fn build_snapshot(pool: &IpPool) -> IpPoolSnapshotDto {
    let now_ms = preheat::current_epoch_ms();
    let cache_snapshot = pool.cache().snapshot();

    let mut entries: Vec<IpPoolCacheEntryDto> = cache_snapshot
        .iter()
        .map(|(key, slot)| {
            let outcome = metrics_to_dto(pool.outcome_metrics(&key.host, key.port));
            IpPoolCacheEntryDto {
                host: key.host.clone(),
                port: key.port,
                best: slot.best.clone(),
                alternatives: slot.alternatives.clone(),
                outcome,
            }
        })
        .collect();

    entries.sort_by(|a, b| a.host.cmp(&b.host).then(a.port.cmp(&b.port)));

    let mut tripped_ips: Vec<String> = pool
        .get_tripped_ips()
        .into_iter()
        .map(|ip| ip.to_string())
        .collect();
    tripped_ips.sort();

    let mut target_pairs: HashSet<(String, u16)> = HashSet::new();
    for domain in preheat::resolve_preheat_domains(pool.file_config()) {
        let host = domain.host.to_ascii_lowercase();
        let ports = if domain.ports.is_empty() {
            vec![443]
        } else {
            domain.ports.clone()
        };
        for port in ports {
            target_pairs.insert((host.clone(), port));
        }
    }

    let mut warmed_pairs: HashSet<(String, u16)> = HashSet::new();
    for (key, slot) in &cache_snapshot {
        if slot.best.is_some() || !slot.alternatives.is_empty() {
            warmed_pairs.insert((key.host.to_ascii_lowercase(), key.port));
        }
    }

    let preheat_targets = target_pairs.len();
    let preheated_targets = target_pairs
        .iter()
        .filter(|key| warmed_pairs.contains(*key))
        .count();

    IpPoolSnapshotDto {
        runtime: pool.runtime_config().clone(),
        file: pool.file_config().clone(),
        enabled: pool.is_enabled(),
        preheat_enabled: pool.preheat_enabled(),
        preheater_active: pool.has_preheater(),
        preheat_targets,
        preheated_targets,
        auto_disabled_until: pool.auto_disabled_until(),
        cache_entries: entries,
        tripped_ips,
        timestamp_ms: now_ms,
    }
}

fn strategy_to_string(strategy: IpSelectionStrategy) -> String {
    match strategy {
        IpSelectionStrategy::SystemDefault => "system".to_string(),
        IpSelectionStrategy::Cached => "cached".to_string(),
    }
}

fn metrics_to_dto(metrics: Option<OutcomeMetrics>) -> Option<OutcomeMetricsDto> {
    metrics.map(|m| OutcomeMetricsDto {
        success: m.success,
        failure: m.failure,
        last_outcome_ms: if m.last_outcome_ms > 0 {
            Some(m.last_outcome_ms)
        } else {
            None
        },
    })
}
