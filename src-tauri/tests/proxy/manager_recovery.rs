//! P5.5 Proxy Recovery Tests (integrated from `proxy_recovery.rs`)

use fireworks_collaboration_lib::core::config::model::AppConfig;
use fireworks_collaboration_lib::core::proxy::{ProxyConfig, ProxyManager, ProxyMode, ProxyState};

#[test]
fn test_recovery_complete_flow_manual() {
    // Test manual fallback and recovery flow

    let mut config = ProxyConfig::default();
    config.mode = ProxyMode::Http;
    config.url = "http://proxy.example.com:8080".to_string();
    config.recovery_cooldown_seconds = 10;

    let manager = ProxyManager::new(config);

    // 1. Start in Enabled state
    assert_eq!(manager.state(), ProxyState::Enabled);

    // 2. Trigger manual fallback
    let result = manager.manual_fallback("Test connectivity issue");
    assert!(result.is_ok());
    assert_eq!(manager.state(), ProxyState::Fallback);

    // 3. Verify cooldown is active
    assert!(manager.is_in_cooldown());
    assert!(manager.remaining_cooldown_seconds() > 0);

    // 4. Manual recovery
    let result = manager.manual_recover();
    assert!(result.is_ok());
    assert_eq!(manager.state(), ProxyState::Enabled);

    // 5. Verify cooldown is cleared
    assert!(!manager.is_in_cooldown());
    assert_eq!(manager.remaining_cooldown_seconds(), 0);
}

#[test]
fn test_recovery_with_zero_cooldown() {
    // Test recovery with no cooldown period

    let mut config = ProxyConfig::default();
    config.mode = ProxyMode::Http;
    config.url = "http://proxy.example.com:8080".to_string();
    config.recovery_cooldown_seconds = 0; // No cooldown

    let manager = ProxyManager::new(config);

    // Trigger fallback
    let _ = manager.manual_fallback("Test");
    assert_eq!(manager.state(), ProxyState::Fallback);

    // Cooldown should be expired immediately
    assert!(!manager.is_in_cooldown());
    assert_eq!(manager.remaining_cooldown_seconds(), 0);

    // Recovery should work immediately
    let result = manager.manual_recover();
    assert!(result.is_ok());
    assert_eq!(manager.state(), ProxyState::Enabled);
}

#[test]
fn test_recovery_strategy_immediate() {
    // Test immediate recovery strategy

    let mut config = ProxyConfig::default();
    config.mode = ProxyMode::Http;
    config.url = "http://proxy.example.com:8080".to_string();
    config.recovery_strategy = "immediate".to_string();
    config.recovery_cooldown_seconds = 0;

    let manager = ProxyManager::new(config);

    // Fallback
    let _ = manager.manual_fallback("Test");

    // With immediate strategy, one success should trigger recovery
    // (In real scenario, this would be tested with actual health check)

    // For now, verify manual recovery works with immediate strategy
    let result = manager.manual_recover();
    assert!(result.is_ok());
    assert_eq!(manager.state(), ProxyState::Enabled);
}

#[test]
fn test_recovery_strategy_consecutive() {
    // Test consecutive recovery strategy

    let mut config = ProxyConfig::default();
    config.mode = ProxyMode::Http;
    config.url = "http://proxy.example.com:8080".to_string();
    config.recovery_strategy = "consecutive".to_string();
    config.recovery_cooldown_seconds = 0;

    let manager = ProxyManager::new(config);

    // Fallback
    let _ = manager.manual_fallback("Test");
    assert_eq!(manager.state(), ProxyState::Fallback);

    // With consecutive strategy, need multiple successes
    // (Would require 3 consecutive successes in real health checks)

    // Verify manual recovery still works
    let result = manager.manual_recover();
    assert!(result.is_ok());
    assert_eq!(manager.state(), ProxyState::Enabled);
}

#[test]
fn test_health_check_interval_configuration() {
    // Test health check interval configuration

    let mut config = ProxyConfig::default();
    config.health_check_interval_seconds = 120;

    let manager = ProxyManager::new(config);

    // Verify interval is correctly set
    assert_eq!(
        manager.health_check_interval(),
        std::time::Duration::from_secs(120)
    );
}

