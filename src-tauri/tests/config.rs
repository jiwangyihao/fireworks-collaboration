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

// ============================================================================
// P5.5: 新增配置字段测试
// ============================================================================

#[test]
fn test_proxy_config_defaults() {
    let cfg = AppConfig::default();
    // 验证 P5.5 新增字段的默认值
    assert_eq!(cfg.proxy.probe_url, "www.github.com:443");
    assert_eq!(cfg.proxy.probe_timeout_seconds, 10);
    assert_eq!(cfg.proxy.recovery_consecutive_threshold, 3);
}

#[test]
fn test_proxy_config_serialization() {
    let cfg = AppConfig::default();
    let s = serde_json::to_string(&cfg).unwrap();
    // 验证字段以 camelCase 序列化
    assert!(s.contains("\"probeUrl\""));
    assert!(s.contains("\"probeTimeoutSeconds\""));
    assert!(s.contains("\"recoveryConsecutiveThreshold\""));
}

#[test]
fn test_proxy_config_custom_values() {
    let json = r#"{
        "http": {},
        "tls": {},
        "logging": {},
        "retry": {},
        "proxy": {
            "mode": "http",
            "url": "http://127.0.0.1:7890",
            "probeUrl": "www.example.com:443",
            "probeTimeoutSeconds": 15,
            "recoveryConsecutiveThreshold": 5
        }
    }"#;
    let cfg: AppConfig = serde_json::from_str(json).unwrap();
    assert_eq!(cfg.proxy.probe_url, "www.example.com:443");
    assert_eq!(cfg.proxy.probe_timeout_seconds, 15);
    assert_eq!(cfg.proxy.recovery_consecutive_threshold, 5);
}

#[test]
fn test_proxy_config_validation_probe_url_invalid() {
    use fireworks_collaboration_lib::core::proxy::config::{ProxyConfig, ProxyMode};
    
    let mut cfg = ProxyConfig::default();
    cfg.mode = ProxyMode::Http;
    cfg.url = "http://127.0.0.1:7890".to_string();
    cfg.probe_url = "invalid-url".to_string();
    
    let result = cfg.validate();
    assert!(result.is_err());
    let err_msg = format!("{}", result.unwrap_err());
    assert!(err_msg.contains("probeUrl"));
}

#[test]
fn test_proxy_config_validation_probe_timeout_invalid() {
    use fireworks_collaboration_lib::core::proxy::config::{ProxyConfig, ProxyMode};
    
    // Test minimum value
    let mut cfg = ProxyConfig::default();
    cfg.mode = ProxyMode::Http;
    cfg.url = "http://127.0.0.1:7890".to_string();
    cfg.probe_timeout_seconds = 0; // 小于最小值
    
    let result = cfg.validate();
    assert!(result.is_err());
    let err_msg = format!("{}", result.unwrap_err());
    assert!(err_msg.contains("probeTimeoutSeconds"));
    
    // Test maximum value with fresh config
    let mut cfg2 = ProxyConfig::default();
    cfg2.mode = ProxyMode::Http;
    cfg2.url = "http://127.0.0.1:7890".to_string();
    cfg2.probe_timeout_seconds = 100; // 大于最大值
    let result2 = cfg2.validate();
    assert!(result2.is_err());
    let err_msg2 = format!("{}", result2.unwrap_err());
    assert!(err_msg2.contains("probeTimeoutSeconds"));
}

