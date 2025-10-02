// 从 src/core/proxy/config.rs 迁移的测试
use fireworks_collaboration_lib::core::proxy::{ProxyConfig, ProxyMode};
use fireworks_collaboration_lib::core::proxy::config::{
    default_timeout_seconds, default_fallback_threshold, default_fallback_window_seconds,
    default_recovery_cooldown_seconds, default_health_check_interval_seconds, default_recovery_strategy,
};

#[test]
fn test_proxy_mode_default() {
    assert_eq!(ProxyMode::default(), ProxyMode::Off);
}

#[test]
fn test_proxy_mode_display() {
    assert_eq!(ProxyMode::Off.to_string(), "off");
    assert_eq!(ProxyMode::Http.to_string(), "http");
    assert_eq!(ProxyMode::Socks5.to_string(), "socks5");
    assert_eq!(ProxyMode::System.to_string(), "system");
}

#[test]
fn test_proxy_mode_serialization() {
    let json = serde_json::to_string(&ProxyMode::Http).unwrap();
    assert_eq!(json, "\"http\"");
    
    let mode: ProxyMode = serde_json::from_str("\"socks5\"").unwrap();
    assert_eq!(mode, ProxyMode::Socks5);
}

#[test]
fn test_proxy_config_default() {
    let config = ProxyConfig::default();
    assert_eq!(config.mode, ProxyMode::Off);
    assert_eq!(config.url, "");
    assert_eq!(config.timeout_seconds, 30);
    assert_eq!(config.fallback_threshold, 0.2);
    assert!(!config.is_enabled());
}

#[test]
fn test_proxy_config_validation() {
    let mut config = ProxyConfig::default();
    
    assert!(config.validate().is_ok());
    
    config.mode = ProxyMode::Http;
    assert!(config.validate().is_err());
    
    config.url = "http://proxy.example.com:8080".to_string();
    assert!(config.validate().is_ok());
    
    config.fallback_threshold = 1.5;
    assert!(config.validate().is_err());
    
    config.fallback_threshold = 0.2;
    assert!(config.validate().is_ok());
}

#[test]
fn test_proxy_config_sanitized_url() {
    let mut config = ProxyConfig::default();
    
    assert_eq!(config.sanitized_url(), "");
    
    config.url = "http://proxy.example.com:8080".to_string();
    assert_eq!(config.sanitized_url(), "http://proxy.example.com:8080");
    
    config.url = "http://user:pass@proxy.example.com:8080".to_string();
    assert_eq!(config.sanitized_url(), "http://***@proxy.example.com:8080");
}

#[test]
fn test_proxy_config_serialization() {
    let config = ProxyConfig {
        mode: ProxyMode::Http,
        url: "http://proxy.example.com:8080".to_string(),
        username: Some("user".to_string()),
        password: Some("pass".to_string()),
        disable_custom_transport: true,
        ..Default::default()
    };
    
    let json = serde_json::to_string(&config).unwrap();
    let deserialized: ProxyConfig = serde_json::from_str(&json).unwrap();
    
    assert_eq!(deserialized.mode, ProxyMode::Http);
    assert_eq!(deserialized.url, "http://proxy.example.com:8080");
    assert_eq!(deserialized.username, Some("user".to_string()));
    assert!(deserialized.disable_custom_transport);
}

#[test]
fn test_proxy_config_is_enabled() {
    let mut config = ProxyConfig::default();
    assert!(!config.is_enabled());

    config.mode = ProxyMode::Http;
    assert!(!config.is_enabled());
    config.url = "http://proxy.example.com:8080".to_string();
    assert!(config.is_enabled());

    config.mode = ProxyMode::Socks5;
    assert!(config.is_enabled());

    config.mode = ProxyMode::System;
    config.url = "".to_string();
    assert!(config.is_enabled());
}

