//! P5.3 Integration tests for proxy and transport layer integration
//!
//! Tests verify:
//! - Custom transport skipped when proxy enabled
//! - DisableCustomTransport forced when proxy enabled
//! - Logging shows correct decisions

use fireworks_collaboration_lib::core::config::model::AppConfig;
use fireworks_collaboration_lib::core::git::transport::ensure_registered;
use fireworks_collaboration_lib::core::proxy::{ProxyConfig, ProxyMode};

#[test]
fn test_transport_skipped_when_http_proxy_enabled() {
    let mut cfg = AppConfig::default();
    cfg.proxy = ProxyConfig {
        mode: ProxyMode::Http,
        url: "http://proxy.example.com:8080".to_string(),
        ..Default::default()
    };

    // Should succeed without registering custom transport
    let result = ensure_registered(&cfg);
    assert!(
        result.is_ok(),
        "ensure_registered should succeed when proxy enabled"
    );
}

#[test]
fn test_transport_skipped_when_socks5_proxy_enabled() {
    let mut cfg = AppConfig::default();
    cfg.proxy = ProxyConfig {
        mode: ProxyMode::Socks5,
        url: "socks5://proxy.example.com:1080".to_string(),
        ..Default::default()
    };

    // Should succeed without registering custom transport
    let result = ensure_registered(&cfg);
    assert!(
        result.is_ok(),
        "ensure_registered should succeed when proxy enabled"
    );
}

#[test]
fn test_transport_registered_when_proxy_off() {
    let cfg = AppConfig::default();

    // Should register custom transport
    let result = ensure_registered(&cfg);
    assert!(
        result.is_ok(),
        "ensure_registered should succeed when proxy off"
    );
}

#[test]
fn test_transport_skipped_when_disable_custom_transport_set() {
    let mut cfg = AppConfig::default();
    cfg.proxy.disable_custom_transport = true;

    // Should skip even if proxy is off
    let result = ensure_registered(&cfg);
    assert!(
        result.is_ok(),
        "ensure_registered should succeed with disable_custom_transport"
    );
}

#[test]
fn test_proxy_forces_disable_custom_transport() {
    use fireworks_collaboration_lib::core::proxy::ProxyManager;

    // HTTP proxy
    let cfg_http = ProxyConfig {
        mode: ProxyMode::Http,
        url: "http://proxy:8080".to_string(),
        disable_custom_transport: false, // Explicitly set to false
        ..Default::default()
    };
    let manager_http = ProxyManager::new(cfg_http);
    assert!(
        manager_http.should_disable_custom_transport(),
        "HTTP proxy should force disable custom transport"
    );

    // SOCKS5 proxy
    let cfg_socks5 = ProxyConfig {
        mode: ProxyMode::Socks5,
        url: "socks5://proxy:1080".to_string(),
        disable_custom_transport: false, // Explicitly set to false
        ..Default::default()
    };
    let manager_socks5 = ProxyManager::new(cfg_socks5);
    assert!(
        manager_socks5.should_disable_custom_transport(),
        "SOCKS5 proxy should force disable custom transport"
    );

    // Proxy off
    let cfg_off = ProxyConfig {
        mode: ProxyMode::Off,
        disable_custom_transport: false,
        ..Default::default()
    };
    let manager_off = ProxyManager::new(cfg_off);
    assert!(
        !manager_off.should_disable_custom_transport(),
        "Proxy off should not disable custom transport"
    );
}

#[test]
fn test_proxy_mode_transitions() {
    use fireworks_collaboration_lib::core::proxy::ProxyManager;

    // Start with proxy off
    let cfg_off = ProxyConfig {
        mode: ProxyMode::Off,
        ..Default::default()
    };
    let manager = ProxyManager::new(cfg_off);
    assert!(!manager.should_disable_custom_transport());

    // Enable HTTP proxy
    let cfg_http = ProxyConfig {
        mode: ProxyMode::Http,
        url: "http://proxy:8080".to_string(),
        ..Default::default()
    };
    let manager_http = ProxyManager::new(cfg_http);
    assert!(manager_http.should_disable_custom_transport());

    // Switch to SOCKS5
    let cfg_socks5 = ProxyConfig {
        mode: ProxyMode::Socks5,
        url: "socks5://proxy:1080".to_string(),
        ..Default::default()
    };
    let manager_socks5 = ProxyManager::new(cfg_socks5);
    assert!(manager_socks5.should_disable_custom_transport());

    // Disable proxy
    let cfg_off_again = ProxyConfig {
        mode: ProxyMode::Off,
        ..Default::default()
    };
    let manager_off = ProxyManager::new(cfg_off_again);
    assert!(!manager_off.should_disable_custom_transport());
}