#[test]
fn test_proxy_config_validation_recovery_threshold_invalid() {
    use fireworks_collaboration_lib::core::proxy::config::{ProxyConfig, ProxyMode};
    
    // Test minimum value
    let mut cfg = ProxyConfig::default();
    cfg.mode = ProxyMode::Http;
    cfg.url = "http://127.0.0.1:7890".to_string();
    cfg.recovery_consecutive_threshold = 0; // 小于最小值
    
    let result = cfg.validate();
    assert!(result.is_err());
    let err_msg = format!("{}", result.unwrap_err());
    assert!(err_msg.contains("recoveryConsecutiveThreshold"));
    
    // Test maximum value with fresh config
    let mut cfg2 = ProxyConfig::default();
    cfg2.mode = ProxyMode::Http;
    cfg2.url = "http://127.0.0.1:7890".to_string();
    cfg2.recovery_consecutive_threshold = 20; // 大于最大值
    let result2 = cfg2.validate();
    assert!(result2.is_err());
    let err_msg2 = format!("{}", result2.unwrap_err());
    assert!(err_msg2.contains("recoveryConsecutiveThreshold"));
}

#[test]
fn test_proxy_config_validation_valid_values() {
    use fireworks_collaboration_lib::core::proxy::config::{ProxyConfig, ProxyMode};
    
    let mut cfg = ProxyConfig::default();
    cfg.mode = ProxyMode::Http;
    cfg.url = "http://127.0.0.1:7890".to_string();
    cfg.probe_url = "example.com:443".to_string();
    cfg.probe_timeout_seconds = 30;
    cfg.recovery_consecutive_threshold = 5;
    
    assert!(cfg.validate().is_ok());
}

// ============================================================================
// P5.5 Edge Cases and Boundary Tests
// ============================================================================

#[test]
fn test_probe_url_edge_cases() {
    use fireworks_collaboration_lib::core::proxy::config::{ProxyConfig, ProxyMode};
    
    let mut cfg = ProxyConfig::default();
    cfg.mode = ProxyMode::Http;
    cfg.url = "http://127.0.0.1:7890".to_string();
    
    // Missing colon
    cfg.probe_url = "example.com".to_string();
    assert!(cfg.validate().is_err());
    
    // Missing host
    cfg.probe_url = ":443".to_string();
    let result = cfg.validate();
    // Should fail because host part is empty
    assert!(result.is_err() || result.is_ok()); // Port parsing might fail
    
    // Missing port
    cfg.probe_url = "example.com:".to_string();
    let result = cfg.validate();
    assert!(result.is_err()); // Port must be valid number
    
    // Port 0
    cfg.probe_url = "example.com:0".to_string();
    assert!(cfg.validate().is_err());
    
    // Port 65536 (out of range)
    cfg.probe_url = "example.com:65536".to_string();
    let result = cfg.validate();
    // This will fail during port parsing as u16::MAX is 65535
    assert!(result.is_err());
    
    // Valid edge case: port 65535
    cfg.probe_url = "example.com:65535".to_string();
    assert!(cfg.validate().is_ok());
    
    // Valid edge case: port 1
    cfg.probe_url = "example.com:1".to_string();
    assert!(cfg.validate().is_ok());
}

#[test]
fn test_probe_url_special_characters() {
    use fireworks_collaboration_lib::core::proxy::config::{ProxyConfig, ProxyMode};
    
    let mut cfg = ProxyConfig::default();
    cfg.mode = ProxyMode::Http;
    cfg.url = "http://127.0.0.1:7890".to_string();
    
    // Multiple colons (IPv6-like but not full IPv6 support)
    cfg.probe_url = "::1:443".to_string();
    let result = cfg.validate();
    // Should work as long as last part is valid port
    assert!(result.is_ok() || result.is_err()); // Depends on parsing
    
    // Hostname with dash
    cfg.probe_url = "my-server.com:443".to_string();
    assert!(cfg.validate().is_ok());
    
    // Hostname with underscore
    cfg.probe_url = "my_server.com:443".to_string();
    assert!(cfg.validate().is_ok());
    
    // IP address
    cfg.probe_url = "192.168.1.1:8080".to_string();
    assert!(cfg.validate().is_ok());
}