#[test]
fn test_validate_port() {
    let mut config = ProxyConfig {
        mode: ProxyMode::Http,
        url: "http://proxy.example.com:8080".to_string(),
        ..Default::default()
    };
    assert!(config.validate().is_ok());
    
    config.url = "http://proxy.example.com:0".to_string();
    assert!(config.validate().is_err());
    
    config.url = "http://proxy.example.com:abc".to_string();
    assert!(config.validate().is_err());
    
    config.url = "http://proxy.example.com:65535".to_string();
    assert!(config.validate().is_ok());
}

#[test]
fn test_validate_timeout() {
    let mut config = ProxyConfig {
        mode: ProxyMode::Http,
        url: "http://proxy.example.com:8080".to_string(),
        ..Default::default()
    };
    
    config.timeout_seconds = 30;
    assert!(config.validate().is_ok());
    
    config.timeout_seconds = 0;
    assert!(config.validate().is_err());
    
    config.timeout_seconds = 400;
    assert!(config.validate().is_err());
    
    config.timeout_seconds = 300;
    assert!(config.validate().is_ok());
}

#[test]
fn test_validate_fallback_window() {
    let mut config = ProxyConfig {
        mode: ProxyMode::Http,
        url: "http://proxy.example.com:8080".to_string(),
        ..Default::default()
    };
    
    config.fallback_window_seconds = 5;
    assert!(config.validate().is_err());
    
    config.fallback_window_seconds = 4000;
    assert!(config.validate().is_err());
    
    config.fallback_window_seconds = 60;
    assert!(config.validate().is_ok());
}

#[test]
fn test_validate_recovery_cooldown() {
    let mut config = ProxyConfig {
        mode: ProxyMode::Http,
        url: "http://proxy.example.com:8080".to_string(),
        ..Default::default()
    };
    
    config.recovery_cooldown_seconds = 5;
    assert!(config.validate().is_err());
    
    config.recovery_cooldown_seconds = 4000;
    assert!(config.validate().is_err());
    
    config.recovery_cooldown_seconds = 300;
    assert!(config.validate().is_ok());
}

#[test]
fn test_validate_health_check_interval() {
    let mut config = ProxyConfig {
        mode: ProxyMode::Http,
        url: "http://proxy.example.com:8080".to_string(),
        ..Default::default()
    };
    
    config.health_check_interval_seconds = 5;
    assert!(config.validate().is_err());
    
    config.health_check_interval_seconds = 4000;
    assert!(config.validate().is_err());
    
    config.health_check_interval_seconds = 60;
    assert!(config.validate().is_ok());
}

#[test]
fn test_validate_recovery_strategy() {
    let mut config = ProxyConfig {
        mode: ProxyMode::Http,
        url: "http://proxy.example.com:8080".to_string(),
        ..Default::default()
    };
    
    config.recovery_strategy = "immediate".to_string();
    assert!(config.validate().is_ok());
    
    config.recovery_strategy = "consecutive".to_string();
    assert!(config.validate().is_ok());
    
    config.recovery_strategy = "exponential-backoff".to_string();
    assert!(config.validate().is_ok());
    
    config.recovery_strategy = "invalid-strategy".to_string();
    assert!(config.validate().is_err());
}

#[test]
fn test_validate_url_format() {
    let mut config = ProxyConfig {
        mode: ProxyMode::Http,
        ..Default::default()
    };
    
    config.url = "http://proxy.example.com :8080".to_string();
    assert!(config.validate().is_err());
    
    config.url = "ftp://proxy.example.com:8080".to_string();
    assert!(config.validate().is_err());
    
    config.url = "http://proxy.example.com:8080".to_string();
    assert!(config.validate().is_ok());
    
    config.url = "https://proxy.example.com:8080".to_string();
    assert!(config.validate().is_ok());
    
    config.url = "socks5://proxy.example.com:1080".to_string();
    assert!(config.validate().is_ok());
}

