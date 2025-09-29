use std::sync::atomic::Ordering;

use super::manager::IpPool;

pub(super) fn maybe_prune_cache(pool: &IpPool, now_ms: i64) {
    let interval_ms = (pool.config.runtime.cache_prune_interval_secs.max(5) as i64) * 1000;
    let last = pool.last_prune_at_ms.load(Ordering::Relaxed);
    if now_ms.saturating_sub(last) < interval_ms {
        return;
    }
    if pool
        .last_prune_at_ms
        .compare_exchange(last, now_ms, Ordering::SeqCst, Ordering::Relaxed)
        .is_ok()
    {
        prune_cache(pool, now_ms);
    }
}

pub(super) fn enforce_cache_capacity(pool: &IpPool) {
    let max_entries = pool.config.runtime.max_cache_entries;
    if max_entries == 0 {
        return;
    }
    let snapshot = pool.cache.snapshot();
    let mut entries: Vec<_> = snapshot
        .into_iter()
        .filter(|(key, _)| !pool.is_preheat_target(&key.host, key.port))
        .collect();
    if entries.len() <= max_entries {
        return;
    }
    entries.sort_by_key(|(_, slot)| {
        slot.best
            .as_ref()
            .and_then(|stat| stat.measured_at_epoch_ms)
            .unwrap_or(0)
    });
    let overflow = entries.len() - max_entries;
    for (key, _) in entries.into_iter().take(overflow) {
        expire_entry(pool, &key.host, key.port);
    }
}

pub(super) fn expire_entry(pool: &IpPool, host: &str, port: u16) {
    pool.cache.remove(host, port);
    if pool.is_preheat_target(host, port) {
        return;
    }
    if let Err(err) = pool.history.remove(host, port) {
        tracing::warn!(
            target = "ip_pool",
            host,
            port,
            error = %err,
            "failed to remove expired entry from history"
        );
    }
}

fn prune_cache(pool: &IpPool, now_ms: i64) {
    let snapshot = pool.cache.snapshot();
    for (key, slot) in &snapshot {
        if pool.is_preheat_target(&key.host, key.port) {
            continue;
        }
        let expired = slot
            .best
            .as_ref()
            .map(|stat| stat.is_expired(now_ms))
            .unwrap_or(true);
        if expired {
            expire_entry(pool, &key.host, key.port);
        }
    }
    enforce_cache_capacity(pool);
}