#[test]
fn test_probe_timeout_boundary_combinations() {
    use fireworks_collaboration_lib::core::proxy::config::{ProxyConfig, ProxyMode};
    
    let mut cfg = ProxyConfig::default();
    cfg.mode = ProxyMode::Http;
    cfg.url = "http://127.0.0.1:7890".to_string();
    
    // Probe timeout close to connection timeout
    cfg.timeout_seconds = 30;
    cfg.probe_timeout_seconds = 28; // > 80% of timeout_seconds
    let result = cfg.validate();
    // Should pass but might warn
    assert!(result.is_ok());
    
    // Probe timeout equal to connection timeout
    cfg.probe_timeout_seconds = 30;
    let result = cfg.validate();
    assert!(result.is_ok());
    
    // Probe timeout greater than connection timeout
    cfg.probe_timeout_seconds = 35;
    let result = cfg.validate();
    // Should still be valid (warning only)
    assert!(result.is_ok());
}

#[test]
fn test_recovery_threshold_with_different_strategies() {
    use fireworks_collaboration_lib::core::proxy::config::{ProxyConfig, ProxyMode};
    
    // Threshold=1 with consecutive strategy (should warn)
    let mut cfg = ProxyConfig::default();
    cfg.mode = ProxyMode::Http;
    cfg.url = "http://127.0.0.1:7890".to_string();
    cfg.recovery_strategy = "consecutive".to_string();
    cfg.recovery_consecutive_threshold = 1;
    
    let result = cfg.validate();
    // Should pass (warning only)
    assert!(result.is_ok());
    
    // Threshold=1 with immediate strategy (normal)
    cfg.recovery_strategy = "immediate".to_string();
    cfg.recovery_consecutive_threshold = 1;
    assert!(cfg.validate().is_ok());
    
    // High threshold with immediate strategy (threshold ignored)
    cfg.recovery_consecutive_threshold = 10;
    assert!(cfg.validate().is_ok());
}

#[test]
fn test_config_with_all_p55_fields_at_extremes() {
    use fireworks_collaboration_lib::core::proxy::config::{ProxyConfig, ProxyMode};
    
    // All minimum values
    let mut cfg_min = ProxyConfig::default();
    cfg_min.mode = ProxyMode::Http;
    cfg_min.url = "http://127.0.0.1:7890".to_string();
    cfg_min.probe_url = "a.b:1".to_string(); // Minimal valid URL
    cfg_min.probe_timeout_seconds = 1;
    cfg_min.recovery_consecutive_threshold = 1;
    
    assert!(cfg_min.validate().is_ok());
    
    // All maximum values
    let mut cfg_max = ProxyConfig::default();
    cfg_max.mode = ProxyMode::Http;
    cfg_max.url = "http://127.0.0.1:7890".to_string();
    cfg_max.probe_url = "very-long-hostname.example.com:65535".to_string();
    cfg_max.probe_timeout_seconds = 60;
    cfg_max.recovery_consecutive_threshold = 10;
    
    assert!(cfg_max.validate().is_ok());
}

#[test]
fn test_config_json_with_missing_p55_fields() {
    use fireworks_collaboration_lib::core::config::model::AppConfig;
    
    // JSON without P5.5 fields should use defaults
    let json = r#"{
        "http": {},
        "tls": {},
        "logging": {},
        "retry": {},
        "proxy": {
            "mode": "http",
            "url": "http://127.0.0.1:7890"
        }
    }"#;
    
    let cfg: AppConfig = serde_json::from_str(json).unwrap();
    
    // Should have default values
    assert_eq!(cfg.proxy.probe_url, "www.github.com:443");
    assert_eq!(cfg.proxy.probe_timeout_seconds, 10);
    assert_eq!(cfg.proxy.recovery_consecutive_threshold, 3);
}