#[test]
fn test_config_json_roundtrip() {
    let original = ProxyConfig {
        mode: ProxyMode::Socks5,
        url: "socks5://user:pass@proxy.example.com:1080".to_string(),
        username: Some("override_user".to_string()),
        password: Some("override_pass".to_string()),
        disable_custom_transport: true,
        timeout_seconds: 60,
        fallback_threshold: 0.3,
        fallback_window_seconds: 120,
        recovery_cooldown_seconds: 600,
        health_check_interval_seconds: 90,
        recovery_strategy: "exponential-backoff".to_string(),
        probe_url: "www.google.com:443".to_string(),
        probe_timeout_seconds: 20,
        recovery_consecutive_threshold: 5,
        debug_proxy_logging: true,
    };
    
    let json = serde_json::to_string(&original).unwrap();
    let restored: ProxyConfig = serde_json::from_str(&json).unwrap();
    
    assert_eq!(restored.mode, original.mode);
    assert_eq!(restored.url, original.url);
    assert_eq!(restored.username, original.username);
    assert_eq!(restored.password, original.password);
    assert_eq!(restored.disable_custom_transport, original.disable_custom_transport);
    assert_eq!(restored.timeout_seconds, original.timeout_seconds);
    assert_eq!(restored.fallback_threshold, original.fallback_threshold);
    assert_eq!(restored.fallback_window_seconds, original.fallback_window_seconds);
    assert_eq!(restored.recovery_cooldown_seconds, original.recovery_cooldown_seconds);
    assert_eq!(restored.health_check_interval_seconds, original.health_check_interval_seconds);
    assert_eq!(restored.recovery_strategy, original.recovery_strategy);
    assert_eq!(restored.probe_url, original.probe_url);
    assert_eq!(restored.probe_timeout_seconds, original.probe_timeout_seconds);
    assert_eq!(restored.recovery_consecutive_threshold, original.recovery_consecutive_threshold);
}

#[test]
fn test_sanitized_url_edge_cases() {
    let mut config = ProxyConfig::default();
    
    assert_eq!(config.sanitized_url(), "");
    
    config.url = "http://user@proxy.example.com:8080".to_string();
    assert_eq!(config.sanitized_url(), "http://***@proxy.example.com:8080");
    
    config.url = "http://user:p@ss:w0rd@proxy.example.com:8080".to_string();
    let sanitized = config.sanitized_url();
    assert!(sanitized.starts_with("http://***@"));
    assert!(sanitized.contains("proxy.example.com"));
    
    config.url = "http://user:pass@proxy.example.com:8080/path".to_string();
    assert_eq!(config.sanitized_url(), "http://***@proxy.example.com:8080/path");
    
    config.url = "http://proxy.example.com:8080".to_string();
    assert_eq!(config.sanitized_url(), "http://proxy.example.com:8080");
}

#[test]
fn test_credential_fields_combination() {
    let mut config = ProxyConfig {
        mode: ProxyMode::Http,
        url: "http://user:pass@proxy.example.com:8080".to_string(),
        ..Default::default()
    };
    assert!(config.validate().is_ok());
    
    config.username = Some("new_user".to_string());
    assert!(config.validate().is_ok());
    
    config.password = Some("new_pass".to_string());
    assert!(config.validate().is_ok());
    
    config.url = "http://proxy.example.com:8080".to_string();
    config.username = None;
    config.password = Some("only_pass".to_string());
    assert!(config.validate().is_ok());
}

#[test]
fn test_url_with_ip_address() {
    let mut config = ProxyConfig {
        mode: ProxyMode::Http,
        ..Default::default()
    };
    
    config.url = "http://192.168.1.1:8080".to_string();
    assert!(config.validate().is_ok());
    
    config.url = "http://user:pass@10.0.0.1:3128".to_string();
    assert!(config.validate().is_ok());
    assert!(config.sanitized_url().contains("***"));
    
    config.url = "http://[::1]:8080".to_string();
    assert!(config.validate().is_ok());
    
    config.url = "http://localhost:8080".to_string();
    assert!(config.validate().is_ok());
}