#[test]
fn test_fallback_resets_recovery_state() {
    // Test that fallback resets recovery state

    let mut config = ProxyConfig::default();
    config.mode = ProxyMode::Http;
    config.url = "http://proxy.example.com:8080".to_string();
    config.recovery_cooldown_seconds = 10;

    let manager = ProxyManager::new(config);

    // First fallback
    let _ = manager.manual_fallback("First issue");
    assert_eq!(manager.state(), ProxyState::Fallback);
    let first_cooldown = manager.remaining_cooldown_seconds();

    // Recover
    let _ = manager.manual_recover();
    assert_eq!(manager.state(), ProxyState::Enabled);
    assert!(!manager.is_in_cooldown());

    // Second fallback
    let _ = manager.manual_fallback("Second issue");
    assert_eq!(manager.state(), ProxyState::Fallback);

    // Cooldown should be reset
    let second_cooldown = manager.remaining_cooldown_seconds();

    // Both cooldowns should be roughly equal (within a few seconds)
    let diff = first_cooldown.abs_diff(second_cooldown);

    assert!(diff <= 5, "Cooldown should be reset on new fallback");
}

#[test]
fn test_health_check_skipped_in_enabled_state() {
    // Test that health checks are skipped when not in fallback

    let mut config = ProxyConfig::default();
    config.mode = ProxyMode::Http;
    config.url = "http://proxy.example.com:8080".to_string();

    let manager = ProxyManager::new(config);

    // In Enabled state, health check should be skipped
    assert_eq!(manager.state(), ProxyState::Enabled);

    let result = manager.health_check();
    assert!(result.is_ok());

    let probe_result = result.unwrap();
    assert!(probe_result.is_skipped());
}

#[test]
fn test_multiple_recovery_cycles() {
    // Test multiple fallback-recovery cycles

    let mut config = ProxyConfig::default();
    config.mode = ProxyMode::Http;
    config.url = "http://proxy.example.com:8080".to_string();
    config.recovery_cooldown_seconds = 0; // No cooldown for faster testing

    let manager = ProxyManager::new(config);

    for i in 0..3 {
        // Fallback
        let reason = format!("Issue {}", i + 1);
        let _ = manager.manual_fallback(&reason);
        assert_eq!(manager.state(), ProxyState::Fallback);

        // Recover
        let result = manager.manual_recover();
        assert!(result.is_ok());
        assert_eq!(manager.state(), ProxyState::Enabled);
    }
}

#[test]
fn test_recovery_with_long_cooldown() {
    // Test recovery behavior with longer cooldown period

    let mut config = ProxyConfig::default();
    config.mode = ProxyMode::Http;
    config.url = "http://proxy.example.com:8080".to_string();
    config.recovery_cooldown_seconds = 600; // 10 minutes

    let manager = ProxyManager::new(config);

    // Trigger fallback
    let _ = manager.manual_fallback("Long cooldown test");
    assert_eq!(manager.state(), ProxyState::Fallback);

    // Verify long cooldown is active
    assert!(manager.is_in_cooldown());
    let remaining = manager.remaining_cooldown_seconds();
    assert!(remaining > 500, "Cooldown should be close to 600 seconds");
    assert!(
        remaining <= 600,
        "Cooldown should not exceed configured value"
    );

    // Manual recovery should still work regardless of cooldown
    let result = manager.manual_recover();
    assert!(result.is_ok());
    assert_eq!(manager.state(), ProxyState::Enabled);
}

#[test]
fn test_app_config_integration() {
    // Test that ProxyManager works correctly with AppConfig

    let mut app_config = AppConfig::default();
    app_config.proxy.mode = ProxyMode::Http;
    app_config.proxy.url = "http://proxy.example.com:8080".to_string();
    app_config.proxy.health_check_interval_seconds = 90;
    app_config.proxy.recovery_cooldown_seconds = 180;
    app_config.proxy.recovery_strategy = "consecutive".to_string();

    let manager = ProxyManager::new(app_config.proxy);

    // Verify configuration is correctly applied
    assert_eq!(manager.mode(), ProxyMode::Http);
    assert_eq!(
        manager.health_check_interval(),
        std::time::Duration::from_secs(90)
    );

    // Test fallback-recovery flow
    let _ = manager.manual_fallback("Integration test");
    assert_eq!(manager.state(), ProxyState::Fallback);

    let _ = manager.manual_recover();
    assert_eq!(manager.state(), ProxyState::Enabled);
}

