//! Config 模块综合测试
//! 合并了 config/loader_tests.rs 和 config/model_tests.rs

// ============================================================================
// loader_tests.rs 的测试
// ============================================================================

use fireworks_collaboration_lib::core::config::loader::{load_or_init_at, save_at};
use fireworks_collaboration_lib::core::config::model::AppConfig;
use std::fs;
use std::path::Path;
use std::sync::{Mutex, OnceLock};

fn test_guard() -> &'static Mutex<()> {
    static G: OnceLock<Mutex<()>> = OnceLock::new();
    G.get_or_init(|| Mutex::new(()))
}

fn with_temp_cwd<T>(name: &str, f: impl FnOnce() -> T) -> T {
    let _lock = test_guard().lock().unwrap();
    let old = std::env::current_dir().unwrap();
    let base =
        std::env::temp_dir().join(format!("fwc-p01-{}-{}", name, uuid::Uuid::new_v4()));
    fs::create_dir_all(&base).unwrap();
    std::env::set_current_dir(&base).unwrap();
    let res = f();
    std::env::set_current_dir(&old).unwrap();
    let _ = fs::remove_dir_all(&base);
    res
}

#[test]
fn test_load_or_init_creates_default_at_base() {
    with_temp_cwd("create-default", || {
        assert!(!std::path::Path::new("config/config.json").exists());
        let cfg =
            load_or_init_at(Path::new(".")).expect("should create default config at base");
        assert!(std::path::Path::new("config/config.json").exists());
        // 校验部分默认值
        assert!(cfg.http.fake_sni_enabled);
        assert!(cfg
            .tls
            .san_whitelist
            .iter()
            .any(|d| d.contains("github.com")));
        assert_eq!(cfg.logging.log_level, "info");
    });
}

#[test]
fn test_save_and_reload_roundtrip_at_base() {
    with_temp_cwd("save-reload", || {
        let mut cfg = AppConfig::default();
        cfg.http.fake_sni_enabled = false;
        cfg.http.max_redirects = 3;
        save_at(&cfg, Path::new(".")).expect("save should succeed");
        // 再次读取
        let loaded = load_or_init_at(Path::new(".")).expect("load should succeed");
        assert_eq!(loaded.http.fake_sni_enabled, false);
        assert_eq!(loaded.http.max_redirects, 3);
    });
}

// ============================================================================
// model_tests.rs 的测试
// ============================================================================

use fireworks_collaboration_lib::core::config::model::{
    default_auto_disable_cooldown_sec, default_auto_disable_threshold_pct,
};

#[test]
fn test_serialize_camel_case_keys() {
    let cfg = AppConfig::default();
    let s = serde_json::to_string(&cfg).unwrap();
    // 关键字段以 camelCase 出现
    assert!(s.contains("\"fakeSniEnabled\""));
    assert!(s.contains("\"followRedirects\""));
    assert!(s.contains("\"maxRedirects\""));
    assert!(s.contains("\"largeBodyWarnBytes\""));
    assert!(s.contains("\"autoDisableFakeThresholdPct\""));
    assert!(s.contains("\"autoDisableFakeCooldownSec\""));
    assert!(s.contains("\"sanWhitelist\""));
    assert!(s.contains("\"authHeaderMasked\""));
    assert!(s.contains("\"logLevel\""));
    assert!(s.contains("\"retry\""));
    assert!(s.contains("\"baseMs\""));
    assert!(s.contains("\"factor\""));
    assert!(s.contains("\"jitter\""));
    assert!(s.contains("\"partialFilterSupported\""));
    assert!(s.contains("\"realHostVerifyEnabled\""));
    assert!(s.contains("\"metricsEnabled\""));
    assert!(s.contains("\"certFpLogEnabled\""));
    assert!(s.contains("\"certFpMaxBytes\""));
    assert!(s.contains("\"spkiPins\""));
    assert!(s.contains("\"ipPool\""));
    assert!(s.contains("\"proxy\""));
}

#[test]
fn test_deserialize_with_defaults() {
    // 只提供部分字段,其他应回退默认
    let json = r#"{
      "http": { "fakeSniEnabled": false },
      "tls": {},
                "logging": { "logLevel": "debug" },
                "retry": { "max": 2, "baseMs": 200, "factor": 2.0, "jitter": false }
    }"#;
    let cfg: AppConfig = serde_json::from_str(json).unwrap();
    // 提供的值覆盖
    assert!(!cfg.http.fake_sni_enabled);
    assert_eq!(cfg.logging.log_level, "debug");
    assert_eq!(cfg.retry.max, 2);
    assert_eq!(cfg.retry.base_ms, 200);
    assert!(!cfg.retry.jitter);
    // 未提供的保持默认
    assert!(cfg.http.follow_redirects);
    assert_eq!(cfg.http.max_redirects, 5);
    assert!(cfg
        .tls
        .san_whitelist
        .iter()
        .any(|d| d.ends_with("github.com")));
    assert_eq!(
        cfg.http.auto_disable_fake_threshold_pct,
        default_auto_disable_threshold_pct()
    );
    assert_eq!(
        cfg.http.auto_disable_fake_cooldown_sec,
        default_auto_disable_cooldown_sec()
    );
    assert!(
        cfg.tls.real_host_verify_enabled,
        "realHostVerifyEnabled default true"
    );
    assert!(cfg.tls.metrics_enabled, "metricsEnabled default true");
    assert!(cfg.tls.cert_fp_log_enabled, "certFpLogEnabled default true");
    assert_eq!(cfg.tls.cert_fp_max_bytes, 5 * 1024 * 1024);
    // P3.4: spkiPins default empty
    assert!(cfg.tls.spki_pins.is_empty());
    assert!(!cfg.ip_pool.enabled, "ipPool defaults to disabled");
    // P5.0: proxy defaults to off mode
    assert!(!cfg.proxy.is_enabled(), "proxy defaults to disabled");
}
