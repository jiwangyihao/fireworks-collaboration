use std::{sync::Arc, time::Duration};

use anyhow::Result;
use tokio::{sync::Notify, time::timeout};

use super::{
    cache::{IpCacheKey, IpCacheSlot, IpStat},
    maintenance,
    manager::IpPool,
    preheat,
};
use crate::core::ip_pool::events::emit_ip_pool_refresh;
use uuid::Uuid;

pub(super) fn get_fresh_cached(
    pool: &IpPool,
    host: &str,
    port: u16,
    now_ms: i64,
) -> Option<IpCacheSlot> {
    if let Some(mut slot) = pool.cache.get(host, port) {
        // Filter out tripped IPs from alternatives
        slot.alternatives
            .retain(|alt| !alt.is_expired(now_ms) && !pool.is_ip_tripped(alt.candidate.address));

        match slot.best.clone() {
            Some(best) => {
                if best.is_expired(now_ms) {
                    if !pool.is_preheat_target(host, port) {
                        maintenance::expire_entry(pool, host, port);
                    }
                    return None;
                }
                if pool.is_ip_tripped(best.candidate.address) {
                    // Best is tripped, fall back to alternatives or refresh
                    if !slot.alternatives.is_empty() {
                        return Some(IpCacheSlot {
                            best: None,
                            alternatives: slot.alternatives,
                        });
                    }
                    return None;
                }
                // Alternatives are already filtered above
                return Some(IpCacheSlot {
                    best: Some(best),
                    alternatives: slot.alternatives,
                });
            }
            None => {
                if !pool.is_preheat_target(host, port) {
                    maintenance::expire_entry(pool, host, port);
                }
            }
        }
    }
    None
}

pub(super) async fn ensure_sampled(
    pool: &IpPool,
    host: &str,
    port: u16,
) -> Result<Option<IpCacheSlot>> {
    let key = IpCacheKey::new(host.to_string(), port);
    loop {
        if let Some(slot) = get_fresh_cached(pool, host, port, preheat::current_epoch_ms()) {
            return Ok(Some(slot));
        }

        let waiter = {
            let mut guard = pool.pending.lock().await;
            if let Some(existing) = guard.get(&key) {
                Some(existing.clone())
            } else {
                let notify = Arc::new(Notify::new());
                guard.insert(key.clone(), notify.clone());
                None
            }
        };

        if let Some(waiter) = waiter {
            let timeout_ms = pool.config.runtime.singleflight_timeout_ms.max(100);
            if timeout(Duration::from_millis(timeout_ms), waiter.notified())
                .await
                .is_err()
            {
                tracing::warn!(
                    target = "ip_pool",
                    host,
                    port,
                    "waited for single-flight probe but timed out"
                );
                return Ok(None);
            }
            continue;
        }

        let result = sample_once(pool, host, port).await;
        let mut guard = pool.pending.lock().await;
        if let Some(entry) = guard.remove(&key) {
            entry.notify_waiters();
        }
        return result;
    }
}

async fn sample_once(pool: &IpPool, host: &str, port: u16) -> Result<Option<IpCacheSlot>> {
    let config = pool.config.clone();
    let history = pool.history.clone();
    let cache = pool.cache.clone();

    let mut candidates = preheat::collect_candidates(host, port, &config, history.clone()).await;
    // Filter out tripped IPs to prevent re-probing known bad IPs unnecessarily
    candidates.retain(|c| !pool.is_ip_tripped(c.candidate.address));

    if candidates.is_empty() {
        tracing::warn!(
            target = "ip_pool",
            host,
            port,
            "no candidates collected for on-demand sampling (or all tripped)"
        );
        maintenance::expire_entry(pool, host, port);
        // Emit failure refresh event for observability (on-demand path)
        emit_ip_pool_refresh(
            Uuid::new_v4(),
            host,
            false,
            &[],
            "no_candidates".to_string(),
        );
        return Ok(None);
    }

    let stats = preheat::measure_candidates(
        host,
        port,
        candidates,
        &config.runtime,
        config.file.score_ttl_seconds,
        None,
        preheat::default_latency_prober(),
    )
    .await;

    if stats.is_empty() {
        tracing::warn!(
            target = "ip_pool",
            host,
            port,
            "all candidates failed probing"
        );
        maintenance::expire_entry(pool, host, port);
        // Emit failure refresh event for observability (on-demand path)
        emit_ip_pool_refresh(
            Uuid::new_v4(),
            host,
            false,
            &[],
            "all_probes_failed".to_string(),
        );
        return Ok(None);
    }

    // Report success for measured candidates to reset their circuit breaker logic if needed
    for stat in &stats {
        pool.report_candidate_outcome(host, port, stat, super::IpOutcome::Success);
    }

    let best = stats.first().cloned();
    let mut alternatives: Vec<IpStat> = Vec::new();
    if stats.len() > 1 {
        alternatives = stats.iter().skip(1).cloned().collect();
    }
    // Clone for event emission before moving into cache/history
    let stats_for_event = stats.clone();
    preheat::update_cache_and_history(host, port, stats, cache, history)?;
    if best.is_some() {
        emit_ip_pool_refresh(
            Uuid::new_v4(),
            host,
            true,
            &stats_for_event,
            "on_demand".to_string(),
        );
    }
    Ok(best.map(|stat| IpCacheSlot {
        best: Some(stat),
        alternatives,
    }))
}