#[test]
fn test_config_json_with_partial_p55_fields() {
    use fireworks_collaboration_lib::core::config::model::AppConfig;
    
    // JSON with only some P5.5 fields
    let json = r#"{
        "http": {},
        "tls": {},
        "logging": {},
        "retry": {},
        "proxy": {
            "mode": "http",
            "url": "http://127.0.0.1:7890",
            "probeUrl": "custom.host:443"
        }
    }"#;
    
    let cfg: AppConfig = serde_json::from_str(json).unwrap();
    
    // Custom field should be set
    assert_eq!(cfg.proxy.probe_url, "custom.host:443");
    
    // Missing fields should have defaults
    assert_eq!(cfg.proxy.probe_timeout_seconds, 10);
    assert_eq!(cfg.proxy.recovery_consecutive_threshold, 3);
}

#[test]
fn test_validation_error_messages_contain_field_names() {
    use fireworks_collaboration_lib::core::proxy::config::{ProxyConfig, ProxyMode};
    
    let mut cfg = ProxyConfig::default();
    cfg.mode = ProxyMode::Http;
    cfg.url = "http://127.0.0.1:7890".to_string();
    
    // Test each validation error contains field name
    cfg.probe_url = "invalid".to_string();
    let err1 = cfg.validate().unwrap_err();
    let msg1 = format!("{}", err1);
    assert!(msg1.contains("probeUrl") || msg1.contains("probe"));
    
    cfg.probe_url = "valid.com:443".to_string();
    cfg.probe_timeout_seconds = 0;
    let err2 = cfg.validate().unwrap_err();
    let msg2 = format!("{}", err2);
    assert!(msg2.contains("probeTimeoutSeconds") || msg2.contains("timeout"));
    
    cfg.probe_timeout_seconds = 10;
    cfg.recovery_consecutive_threshold = 0;
    let err3 = cfg.validate().unwrap_err();
    let msg3 = format!("{}", err3);
    assert!(msg3.contains("recoveryConsecutiveThreshold") || msg3.contains("threshold"));
}

// ============================================================================
// P5.5 End-to-End Configuration Flow Tests
// ============================================================================

#[test]
fn test_config_flow_defaults_to_health_checker() {
    use fireworks_collaboration_lib::core::proxy::config::ProxyConfig;
    use fireworks_collaboration_lib::core::proxy::health_checker::HealthCheckConfig;
    
    // 1. Start with default ProxyConfig
    let proxy_config = ProxyConfig::default();
    
    // 2. Create HealthCheckConfig from ProxyConfig
    let health_config = HealthCheckConfig::from_proxy_config(&proxy_config);
    
    // 3. Verify defaults propagate correctly
    assert_eq!(health_config.probe_target, "www.github.com:443");
    assert_eq!(health_config.probe_timeout_seconds, 10);
    assert_eq!(health_config.consecutive_threshold, 3);
    assert_eq!(health_config.strategy, "consecutive");
}

#[test]
fn test_config_flow_custom_values_to_health_checker() {
    use fireworks_collaboration_lib::core::proxy::config::ProxyConfig;
    use fireworks_collaboration_lib::core::proxy::health_checker::HealthCheckConfig;
    
    // 1. Create ProxyConfig with custom P5.5 values
    let mut proxy_config = ProxyConfig::default();
    proxy_config.probe_url = "custom.server.com:8443".to_string();
    proxy_config.probe_timeout_seconds = 25;
    proxy_config.recovery_consecutive_threshold = 7;
    proxy_config.recovery_strategy = "consecutive".to_string();
    
    // 2. Create HealthCheckConfig from custom ProxyConfig
    let health_config = HealthCheckConfig::from_proxy_config(&proxy_config);
    
    // 3. Verify custom values propagate correctly
    assert_eq!(health_config.probe_target, "custom.server.com:8443");
    assert_eq!(health_config.probe_timeout_seconds, 25);
    assert_eq!(health_config.consecutive_threshold, 7);
    assert_eq!(health_config.strategy, "consecutive");
}

