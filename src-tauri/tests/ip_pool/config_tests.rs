use fireworks_collaboration_lib::core::ip_pool::config::{
    default_cache_prune_interval_secs, default_cooldown_seconds, default_failure_rate_threshold,
    default_failure_threshold, default_failure_window_seconds, default_max_cache_entries,
    default_max_parallel_probes, default_min_samples_in_window, default_probe_timeout_ms,
    default_score_ttl_seconds, default_singleflight_timeout_ms, EffectiveIpPoolConfig,
    IpPoolFileConfig, IpPoolRuntimeConfig, PreheatDomain, UserStaticIp,
};
use fireworks_collaboration_lib::core::ip_pool::config::{
    join_ip_config_path, load_or_init_file_at, save_file_at,
};
use std::fs;
use std::sync::{Mutex, OnceLock};

#[test]
fn runtime_defaults_are_disabled() {
    let cfg = IpPoolRuntimeConfig::default();
    assert!(!cfg.enabled);
    assert_eq!(cfg.max_parallel_probes, default_max_parallel_probes());
    assert_eq!(cfg.probe_timeout_ms, default_probe_timeout_ms());
    assert!(cfg.history_path.is_none());
    assert!(cfg.sources.builtin);
    assert!(cfg.sources.dns);
    assert!(cfg.sources.history);
    assert!(cfg.sources.user_static);
    assert!(cfg.sources.fallback);
    assert_eq!(
        cfg.cache_prune_interval_secs,
        default_cache_prune_interval_secs()
    );
    assert_eq!(cfg.max_cache_entries, default_max_cache_entries());
    assert_eq!(
        cfg.singleflight_timeout_ms,
        default_singleflight_timeout_ms()
    );
    assert_eq!(cfg.failure_threshold, default_failure_threshold());
    assert_eq!(cfg.failure_rate_threshold, default_failure_rate_threshold());
    assert_eq!(cfg.failure_window_seconds, default_failure_window_seconds());
    assert_eq!(cfg.min_samples_in_window, default_min_samples_in_window());
    assert_eq!(cfg.cooldown_seconds, default_cooldown_seconds());
    assert!(cfg.circuit_breaker_enabled);
}

#[test]
fn file_defaults_are_empty_preheat() {
    let cfg = IpPoolFileConfig::default();
    assert!(cfg.preheat_domains.is_empty());
    assert_eq!(cfg.score_ttl_seconds, default_score_ttl_seconds());
    assert!(cfg.user_static.is_empty());
    assert!(cfg.blacklist.is_empty());
    assert!(cfg.whitelist.is_empty());
}

#[test]
fn deserializes_with_defaults() {
    let json = r#"{
        "runtime": {
            "enabled": true,
            "maxParallelProbes": 8,
            "historyPath": "custom/ip-history.json"
        },
        "file": {
            "preheatDomains": [{"host": "github.com", "ports": [443, 80]}]
        }
    }"#;
    let cfg: EffectiveIpPoolConfig = serde_json::from_str(json).unwrap();
    assert!(cfg.runtime.enabled);
    assert_eq!(cfg.runtime.max_parallel_probes, 8);
    assert_eq!(cfg.runtime.probe_timeout_ms, default_probe_timeout_ms());
    assert_eq!(cfg.file.score_ttl_seconds, default_score_ttl_seconds());
    assert_eq!(cfg.file.preheat_domains.len(), 1);
    let domain = &cfg.file.preheat_domains[0];
    assert_eq!(domain.host, "github.com");
    assert_eq!(domain.ports, vec![443, 80]);
    assert!(cfg.file.user_static.is_empty());
    assert_eq!(
        cfg.runtime.cache_prune_interval_secs,
        default_cache_prune_interval_secs()
    );
    assert_eq!(cfg.runtime.max_cache_entries, default_max_cache_entries());
    assert_eq!(
        cfg.runtime.singleflight_timeout_ms,
        default_singleflight_timeout_ms()
    );
}

#[test]
fn load_or_init_file_creates_default() {
    let guard = test_guard().lock().unwrap();
    let temp_dir = std::env::temp_dir().join(format!("fwc-ip-pool-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&temp_dir).unwrap();
    let cfg = load_or_init_file_at(&temp_dir).expect("create default ip config");
    assert!(cfg.preheat_domains.is_empty());
    assert_eq!(cfg.score_ttl_seconds, default_score_ttl_seconds());
    let path = join_ip_config_path(&temp_dir);
    assert!(path.exists());
    fs::remove_dir_all(&temp_dir).ok();
    drop(guard);
}

#[test]
fn save_file_persists_changes() {
    let guard = test_guard().lock().unwrap();
    let temp_dir =
        std::env::temp_dir().join(format!("fwc-ip-pool-save-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&temp_dir).unwrap();
    let mut cfg = IpPoolFileConfig::default();
    cfg.preheat_domains.push(PreheatDomain::new("github.com"));
    cfg.score_ttl_seconds = 120;
    cfg.user_static.push(UserStaticIp {
        host: "github.com".into(),
        ip: "140.82.112.3".into(),
        ports: vec![443],
    });
    save_file_at(&cfg, &temp_dir).expect("save ip config");
    let loaded = load_or_init_file_at(&temp_dir).expect("load ip config");
    assert_eq!(loaded.preheat_domains.len(), 1);
    assert_eq!(loaded.score_ttl_seconds, 120);
    assert_eq!(loaded.user_static.len(), 1);
    fs::remove_dir_all(&temp_dir).ok();
    drop(guard);
}

fn test_guard() -> &'static Mutex<()> {
    static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
    GUARD.get_or_init(|| Mutex::new(()))
}