#[test]
fn test_system_mode_validation() {
    let mut config = ProxyConfig {
        mode: ProxyMode::System,
        ..Default::default()
    };
    
    assert!(config.validate().is_ok());
    
    config.timeout_seconds = 0;
    assert!(config.validate().is_err());
    
    config.timeout_seconds = 30;
    config.fallback_threshold = 1.5;
    assert!(config.validate().is_err());
}

#[test]
fn test_timeout_duration_conversion() {
    let config = ProxyConfig {
        timeout_seconds: 45,
        ..Default::default()
    };
    
    let duration = config.timeout();
    assert_eq!(duration.as_secs(), 45);
}

#[test]
fn test_default_values_completeness() {
    let config = ProxyConfig::default();
    
    assert_eq!(config.mode, ProxyMode::Off);
    assert_eq!(config.url, "");
    assert_eq!(config.username, None);
    assert_eq!(config.password, None);
    assert_eq!(config.disable_custom_transport, false);
    assert_eq!(config.timeout_seconds, default_timeout_seconds());
    assert_eq!(config.fallback_threshold, default_fallback_threshold());
    assert_eq!(config.fallback_window_seconds, default_fallback_window_seconds());
    assert_eq!(config.recovery_cooldown_seconds, default_recovery_cooldown_seconds());
    assert_eq!(config.health_check_interval_seconds, default_health_check_interval_seconds());
    assert_eq!(config.recovery_strategy, default_recovery_strategy());
    
    assert!(config.validate().is_ok());
}

#[test]
fn test_camel_case_serialization() {
    let config = ProxyConfig {
        mode: ProxyMode::Http,
        url: "http://proxy.example.com:8080".to_string(),
        disable_custom_transport: true,
        timeout_seconds: 60,
        fallback_threshold: 0.3,
        fallback_window_seconds: 120,
        recovery_cooldown_seconds: 600,
        health_check_interval_seconds: 90,
        recovery_strategy: "immediate".to_string(),
        ..Default::default()
    };
    
    let json = serde_json::to_string(&config).unwrap();
    
    assert!(json.contains("disableCustomTransport"));
    assert!(json.contains("timeoutSeconds"));
    assert!(json.contains("fallbackThreshold"));
    assert!(json.contains("fallbackWindowSeconds"));
    assert!(json.contains("recoveryCooldownSeconds"));
    assert!(json.contains("healthCheckIntervalSeconds"));
    assert!(json.contains("recoveryStrategy"));
    
    assert!(!json.contains("disable_custom_transport"));
    assert!(!json.contains("timeout_seconds"));
}

// ============================================================================
// mod_tests.rs 的测试
// ============================================================================

use fireworks_collaboration_lib::core::proxy::PlaceholderConnector;
use fireworks_collaboration_lib::core::proxy::ProxyConnector;

#[test]
fn test_placeholder_connector() {
    let connector = PlaceholderConnector;
    assert_eq!(connector.proxy_type(), "placeholder");
    
    // Test connecting to a well-known host (will fail in CI but tests the interface)
    // This is just to verify the trait implementation compiles
    let _ = connector.connect("example.com", 80);
}

// ============================================================================
// errors_tests.rs 的测试
// ============================================================================

use fireworks_collaboration_lib::core::proxy::errors::ProxyError;

#[test]
fn test_proxy_error_display() {
    let error = ProxyError::network("Connection refused");
    assert_eq!(error.to_string(), "Network error: Connection refused");

    let error = ProxyError::auth("Invalid credentials");
    assert_eq!(error.to_string(), "Authentication error: Invalid credentials");

    let error = ProxyError::proxy("Bad gateway");
    assert_eq!(error.to_string(), "Proxy error: Bad gateway");

    let error = ProxyError::timeout("Connection timeout");
    assert_eq!(error.to_string(), "Timeout error: Connection timeout");

    let error = ProxyError::config("Invalid URL");
    assert_eq!(error.to_string(), "Configuration error: Invalid URL");
}