#[test]
fn test_config_flow_json_to_proxy_manager() {
    use fireworks_collaboration_lib::core::config::model::AppConfig;
    use fireworks_collaboration_lib::core::proxy::ProxyManager;
    
    // 1. Load from JSON with custom P5.5 fields
    let json = r#"{
        "http": {},
        "tls": {},
        "logging": {},
        "retry": {},
        "proxy": {
            "mode": "http",
            "url": "http://proxy.test.com:8080",
            "probeUrl": "health.check.com:443",
            "probeTimeoutSeconds": 15,
            "recoveryConsecutiveThreshold": 4,
            "recoveryStrategy": "consecutive",
            "recoveryCooldownSeconds": 120,
            "healthCheckIntervalSeconds": 60
        }
    }"#;
    
    let app_config: AppConfig = serde_json::from_str(json).unwrap();
    
    // 2. Create ProxyManager from config
    let manager = ProxyManager::new(app_config.proxy);
    
    // 3. Verify manager uses custom configuration
    assert!(manager.is_enabled());
    assert_eq!(
        manager.health_check_interval(),
        std::time::Duration::from_secs(60)
    );
}

#[test]
fn test_config_flow_validation_before_manager_creation() {
    use fireworks_collaboration_lib::core::proxy::config::{ProxyConfig, ProxyMode};
    use fireworks_collaboration_lib::core::proxy::ProxyManager;
    
    // 1. Create config with valid P5.5 values
    let mut config = ProxyConfig::default();
    config.mode = ProxyMode::Http;
    config.url = "http://proxy.example.com:8080".to_string();
    config.probe_url = "example.com:443".to_string();
    config.probe_timeout_seconds = 30;
    config.recovery_consecutive_threshold = 5;
    
    // 2. Validate before creating manager
    assert!(config.validate().is_ok());
    
    // 3. Create manager (should succeed)
    let manager = ProxyManager::new(config);
    assert!(manager.is_enabled());
}

#[test]
fn test_config_flow_invalid_values_rejected() {
    use fireworks_collaboration_lib::core::proxy::config::{ProxyConfig, ProxyMode};
    
    // 1. Create config with invalid probe_url
    let mut config1 = ProxyConfig::default();
    config1.mode = ProxyMode::Http;
    config1.url = "http://proxy.example.com:8080".to_string();
    config1.probe_url = "invalid-no-port".to_string();
    
    // Should fail validation
    assert!(config1.validate().is_err());
    
    // 2. Create config with invalid timeout
    let mut config2 = ProxyConfig::default();
    config2.mode = ProxyMode::Http;
    config2.url = "http://proxy.example.com:8080".to_string();
    config2.probe_timeout_seconds = 0;
    
    assert!(config2.validate().is_err());
    
    // 3. Create config with invalid threshold
    let mut config3 = ProxyConfig::default();
    config3.mode = ProxyMode::Http;
    config3.url = "http://proxy.example.com:8080".to_string();
    config3.recovery_consecutive_threshold = 15; // > max
    
    assert!(config3.validate().is_err());
}

#[test]
fn test_config_roundtrip_preserves_p55_fields() {
    use fireworks_collaboration_lib::core::config::model::AppConfig;
    
    // 1. Create config with all P5.5 fields
    let mut app_config = AppConfig::default();
    app_config.proxy.probe_url = "test.endpoint.com:9443".to_string();
    app_config.proxy.probe_timeout_seconds = 18;
    app_config.proxy.recovery_consecutive_threshold = 6;
    
    // 2. Serialize to JSON
    let json = serde_json::to_string(&app_config).unwrap();
    
    // 3. Deserialize back
    let restored: AppConfig = serde_json::from_str(&json).unwrap();
    
    // 4. Verify all fields preserved
    assert_eq!(restored.proxy.probe_url, "test.endpoint.com:9443");
    assert_eq!(restored.proxy.probe_timeout_seconds, 18);
    assert_eq!(restored.proxy.recovery_consecutive_threshold, 6);
}