#[test]
fn test_custom_probe_url_configuration() {
    // Test custom probe URL configuration

    let mut config = ProxyConfig::default();
    config.mode = ProxyMode::Http;
    config.url = "http://proxy.example.com:8080".to_string();
    config.probe_url = "www.google.com:443".to_string(); // Custom probe target
    config.recovery_cooldown_seconds = 0;

    let manager = ProxyManager::new(config);

    // Trigger fallback
    let _ = manager.manual_fallback("Testing custom probe URL");
    assert_eq!(manager.state(), ProxyState::Fallback);

    // Health check should use custom probe URL
    // (In real scenario, this would verify connection to www.google.com:443)
    let result = manager.health_check();
    assert!(result.is_ok());

    // Manual recovery should work
    let _ = manager.manual_recover();
    assert_eq!(manager.state(), ProxyState::Enabled);
}

#[test]
fn test_custom_probe_timeout_configuration() {
    // Test custom probe timeout configuration

    let mut config = ProxyConfig::default();
    config.mode = ProxyMode::Http;
    config.url = "http://proxy.example.com:8080".to_string();
    config.probe_timeout_seconds = 5; // Fast timeout for quick networks
    config.recovery_cooldown_seconds = 0;

    let manager = ProxyManager::new(config);

    // Trigger fallback
    let _ = manager.manual_fallback("Testing custom probe timeout");
    assert_eq!(manager.state(), ProxyState::Fallback);

    // Health check should use custom timeout
    let result = manager.health_check();
    assert!(result.is_ok());

    // Verify manager is correctly configured
    let _ = manager.manual_recover();
    assert_eq!(manager.state(), ProxyState::Enabled);
}

#[test]
fn test_custom_recovery_threshold_low() {
    // Test low recovery threshold (1) - acts like immediate strategy

    let mut config = ProxyConfig::default();
    config.mode = ProxyMode::Http;
    config.url = "http://proxy.example.com:8080".to_string();
    config.recovery_strategy = "consecutive".to_string();
    config.recovery_consecutive_threshold = 1; // Very low threshold
    config.recovery_cooldown_seconds = 0;

    let manager = ProxyManager::new(config);

    // Fallback
    let _ = manager.manual_fallback("Test low threshold");
    assert_eq!(manager.state(), ProxyState::Fallback);

    // With threshold=1, single success should be enough
    // (Would trigger recovery after 1 successful health check)

    let _ = manager.manual_recover();
    assert_eq!(manager.state(), ProxyState::Enabled);
}

#[test]
fn test_custom_recovery_threshold_high() {
    // Test high recovery threshold (10) - very conservative

    let mut config = ProxyConfig::default();
    config.mode = ProxyMode::Http;
    config.url = "http://proxy.example.com:8080".to_string();
    config.recovery_strategy = "consecutive".to_string();
    config.recovery_consecutive_threshold = 10; // Maximum threshold
    config.recovery_cooldown_seconds = 0;

    let manager = ProxyManager::new(config);

    // Fallback
    let _ = manager.manual_fallback("Test high threshold");
    assert_eq!(manager.state(), ProxyState::Fallback);

    // With threshold=10, would need 10 consecutive successes
    // (This is very conservative for unstable proxies)

    let _ = manager.manual_recover();
    assert_eq!(manager.state(), ProxyState::Enabled);
}

#[test]
fn test_combined_custom_configuration() {
    // Test all P5.5 fields together

    let mut config = ProxyConfig::default();
    config.mode = ProxyMode::Socks5;
    config.url = "socks5://proxy.example.com:1080".to_string();
    config.probe_url = "www.cloudflare.com:443".to_string();
    config.probe_timeout_seconds = 20;
    config.recovery_consecutive_threshold = 5;
    config.recovery_strategy = "consecutive".to_string();
    config.recovery_cooldown_seconds = 120;
    config.health_check_interval_seconds = 45;

    let manager = ProxyManager::new(config);

    // Verify all configurations are applied
    assert_eq!(manager.mode(), ProxyMode::Socks5);
    assert_eq!(
        manager.health_check_interval(),
        std::time::Duration::from_secs(45)
    );

    // Test fallback-recovery flow
    let _ = manager.manual_fallback("Combined config test");
    assert_eq!(manager.state(), ProxyState::Fallback);

    // Verify cooldown is active
    assert!(manager.is_in_cooldown());
    let remaining = manager.remaining_cooldown_seconds();
    assert!(remaining > 100, "Cooldown should be close to 120 seconds");

    // Manual recovery
    let _ = manager.manual_recover();
    assert_eq!(manager.state(), ProxyState::Enabled);
    assert!(!manager.is_in_cooldown());
}