#[test]
fn test_proxy_error_category() {
    assert_eq!(ProxyError::network("test").category(), "network");
    assert_eq!(ProxyError::auth("test").category(), "auth");
    assert_eq!(ProxyError::proxy("test").category(), "proxy");
    assert_eq!(ProxyError::timeout("test").category(), "timeout");
    assert_eq!(ProxyError::config("test").category(), "config");
}

#[test]
fn test_proxy_error_equality() {
    let error1 = ProxyError::network("test");
    let error2 = ProxyError::network("test");
    let error3 = ProxyError::network("other");

    assert_eq!(error1, error2);
    assert_ne!(error1, error3);
}

// ============================================================================
// system_detector_tests.rs 的测试
// ============================================================================

use fireworks_collaboration_lib::core::proxy::system_detector::SystemProxyDetector;

#[test]
fn test_parse_proxy_url_http() {
    let config = SystemProxyDetector::parse_proxy_url("http://proxy.example.com:8080");
    assert!(config.is_some());
    let config = config.unwrap();
    assert_eq!(config.mode, ProxyMode::Http);
    assert_eq!(config.url, "http://proxy.example.com:8080");
}

#[test]
fn test_parse_proxy_url_https() {
    let config = SystemProxyDetector::parse_proxy_url("https://proxy.example.com:8443");
    assert!(config.is_some());
    let config = config.unwrap();
    assert_eq!(config.mode, ProxyMode::Http);
    assert_eq!(config.url, "https://proxy.example.com:8443");
}

#[test]
fn test_parse_proxy_url_socks5() {
    let config = SystemProxyDetector::parse_proxy_url("socks5://127.0.0.1:1080");
    assert!(config.is_some());
    let config = config.unwrap();
    assert_eq!(config.mode, ProxyMode::Socks5);
    assert_eq!(config.url, "socks5://127.0.0.1:1080");
}

#[test]
fn test_parse_proxy_url_no_scheme() {
    // Should auto-add http:// scheme
    let config = SystemProxyDetector::parse_proxy_url("proxy.example.com:8080");
    assert!(config.is_some());
    let config = config.unwrap();
    assert_eq!(config.mode, ProxyMode::Http);
    assert_eq!(config.url, "http://proxy.example.com:8080");
}

#[test]
fn test_parse_proxy_url_invalid() {
    // Empty host
    let config = SystemProxyDetector::parse_proxy_url("http://");
    assert!(config.is_none());
    
    // Invalid format
    let config = SystemProxyDetector::parse_proxy_url("not-a-url");
    assert!(config.is_some()); // Will be parsed as "http://not-a-url"
}

#[test]
fn test_detect_from_env() {
    // This test depends on actual environment variables
    // Just verify it doesn't panic
    let _ = SystemProxyDetector::detect_from_env();
}

#[test]
fn test_detect() {
    // This test depends on actual system configuration
    // Just verify it doesn't panic and returns a valid option
    let result = SystemProxyDetector::detect();
    if let Some(config) = result {
        // If detected, validate it
        assert!(config.validate().is_ok());
    }
}

#[cfg(target_os = "macos")]
#[test]
fn test_parse_scutil_output() {
    let lines = vec![
        "  HTTPEnable : 1",
        "  HTTPProxy : proxy.example.com",
        "  HTTPPort : 8080",
    ];
    
    let config = SystemProxyDetector::parse_scutil_output(&lines, "HTTP");
    assert!(config.is_some());
    let config = config.unwrap();
    assert_eq!(config.mode, ProxyMode::Http);
    assert!(config.url.contains("proxy.example.com"));
    assert!(config.url.contains("8080"));
}

#[cfg(target_os = "macos")]
#[test]
fn test_parse_scutil_output_disabled() {
    let lines = vec![
        "  HTTPEnable : 0",
        "  HTTPProxy : proxy.example.com",
        "  HTTPPort : 8080",
    ];
    
    let config = SystemProxyDetector::parse_scutil_output(&lines, "HTTP");
    assert!(config.is_none());
}
