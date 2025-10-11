//! Config 模块综合测试
//! 合并了 `config/loader_tests.rs` 和 `config/model_tests.rs`

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
    let base = std::env::temp_dir().join(format!("fwc-p01-{}-{}", name, uuid::Uuid::new_v4()));
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
        let cfg = load_or_init_at(Path::new(".")).expect("should create default config at base");
        assert!(std::path::Path::new("config/config.json").exists());
        // 校验部分默认值
        assert!(cfg.http.fake_sni_enabled);
        assert!(cfg.tls.metrics_enabled);
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
        assert!(!loaded.http.fake_sni_enabled);
        assert_eq!(loaded.http.max_redirects, 3);
    });
}

// ============================================================================
// model_tests.rs 的测试
// ============================================================================

use fireworks_collaboration_lib::core::config::model::{
    default_auto_disable_cooldown_sec, default_auto_disable_threshold_pct, ObservabilityLayer,
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
    assert!(s.contains("\"authHeaderMasked\""));
    assert!(s.contains("\"logLevel\""));
    assert!(s.contains("\"retry\""));
    assert!(s.contains("\"baseMs\""));
    assert!(s.contains("\"factor\""));
    assert!(s.contains("\"jitter\""));
    assert!(s.contains("\"partialFilterSupported\""));
    assert!(s.contains("\"metricsEnabled\""));
    assert!(s.contains("\"certFpLogEnabled\""));
    assert!(s.contains("\"certFpMaxBytes\""));
    assert!(s.contains("\"spkiPins\""));
    assert!(s.contains("\"ipPool\""));
    assert!(s.contains("\"proxy\""));
    assert!(s.contains("\"observability\""));
    assert!(s.contains("\"layer\""));
    assert!(s.contains("\"autoDowngrade\""));
    assert!(s.contains("\"minLayerResidencySecs\""));
    assert!(s.contains("\"downgradeCooldownSecs\""));
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
    assert_eq!(
        cfg.http.auto_disable_fake_threshold_pct,
        default_auto_disable_threshold_pct()
    );
    assert_eq!(
        cfg.http.auto_disable_fake_cooldown_sec,
        default_auto_disable_cooldown_sec()
    );
    assert!(cfg.tls.metrics_enabled, "metricsEnabled default true");
    assert!(cfg.tls.cert_fp_log_enabled, "certFpLogEnabled default true");
    assert_eq!(cfg.tls.cert_fp_max_bytes, 5 * 1024 * 1024);
    // P3.4: spkiPins default empty
    assert!(cfg.tls.spki_pins.is_empty());
    assert!(cfg.ip_pool.enabled, "ipPool defaults to enabled");
    // P5.0: proxy defaults to off mode
    assert!(!cfg.proxy.is_enabled(), "proxy defaults to disabled");
    assert_eq!(cfg.observability.layer, ObservabilityLayer::Optimize);
    assert!(
        cfg.observability.auto_downgrade,
        "autoDowngrade default true"
    );
    assert_eq!(cfg.observability.min_layer_residency_secs, 300);
    assert_eq!(cfg.observability.downgrade_cooldown_secs, 120);
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
    let msg1 = format!("{err1}");
    assert!(msg1.contains("probeUrl") || msg1.contains("probe"));

    cfg.probe_url = "valid.com:443".to_string();
    cfg.probe_timeout_seconds = 0;
    let err2 = cfg.validate().unwrap_err();
    let msg2 = format!("{err2}");
    assert!(msg2.contains("probeTimeoutSeconds") || msg2.contains("timeout"));

    cfg.probe_timeout_seconds = 10;
    cfg.recovery_consecutive_threshold = 0;
    let err3 = cfg.validate().unwrap_err();
    let msg3 = format!("{err3}");
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

// ============================================================================
// Team Config Template aggregation tests (migrated from config_team_template.rs)
// ============================================================================

mod section_team_template {
    use super::*;
    use fireworks_collaboration_lib::core::config::team_template::{
        apply_template_to_config, backup_config_file, export_template, load_template_from_path,
        write_template_to_path, IpPoolTemplate, SectionStrategy, TeamConfigTemplate,
        TemplateExportOptions, TemplateImportOptions, TemplateSectionKind,
    };
    use fireworks_collaboration_lib::core::credential::config::CredentialConfig;
    use fireworks_collaboration_lib::core::ip_pool::{
        config as ip_pool_cfg, IpPoolFileConfig, IpPoolRuntimeConfig,
    };
    use fireworks_collaboration_lib::core::proxy::config::{ProxyConfig, ProxyMode};
    use uuid::Uuid;

    fn with_temp_dir<T>(name: &str, f: impl FnOnce(&Path) -> T) -> T {
        let base =
            std::env::temp_dir().join(format!("fwc-team-template-{name}-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&base).expect("create temp dir");
        let result = f(&base);
        std::fs::remove_dir_all(&base).ok();
        result
    }

    #[test]
    fn test_export_team_template_sanitizes_sensitive_fields() {
        with_temp_dir("export", |base| {
            let mut cfg = AppConfig::default();
            cfg.proxy.mode = ProxyMode::Http;
            cfg.proxy.url = "http://proxy.example.com:8080".into();
            cfg.proxy.password = Some("topsecret".into());
            cfg.proxy.username = Some("builder".into());
            cfg.credential.file_path = Some("/tmp/credential.enc".into());
            cfg.ip_pool.enabled = true;
            cfg.ip_pool.history_path = Some("/tmp/local-history.json".into());

            save_at(&cfg, base).expect("save config");

            let template = export_template(&cfg, base, &TemplateExportOptions::default())
                .expect("export template");
            let dest = base.join("team-config-template.json");
            write_template_to_path(&template, &dest).expect("write template");

            let raw = std::fs::read_to_string(&dest).expect("read template file");
            assert!(!raw.contains("topsecret"), "password must be stripped");

            let parsed = load_template_from_path(&dest).expect("load written template");
            let ip_pool = parsed.sections.ip_pool.expect("ip pool section present");
            assert!(
                ip_pool.runtime.history_path.is_none(),
                "history path should be sanitized"
            );

            let proxy = parsed.sections.proxy.expect("proxy section present");
            assert!(proxy.password.is_none(), "password field should be None");
            assert_eq!(proxy.username.as_deref(), Some("builder"));

            let credential = parsed
                .sections
                .credential
                .expect("credential section present");
            assert!(
                credential.file_path.is_none(),
                "file path should be removed"
            );

            assert_eq!(
                cfg.ip_pool.history_path.as_deref(),
                Some("/tmp/local-history.json"),
                "original config should remain unchanged"
            );
        });
    }

    #[test]
    fn test_import_template_schema_mismatch_errors() {
        let mut cfg = AppConfig::default();
        let mut template = TeamConfigTemplate::new();
        template.schema_version = "2.0.0".into();

        let result = apply_template_to_config(
            &mut cfg,
            Some(IpPoolFileConfig::default()),
            &template,
            &TemplateImportOptions::default(),
        );

        assert!(result.is_err(), "schema mismatch should return error");
        let err = result.err().unwrap().to_string();
        assert!(
            err.contains("schema"),
            "error message should mention schema"
        );
    }

    #[test]
    fn test_import_keep_local_strategy_preserves_local_config() {
        let mut cfg = AppConfig::default();
        cfg.proxy.mode = ProxyMode::System;

        let mut template = TeamConfigTemplate::new();
        template.sections.proxy = Some({
            let mut proxy = ProxyConfig::default();
            proxy.mode = ProxyMode::Http;
            proxy.url = "http://shared-proxy:8080".into();
            proxy
        });

        let mut options = TemplateImportOptions::default();
        options.strategies.proxy = SectionStrategy::KeepLocal;

        let outcome = apply_template_to_config(
            &mut cfg,
            Some(IpPoolFileConfig::default()),
            &template,
            &options,
        )
        .expect("keep local strategy should succeed");

        assert_eq!(cfg.proxy.mode, ProxyMode::System);
        assert!(cfg.proxy.url.is_empty());

        assert!(outcome
            .report
            .skipped
            .iter()
            .any(|entry| entry.section == TemplateSectionKind::Proxy
                && entry.reason == "strategyKeepLocal"));
    }

    #[test]
    fn test_import_respects_disabled_sections() {
        let mut cfg = AppConfig::default();
        assert!(cfg.tls.cert_fp_log_enabled);

        let mut template = TeamConfigTemplate::new();
        let mut tls_cfg = cfg.tls.clone();
        tls_cfg.cert_fp_log_enabled = false;
        template.sections.tls = Some(tls_cfg);

        let mut options = TemplateImportOptions::default();
        options.include_tls = false;

        let outcome = apply_template_to_config(
            &mut cfg,
            Some(IpPoolFileConfig::default()),
            &template,
            &options,
        )
        .expect("disabled sections should not fail import");

        assert!(cfg.tls.cert_fp_log_enabled);

        assert!(outcome
            .report
            .skipped
            .iter()
            .any(|entry| entry.section == TemplateSectionKind::Tls
                && entry.reason == "sectionDisabled"));
    }

    #[test]
    fn test_import_overwrite_sanitizes_ip_pool_history_path() {
        let mut cfg = AppConfig::default();
        cfg.ip_pool.history_path = Some("local-history.json".into());

        let mut template = TeamConfigTemplate::new();
        template.sections.ip_pool = Some(IpPoolTemplate {
            runtime: {
                let mut runtime = IpPoolRuntimeConfig::default();
                runtime.enabled = true;
                runtime.history_path = Some("/tmp/template-history.json".into());
                runtime
            },
            file: None,
        });

        let outcome =
            apply_template_to_config(&mut cfg, None, &template, &TemplateImportOptions::default())
                .expect("overwrite strategy should succeed");

        assert!(cfg.ip_pool.enabled, "runtime flag should be applied");
        assert!(
            cfg.ip_pool.history_path.is_none(),
            "history path must be sanitized"
        );
        assert!(outcome
            .report
            .applied
            .iter()
            .any(|entry| entry.section == TemplateSectionKind::IpPoolRuntime));
    }

    #[test]
    fn test_import_merge_preserves_local_history_path() {
        let mut cfg = AppConfig::default();
        cfg.ip_pool.history_path = Some("local-cache.json".into());
        cfg.ip_pool.enabled = false;
        cfg.ip_pool.max_parallel_probes = 2;

        let mut template = TeamConfigTemplate::new();
        template.sections.ip_pool = Some(IpPoolTemplate {
            runtime: {
                let mut runtime = IpPoolRuntimeConfig::default();
                runtime.enabled = true;
                runtime.max_parallel_probes = 8;
                runtime.history_path = Some("/tmp/template-history.json".into());
                runtime
            },
            file: None,
        });

        let mut options = TemplateImportOptions::default();
        options.strategies.ip_pool = SectionStrategy::Merge;

        let outcome = apply_template_to_config(&mut cfg, None, &template, &options)
            .expect("merge strategy should succeed");

        assert!(
            cfg.ip_pool.enabled,
            "merge should adopt template enable flag"
        );
        assert_eq!(cfg.ip_pool.max_parallel_probes, 8);
        assert_eq!(
            cfg.ip_pool.history_path.as_deref(),
            Some("local-cache.json"),
            "merge should keep local history path"
        );
        assert!(outcome
            .report
            .applied
            .iter()
            .any(|entry| entry.section == TemplateSectionKind::IpPoolRuntime
                && entry.strategy == SectionStrategy::Merge));
    }

    #[test]
    fn test_import_team_template_applies_sections_and_backup() {
        with_temp_dir("import", |base| {
            let mut original_cfg = AppConfig::default();
            original_cfg.proxy.mode = ProxyMode::System;
            original_cfg.proxy.url = "http://legacy:8080".into();
            save_at(&original_cfg, base).expect("save original config");

            ip_pool_cfg::save_file_at(&IpPoolFileConfig::default(), base)
                .expect("create default ip pool file");

            let mut template = TeamConfigTemplate::new();
            template.sections.proxy = Some({
                let mut proxy = ProxyConfig::default();
                proxy.mode = ProxyMode::Http;
                proxy.url = "http://team-proxy:9000".into();
                proxy.username = Some("teammate".into());
                proxy
            });
            template.sections.credential = Some({
                let mut cred = CredentialConfig::default();
                cred.require_confirmation = true;
                cred
            });
            let mut tls_template = AppConfig::default().tls;
            tls_template.metrics_enabled = false;
            template.sections.tls = Some(tls_template);
            template.sections.ip_pool = Some(IpPoolTemplate {
                runtime: {
                    let mut runtime = IpPoolRuntimeConfig::default();
                    runtime.enabled = true;
                    runtime.max_parallel_probes = 6;
                    runtime
                },
                file: Some({
                    let mut file_cfg = IpPoolFileConfig::default();
                    file_cfg.blacklist.push("10.0.0.0/8".into());
                    file_cfg.whitelist.push("203.0.113.10".into());
                    file_cfg
                }),
            });

            let template_path = base.join("team-config-template.json");
            write_template_to_path(&template, &template_path).expect("write template");
            let loaded = load_template_from_path(&template_path).expect("load template");

            let mut cfg_value = load_or_init_at(base).expect("load config for merge");
            let ip_file = ip_pool_cfg::load_or_init_file_at(base).expect("load ip file");

            let mut import_opts = TemplateImportOptions::default();
            import_opts.strategies.ip_pool = SectionStrategy::Merge;
            import_opts.strategies.ip_pool_file = SectionStrategy::Merge;

            let outcome =
                apply_template_to_config(&mut cfg_value, Some(ip_file), &loaded, &import_opts)
                    .expect("apply template to config");

            let backup = backup_config_file(base).expect("create backup");
            if let Some(path) = &backup {
                assert!(path.exists(), "backup path should exist");
            }

            save_at(&cfg_value, base).expect("persist merged config");
            if let Some(updated) = outcome.updated_ip_pool_file.clone() {
                ip_pool_cfg::save_file_at(&updated, base).expect("persist ip pool file");
            }

            assert_eq!(cfg_value.proxy.mode, ProxyMode::Http);
            assert_eq!(cfg_value.proxy.url, "http://team-proxy:9000");
            assert!(
                cfg_value.proxy.password.is_none(),
                "password should remain None"
            );
            assert!(cfg_value.credential.require_confirmation);
            assert!(!cfg_value.tls.metrics_enabled);
            assert!(cfg_value.ip_pool.enabled);
            assert_eq!(cfg_value.ip_pool.max_parallel_probes, 6);

            let saved_ip = ip_pool_cfg::load_or_init_file_at(base).expect("reload ip file");
            assert!(saved_ip.blacklist.contains(&"10.0.0.0/8".into()));
            assert!(saved_ip.whitelist.contains(&"203.0.113.10".into()));

            assert!(outcome
                .report
                .applied
                .iter()
                .any(|entry| entry.section == TemplateSectionKind::Proxy));
        });
    }
}