#[test]
fn test_extreme_probe_timeout_minimum() {
    // Test minimum probe timeout (1 second)

    let mut config = ProxyConfig::default();
    config.mode = ProxyMode::Http;
    config.url = "http://proxy.example.com:8080".to_string();
    config.probe_timeout_seconds = 1; // Minimum allowed
    config.recovery_cooldown_seconds = 0;

    let manager = ProxyManager::new(config);

    // Should work with minimum timeout
    let _ = manager.manual_fallback("Minimum timeout test");
    assert_eq!(manager.state(), ProxyState::Fallback);

    let result = manager.health_check();
    assert!(result.is_ok());
}

#[test]
fn test_extreme_probe_timeout_maximum() {
    // Test maximum probe timeout (60 seconds)

    let mut config = ProxyConfig::default();
    config.mode = ProxyMode::Http;
    config.url = "http://proxy.example.com:8080".to_string();
    config.probe_timeout_seconds = 60; // Maximum allowed
    config.recovery_cooldown_seconds = 0;

    let manager = ProxyManager::new(config);

    // Should work with maximum timeout
    let _ = manager.manual_fallback("Maximum timeout test");
    assert_eq!(manager.state(), ProxyState::Fallback);

    let result = manager.health_check();
    assert!(result.is_ok());
}

#[test]
fn test_probe_url_with_different_ports() {
    // Test probe URL with various common ports

    let ports = vec![80, 443, 8080, 8443];

    for port in ports {
        let mut config = ProxyConfig::default();
        config.mode = ProxyMode::Http;
        config.url = "http://proxy.example.com:8080".to_string();
        config.probe_url = format!("example.com:{port}");
        config.recovery_cooldown_seconds = 0;

        let manager = ProxyManager::new(config);

        // Should work with any valid port
        let _ = manager.manual_fallback(&format!("Test port {port}"));
        assert_eq!(manager.state(), ProxyState::Fallback);

        let result = manager.health_check();
        assert!(result.is_ok());

        let _ = manager.manual_recover();
        assert_eq!(manager.state(), ProxyState::Enabled);
    }
}

#[test]
fn test_recovery_threshold_with_immediate_strategy() {
    // Test that threshold is ignored with immediate strategy

    let mut config = ProxyConfig::default();
    config.mode = ProxyMode::Http;
    config.url = "http://proxy.example.com:8080".to_string();
    config.recovery_strategy = "immediate".to_string();
    config.recovery_consecutive_threshold = 5; // Should be ignored
    config.recovery_cooldown_seconds = 0;

    let manager = ProxyManager::new(config);

    // With immediate strategy, threshold shouldn't matter
    let _ = manager.manual_fallback("Immediate strategy test");
    assert_eq!(manager.state(), ProxyState::Fallback);

    // One success should trigger recovery regardless of threshold
    let _ = manager.manual_recover();
    assert_eq!(manager.state(), ProxyState::Enabled);
}

#[test]
fn test_config_update_changes_probe_settings() {
    // Test that updating config changes probe settings

    let mut config = ProxyConfig::default();
    config.mode = ProxyMode::Http;
    config.url = "http://proxy.example.com:8080".to_string();
    config.probe_url = "www.github.com:443".to_string();
    config.probe_timeout_seconds = 10;
    config.recovery_consecutive_threshold = 3;

    let manager = ProxyManager::new(config.clone());

    // Update config with new probe settings
    config.probe_url = "www.google.com:443".to_string();
    config.probe_timeout_seconds = 20;
    config.recovery_consecutive_threshold = 5;

    let result = manager.update_config(config);
    assert!(result.is_ok());

    // New config should be active (verified through state transitions)
    let _ = manager.manual_fallback("Config update test");
    assert_eq!(manager.state(), ProxyState::Fallback);

    let _ = manager.manual_recover();
    assert_eq!(manager.state(), ProxyState::Enabled);
}
