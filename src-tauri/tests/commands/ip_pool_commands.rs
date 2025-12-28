//! IP Pool Commands Integration Tests
//!
//! Tests the IP pool core logic that commands wrap.

use std::sync::{Arc, Mutex};

use fireworks_collaboration_lib::core::config::loader;
use fireworks_collaboration_lib::core::ip_pool::{
    config::{IpPoolFileConfig, IpPoolRuntimeConfig, UserStaticIp},
    EffectiveIpPoolConfig, IpPool, IpSelectionStrategy,
};
use tempfile::TempDir;

struct IpPoolTestEnv {
    _base_dir: TempDir,
    shared_pool: Arc<Mutex<IpPool>>,
}

impl IpPoolTestEnv {
    fn new() -> Self {
        let base_dir = tempfile::tempdir().expect("create temp base dir");
        loader::set_global_base_dir(base_dir.path());

        let runtime_cfg = IpPoolRuntimeConfig::default();
        let file_cfg = IpPoolFileConfig::default();
        let effective = EffectiveIpPoolConfig::from_parts(runtime_cfg.clone(), file_cfg.clone());
        let shared_pool = Arc::new(Mutex::new(IpPool::new(effective)));

        Self {
            _base_dir: base_dir,
            shared_pool,
        }
    }
}

#[test]
fn test_ip_pool_default_state() {
    let env = IpPoolTestEnv::new();

    let pool = env.shared_pool.lock().expect("lock pool");
    let runtime = pool.runtime_config();

    assert!(runtime.enabled);
    assert!(pool.auto_disabled_until().is_none());
}

#[test]
fn test_ip_pool_update_config() {
    let env = IpPoolTestEnv::new();

    let mut runtime_update = IpPoolRuntimeConfig::default();
    runtime_update.enabled = true;
    runtime_update.max_parallel_probes = 16;
    runtime_update.history_path = Some("cache/history.json".to_string());
    runtime_update.failure_threshold = 6;
    runtime_update.failure_rate_threshold = 0.35;

    let mut file_update = IpPoolFileConfig::default();
    file_update.score_ttl_seconds = 900;
    file_update.user_static = vec![UserStaticIp {
        host: "example.com".into(),
        ip: "203.0.113.10".into(),
        ports: vec![443, 8443],
    }];
    file_update.blacklist = vec!["10.0.0.0/8".into()];
    file_update.whitelist = vec!["203.0.113.0/24".into()];

    let effective = EffectiveIpPoolConfig::from_parts(runtime_update.clone(), file_update.clone());

    {
        let mut pool = env.shared_pool.lock().expect("lock pool");
        pool.update_config(effective);
    }

    let pool = env.shared_pool.lock().expect("lock pool after update");
    assert!(pool.runtime_config().enabled);
    assert_eq!(pool.runtime_config().failure_threshold, 6);
    assert_eq!(
        pool.runtime_config().history_path,
        Some("cache/history.json".into())
    );
}

#[test]
fn test_ip_pool_auto_disable_and_clear() {
    let env = IpPoolTestEnv::new();

    // Set auto disabled
    {
        let pool = env.shared_pool.lock().expect("lock pool");
        pool.set_auto_disabled("test", 30_000);
    }

    // Verify it's disabled
    {
        let pool = env.shared_pool.lock().expect("lock pool after disable");
        assert!(pool.auto_disabled_until().is_some());
    }

    // Clear it
    {
        let pool = env.shared_pool.lock().expect("lock pool for clear");
        pool.clear_auto_disabled();
    }

    // Verify it's cleared
    let pool = env.shared_pool.lock().expect("lock pool after clear");
    assert!(pool.auto_disabled_until().is_none());
}

#[tokio::test]
async fn test_ip_pool_pick_best_fallback() {
    let env = IpPoolTestEnv::new();

    let result = {
        let pool = env.shared_pool.lock().expect("lock pool");
        pool.pick_best("github.com", 443).await
    };

    // With default config, may use Cached or SystemDefault depending on cache state
    assert!(
        matches!(
            result.strategy(),
            IpSelectionStrategy::SystemDefault | IpSelectionStrategy::Cached
        ),
        "Expected SystemDefault or Cached, got {:?}",
        result.strategy()
    );
    // selected may or may not be present depending on cache
}

#[test]
fn test_effective_config_from_parts() {
    let runtime = IpPoolRuntimeConfig {
        enabled: true,
        max_parallel_probes: 8,
        failure_threshold: 5,
        failure_rate_threshold: 0.5,
        ..Default::default()
    };

    let file = IpPoolFileConfig {
        score_ttl_seconds: 600,
        user_static: vec![UserStaticIp {
            host: "test.com".into(),
            ip: "1.2.3.4".into(),
            ports: vec![443],
        }],
        ..Default::default()
    };

    let effective = EffectiveIpPoolConfig::from_parts(runtime.clone(), file.clone());

    // EffectiveIpPoolConfig merges runtime and file configs
    assert!(effective.runtime.enabled);
    assert_eq!(effective.runtime.max_parallel_probes, 8);
    assert_eq!(effective.file.score_ttl_seconds, 600);
    assert_eq!(effective.file.user_static.len(), 1);
}
