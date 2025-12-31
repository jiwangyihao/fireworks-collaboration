use fireworks_collaboration_lib::core::ip_pool::{
    EffectiveIpPoolConfig, IpCacheKey, IpCacheSlot, IpCandidate, IpOutcome, IpPool, IpSource,
    IpStat,
};
use std::net::IpAddr;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn create_test_pool() -> IpPool {
    let mut config = EffectiveIpPoolConfig::default();
    config.runtime.enabled = true;
    config.runtime.circuit_breaker_enabled = true;
    // Set low thresholds for circuit breaker testing
    config.runtime.failure_threshold = 2; // Trip after 2 failures
    config.runtime.failure_window_seconds = 60;
    config.runtime.cooldown_seconds = 60;
    IpPool::new(config)
}

fn current_ts_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
}

fn create_dummy_stat(ip_str: &str, latency: u32, expires_at: Option<i64>) -> IpStat {
    let addr: IpAddr = ip_str.parse().unwrap();
    let candidate = IpCandidate::new(addr, 443, IpSource::Dns);
    let mut stat = IpStat::with_latency(candidate, latency);
    stat.expires_at_epoch_ms = expires_at;
    stat
}

#[test]
fn test_ip_pool_auto_disable() {
    let pool = create_test_pool();
    assert!(pool.is_enabled(), "Pool should be enabled initially");

    // Disable for 100ms
    pool.set_auto_disabled("test reason", 100);
    assert!(
        !pool.is_enabled(),
        "Pool should be disabled after set_auto_disabled"
    );

    // Wait for auto-enable
    thread::sleep(Duration::from_millis(150));
    assert!(
        pool.is_enabled(),
        "Pool should auto-enable after cooldown expires"
    );
}

#[test]
fn test_circuit_breaker_integration() {
    let pool = create_test_pool();
    let ip_str = "192.0.2.1"; // TEST-NET-1
    let addr: IpAddr = ip_str.parse().unwrap();

    // 1. Initially not tripped
    assert!(!pool.is_ip_tripped(addr));

    let candidate_stat = create_dummy_stat(ip_str, 100, None);

    // 2. Report failures to reach threshold (2)
    pool.report_candidate_outcome("example.com", 443, &candidate_stat, IpOutcome::Failure);
    assert!(
        !pool.is_ip_tripped(addr),
        "Should not trip after 1 failure (threshold 2)"
    );

    pool.report_candidate_outcome("example.com", 443, &candidate_stat, IpOutcome::Failure);
    assert!(
        pool.is_ip_tripped(addr),
        "Should trip after 2 failures (threshold 2)"
    );

    // 3. Verify get_tripped_ips
    let tripped = pool.get_tripped_ips();
    assert!(tripped.contains(&addr));

    // 4. Manual reset
    pool.reset_circuit_breaker(addr);
    assert!(!pool.is_ip_tripped(addr));
}

#[test]
fn test_cache_pruning() {
    let pool = create_test_pool();
    let cache = pool.cache();
    let host = "expire.test";
    let port = 443;
    let key = IpCacheKey::new(host, port);

    // 1. Insert an entry that expires in the past
    let now = current_ts_ms();
    let expired_stat = create_dummy_stat("192.0.2.10", 50, Some(now - 1000));
    let slot = IpCacheSlot::with_best(expired_stat);
    cache.insert(key.clone(), slot);

    assert!(cache.get(host, port).is_some());

    // 2. Trigger maintenance tick using current time (which is > expired time)
    pool.maintenance_tick();

    // 3. Verify entry is removed
    assert!(
        cache.get(host, port).is_none(),
        "Expired entry should be pruned"
    );

    // 4. Insert an entry that expires in the future
    let future_stat = create_dummy_stat("192.0.2.11", 50, Some(now + 10000));
    let valid_slot = IpCacheSlot::with_best(future_stat);
    cache.insert(key.clone(), valid_slot);

    pool.maintenance_tick();
    assert!(
        cache.get(host, port).is_some(),
        "Future entry should NOT be pruned"
    );
}

#[tokio::test]
async fn test_sampling_expiration() {
    let pool = create_test_pool();
    let cache = pool.cache();
    let host = "sampling.test";
    let port = 443;
    let key = IpCacheKey::new(host, port);

    // 1. Insert logic that marks entry as expired
    let now = current_ts_ms();
    // Simulate an entry stored previously with expiration just reached
    let expired_stat = create_dummy_stat("192.0.2.20", 50, Some(now - 1));
    let slot = IpCacheSlot::with_best(expired_stat);
    cache.insert(key, slot);

    // 2. Calling `pick_best` or `get_fresh_cached` should treat it as expired.
    // We using blocking runtime call inside async test is fine for unit test scope
    // But since we are allowed to use async here (thanks to #[tokio::test]), we can use `pick_best` directly!
    // However, `pick_best` calls `measure_candidates` which creates sockets.
    // `preheat::collect_candidates` returns empty candidates for "sampling.test".
    // So `sample_once` returns None or fails harmlessly.

    // We use pick_best directly since we are in async context.
    let selection = pool.pick_best(host, port).await;

    assert!(
        cache.get(host, port).is_none(),
        "Expired entry should be invalided during pick_best sampling flow"
    );
    assert!(selection.is_system_default());
}