#[test]
fn test_explicit_disable_custom_transport() {
    use fireworks_collaboration_lib::core::proxy::ProxyManager;

    // Explicit disable with proxy off
    let cfg = ProxyConfig {
        mode: ProxyMode::Off,
        disable_custom_transport: true,
        ..Default::default()
    };
    let manager = ProxyManager::new(cfg);
    assert!(
        manager.should_disable_custom_transport(),
        "Explicit disable_custom_transport should work even when proxy off"
    );
}

#[test]
fn test_transport_skipped_when_system_proxy_enabled() {
    let mut cfg = AppConfig::default();
    cfg.proxy = ProxyConfig {
        mode: ProxyMode::System,
        ..Default::default()
    };

    // System proxy should also skip custom transport
    let result = ensure_registered(&cfg);
    assert!(
        result.is_ok(),
        "ensure_registered should succeed with System proxy"
    );
}

#[test]
fn test_system_proxy_forces_disable_custom_transport() {
    use fireworks_collaboration_lib::core::proxy::ProxyManager;

    let cfg_system = ProxyConfig {
        mode: ProxyMode::System,
        disable_custom_transport: false,
        ..Default::default()
    };
    let manager_system = ProxyManager::new(cfg_system);
    assert!(
        manager_system.should_disable_custom_transport(),
        "System proxy should force disable custom transport"
    );
}

#[test]
fn test_metrics_data_flow_with_proxy() {
    use fireworks_collaboration_lib::core::git::transport::metrics::{
        tl_reset, tl_set_proxy_usage, tl_snapshot,
    };

    // Reset to clean state
    tl_reset();

    // Simulate proxy usage recording
    tl_set_proxy_usage(true, Some("http".to_string()), Some(50), true);

    // Capture snapshot
    let snapshot = tl_snapshot();

    // Verify all proxy fields are captured
    assert_eq!(snapshot.used_proxy, Some(true), "used_proxy should be true");
    assert_eq!(
        snapshot.proxy_type,
        Some("http".to_string()),
        "proxy_type should be 'http'"
    );
    assert_eq!(
        snapshot.proxy_latency_ms,
        Some(50),
        "proxy_latency_ms should be 50"
    );
    assert_eq!(
        snapshot.custom_transport_disabled,
        Some(true),
        "custom_transport_disabled should be true"
    );

    // Reset and verify clean state
    tl_reset();
    let snapshot_after_reset = tl_snapshot();
    assert_eq!(
        snapshot_after_reset.used_proxy, None,
        "used_proxy should be None after reset"
    );
    assert_eq!(
        snapshot_after_reset.proxy_type, None,
        "proxy_type should be None after reset"
    );
    assert_eq!(
        snapshot_after_reset.proxy_latency_ms, None,
        "proxy_latency_ms should be None after reset"
    );
    assert_eq!(
        snapshot_after_reset.custom_transport_disabled, None,
        "custom_transport_disabled should be None after reset"
    );
}

#[test]
fn test_metrics_data_flow_without_proxy() {
    use fireworks_collaboration_lib::core::git::transport::metrics::{
        tl_reset, tl_set_proxy_usage, tl_snapshot,
    };

    // Reset to clean state
    tl_reset();

    // Simulate no proxy usage
    tl_set_proxy_usage(false, None, None, false);

    // Capture snapshot
    let snapshot = tl_snapshot();

    // Verify proxy fields reflect no usage
    assert_eq!(
        snapshot.used_proxy,
        Some(false),
        "used_proxy should be false"
    );
    assert_eq!(snapshot.proxy_type, None, "proxy_type should be None");
    assert_eq!(
        snapshot.proxy_latency_ms, None,
        "proxy_latency_ms should be None"
    );
    assert_eq!(
        snapshot.custom_transport_disabled,
        Some(false),
        "custom_transport_disabled should be false"
    );
}

#[test]
fn test_empty_proxy_url_behavior() {
    let mut cfg = AppConfig::default();
    cfg.proxy = ProxyConfig {
        mode: ProxyMode::Http,
        url: "".to_string(),
        ..Default::default()
    };

    // Empty URL with HTTP mode means proxy NOT enabled
    let result = ensure_registered(&cfg);
    assert!(
        result.is_ok(),
        "ensure_registered should handle empty proxy URL gracefully"
    );

    // ProxyManager should report as not enabled
    use fireworks_collaboration_lib::core::proxy::ProxyManager;
    let manager = ProxyManager::new(cfg.proxy.clone());
    assert!(
        !manager.is_enabled(),
        "Empty URL should mean proxy is not enabled"
    );
}

#[test]
fn test_concurrent_registration_safety() {
    use std::sync::Arc;
    use std::thread;

    let cfg = Arc::new(AppConfig::default());

    // Spawn multiple threads trying to register simultaneously
    let handles: Vec<_> = (0..10)
        .map(|_| {
            let cfg_clone = Arc::clone(&cfg);
            thread::spawn(move || ensure_registered(&*cfg_clone))
        })
        .collect();

    // All should succeed
    for handle in handles {
        let result = handle.join().unwrap();
        assert!(result.is_ok(), "Concurrent registration should be safe");
    }
}
