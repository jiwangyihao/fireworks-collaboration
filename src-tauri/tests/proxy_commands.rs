//! Tests for P5.6 proxy commands and events

use fireworks_collaboration_lib::core::proxy::{
    ProxyConfig, ProxyManager, ProxyMode, ProxyStateEvent, SystemProxyDetector,
};

#[test]
fn test_detect_system_proxy_returns_option() {
    // Should not panic regardless of platform
    let result = SystemProxyDetector::detect();

    // Result should be either Some(config) or None
    match result {
        Some(config) => {
            println!("Detected proxy: mode={:?}, url={}", config.mode, config.url);
            assert!(config.mode != ProxyMode::Off || config.url.is_empty());
        }
        None => {
            println!("No system proxy detected");
        }
    }
}

#[test]
fn test_proxy_state_event_basic_creation() {
    use fireworks_collaboration_lib::core::proxy::ProxyState;

    let event = ProxyStateEvent::new(
        ProxyState::Disabled,
        ProxyState::Enabled,
        Some("Test transition".to_string()),
    );

    assert_eq!(event.previous_state, ProxyState::Disabled);
    assert_eq!(event.current_state, ProxyState::Enabled);
    assert_eq!(event.reason, Some("Test transition".to_string()));
    assert_eq!(event.proxy_mode, ProxyMode::Off); // Default
    assert!(!event.custom_transport_disabled); // Default
}

#[test]
fn test_proxy_state_event_extended_creation() {
    use fireworks_collaboration_lib::core::proxy::ProxyState;

    let event = ProxyStateEvent::new_extended(
        ProxyState::Enabled,
        ProxyState::Fallback,
        Some("Failure threshold exceeded".to_string()),
        ProxyMode::Http,
        Some("Connection timeout".to_string()),
        Some(5),
        Some(0.6),
        Some(1234567890),
        Some("http://proxy.example.com".to_string()),
        true,
    );

    assert_eq!(event.previous_state, ProxyState::Enabled);
    assert_eq!(event.current_state, ProxyState::Fallback);
    assert_eq!(event.proxy_mode, ProxyMode::Http);
    assert_eq!(
        event.fallback_reason,
        Some("Connection timeout".to_string())
    );
    assert_eq!(event.failure_count, Some(5));
    assert_eq!(event.health_check_success_rate, Some(0.6));
    assert_eq!(event.next_health_check_at, Some(1234567890));
    assert_eq!(
        event.system_proxy_url,
        Some("http://proxy.example.com".to_string())
    );
    assert!(event.custom_transport_disabled);
}

#[test]
fn test_proxy_config_debug_logging_default() {
    let config = ProxyConfig::default();
    assert!(!config.debug_proxy_logging);
}

#[test]
fn test_proxy_config_debug_logging_enabled() {
    let mut config = ProxyConfig::default();
    config.debug_proxy_logging = true;
    assert!(config.debug_proxy_logging);
}

#[test]
fn test_proxy_manager_force_fallback() {
    let mut config = ProxyConfig::default();
    config.mode = ProxyMode::Http;
    config.url = "http://proxy.example.com:8080".to_string();

    let mut manager = ProxyManager::new(config);

    // Should start in Enabled state
    // (Note: actual state depends on implementation details)

    // Force fallback should succeed
    let result = manager.force_fallback("Test fallback");
    assert!(result.is_ok(), "force_fallback should succeed");
}

#[test]
fn test_proxy_manager_force_recovery() {
    let mut config = ProxyConfig::default();
    config.mode = ProxyMode::Http;
    config.url = "http://proxy.example.com:8080".to_string();

    let mut manager = ProxyManager::new(config);

    // Trigger fallback first
    let _ = manager.force_fallback("Setup");

    // Force recovery should succeed
    let result = manager.force_recovery();
    assert!(result.is_ok(), "force_recovery should succeed");
}

#[test]
fn test_proxy_state_event_serialization() {
    use fireworks_collaboration_lib::core::proxy::ProxyState;

    let event = ProxyStateEvent::new_extended(
        ProxyState::Disabled,
        ProxyState::Enabled,
        Some("Config changed".to_string()),
        ProxyMode::Socks5,
        None,
        None,
        Some(0.95),
        None,
        None,
        false,
    );

    // Should serialize to JSON without errors
    let json = serde_json::to_string(&event);
    assert!(json.is_ok(), "Event should serialize to JSON");

    let json_str = json.unwrap();
    assert!(json_str.contains("previousState"));
    assert!(json_str.contains("currentState"));
    assert!(json_str.contains("proxyMode"));
    assert!(json_str.contains("socks5"));
}

#[test]
fn test_system_proxy_detection_env_fallback() {
    // Test environment variable detection (should not panic)
    let result = SystemProxyDetector::detect_from_env();

    // Result depends on actual environment, just verify it doesn't crash
    match result {
        Some(config) => {
            println!("Detected from env: {:?}", config.url);
        }
        None => {
            println!("No proxy in environment variables");
        }
    }
}

#[test]
fn test_proxy_config_validation_with_debug_logging() {
    let mut config = ProxyConfig::default();
    config.mode = ProxyMode::Http;
    config.url = "http://localhost:8080".to_string();
    config.debug_proxy_logging = true;

    let result = config.validate();
    assert!(
        result.is_ok(),
        "Valid config with debug logging should pass validation"
    );
}

#[test]
fn test_proxy_state_event_all_fields_serialization() {
    use fireworks_collaboration_lib::core::proxy::ProxyState;

    // Create event with all optional fields populated
    let event = ProxyStateEvent::new_extended(
        ProxyState::Enabled,
        ProxyState::Recovering,
        Some("Starting recovery".to_string()),
        ProxyMode::Socks5,
        Some("Previous connection timeout".to_string()),
        Some(8),
        Some(0.45),
        Some(1609459200),
        Some("socks5://proxy.local:1080".to_string()),
        true,
    );

    // Serialize and verify all fields are present
    let json = serde_json::to_value(&event).expect("Should serialize");

    assert!(json["proxyMode"].is_string());
    assert_eq!(json["proxyMode"], "socks5");

    assert!(json["fallbackReason"].is_string());
    assert_eq!(json["fallbackReason"], "Previous connection timeout");

    assert!(json["failureCount"].is_number());
    assert_eq!(json["failureCount"], 8);

    assert!(json["healthCheckSuccessRate"].is_number());
    assert_eq!(json["healthCheckSuccessRate"], 0.45);

    assert!(json["nextHealthCheckAt"].is_number());
    assert_eq!(json["nextHealthCheckAt"], 1609459200);

    assert!(json["systemProxyUrl"].is_string());
    assert_eq!(json["systemProxyUrl"], "socks5://proxy.local:1080");

    assert!(json["customTransportDisabled"].is_boolean());
    assert_eq!(json["customTransportDisabled"], true);
}

#[test]
fn test_proxy_state_event_deserialization() {
    use fireworks_collaboration_lib::core::proxy::ProxyState;

    let json_str = r#"{
        "previousState": "disabled",
        "currentState": "enabled",
        "proxyState": "enabled",
        "reason": "User enabled proxy",
        "timestamp": 1234567890,
        "proxyMode": "http",
        "fallbackReason": null,
        "failureCount": 0,
        "healthCheckSuccessRate": 1.0,
        "nextHealthCheckAt": null,
        "systemProxyUrl": "http://detected.proxy:8080",
        "customTransportDisabled": true
    }"#;

    let event: ProxyStateEvent =
        serde_json::from_str(json_str).expect("Should deserialize valid JSON");

    assert_eq!(event.previous_state, ProxyState::Disabled);
    assert_eq!(event.current_state, ProxyState::Enabled);
    assert_eq!(event.proxy_state, ProxyState::Enabled);
    assert_eq!(event.proxy_mode, ProxyMode::Http);
    assert_eq!(event.fallback_reason, None);
    assert_eq!(event.failure_count, Some(0));
    assert_eq!(event.health_check_success_rate, Some(1.0));
    assert_eq!(event.next_health_check_at, None);
    assert_eq!(
        event.system_proxy_url,
        Some("http://detected.proxy:8080".to_string())
    );
    assert!(event.custom_transport_disabled);
}

#[test]
fn test_proxy_state_event_optional_fields_none() {
    use fireworks_collaboration_lib::core::proxy::ProxyState;

    // Create event with minimal fields (all optionals = None)
    let event = ProxyStateEvent::new_extended(
        ProxyState::Disabled,
        ProxyState::Enabled,
        Some("Config changed".to_string()),
        ProxyMode::Off,
        None, // fallback_reason
        None, // failure_count
        None, // health_check_success_rate
        None, // next_health_check_at
        None, // system_proxy_url
        false,
    );

    // Verify optional fields are None
    assert_eq!(event.fallback_reason, None);
    assert_eq!(event.failure_count, None);
    assert_eq!(event.health_check_success_rate, None);
    assert_eq!(event.next_health_check_at, None);
    assert_eq!(event.system_proxy_url, None);

    // Serialize and check JSON
    let json = serde_json::to_value(&event).expect("Should serialize");

    // Optional None fields should be null in JSON
    assert!(json["fallbackReason"].is_null());
    assert!(json["nextHealthCheckAt"].is_null());
    assert!(json["systemProxyUrl"].is_null());
}

#[test]
fn test_proxy_mode_all_variants_serialization() {
    use fireworks_collaboration_lib::core::proxy::ProxyState;

    let modes = vec![
        (ProxyMode::Off, "off"),
        (ProxyMode::Http, "http"),
        (ProxyMode::Socks5, "socks5"),
        (ProxyMode::System, "system"),
    ];

    for (mode, expected_str) in modes {
        let event = ProxyStateEvent::new_extended(
            ProxyState::Disabled,
            ProxyState::Enabled,
            None,
            mode,
            None,
            None,
            None,
            None,
            None,
            false,
        );

        let json = serde_json::to_value(&event).expect("Should serialize");
        assert_eq!(
            json["proxyMode"], expected_str,
            "Mode {:?} should serialize as {}",
            mode, expected_str
        );
    }
}

// === Additional manager force method tests ===

#[test]
fn test_proxy_manager_force_fallback_when_disabled() {
    // Test fallback on disabled (mode=off) proxy
    let config = ProxyConfig::default(); // mode=Off by default
    let mut manager = ProxyManager::new(config);

    let result = manager.force_fallback("Test fallback on disabled");
    // Should fail because proxy is disabled (mode=off)
    assert!(
        result.is_err(),
        "force_fallback should fail when proxy mode is off"
    );
}

#[test]
fn test_proxy_manager_force_recovery_when_disabled() {
    // Test recovery on disabled proxy
    let config = ProxyConfig::default(); // mode=Off
    let mut manager = ProxyManager::new(config);

    let result = manager.force_recovery();
    // Should fail when proxy is disabled
    assert!(
        result.is_err(),
        "force_recovery should fail when proxy mode is off"
    );
}

#[test]
fn test_proxy_manager_force_fallback_multiple_times() {
    let mut config = ProxyConfig::default();
    config.mode = ProxyMode::Http;
    config.url = "http://proxy.test:8080".to_string();

    let mut manager = ProxyManager::new(config);

    // First fallback should succeed
    assert!(manager.force_fallback("First fallback").is_ok());

    // Second fallback should fail (already in Fallback state)
    assert!(
        manager.force_fallback("Second fallback").is_err(),
        "force_fallback should fail when already in Fallback state"
    );

    // Try recovery to reset state
    let _ = manager.force_recovery();

    // After recovery, fallback should work again
    assert!(manager.force_fallback("Third fallback").is_ok());
}

#[test]
fn test_proxy_manager_force_recovery_multiple_times() {
    let mut config = ProxyConfig::default();
    config.mode = ProxyMode::Http;
    config.url = "http://proxy.test:8080".to_string();

    let mut manager = ProxyManager::new(config);

    // Setup: force fallback first
    let _ = manager.force_fallback("Setup");

    // Force recovery multiple times
    assert!(manager.force_recovery().is_ok());
    // Subsequent recoveries may succeed or fail depending on state
    let _ = manager.force_recovery();
}

#[test]
fn test_proxy_manager_alternating_force_operations() {
    let mut config = ProxyConfig::default();
    config.mode = ProxyMode::Socks5;
    config.url = "socks5://localhost:1080".to_string();

    let mut manager = ProxyManager::new(config);

    // Alternate between fallback and recovery
    assert!(manager.force_fallback("Test 1").is_ok());
    assert!(manager.force_recovery().is_ok());
    assert!(manager.force_fallback("Test 2").is_ok());
    assert!(manager.force_recovery().is_ok());
    assert!(manager.force_fallback("Test 3").is_ok());

    // All operations should succeed
}

#[test]
fn test_proxy_config_with_invalid_url_and_force_fallback() {
    let mut config = ProxyConfig::default();
    config.mode = ProxyMode::Http;
    config.url = "invalid-url-format".to_string();

    // Manager should be created even with invalid URL
    let mut manager = ProxyManager::new(config);

    // force_fallback should not panic
    let result = manager.force_fallback("Invalid URL test");
    // Just verify no panic occurs
    let _ = result;
}

// === System proxy detection edge cases ===

#[test]
fn test_system_proxy_detection_does_not_panic() {
    // Detection should never panic regardless of environment
    let result = SystemProxyDetector::detect();
    // Just verify we get a result (Some or None)
    match result {
        Some(config) => {
            // Verify basic config structure
            assert!(!config.url.is_empty() || config.mode == ProxyMode::Off);
        }
        None => {
            // No proxy detected is valid
        }
    }
}

#[test]
fn test_system_proxy_detection_returns_valid_mode() {
    if let Some(config) = SystemProxyDetector::detect() {
        // If proxy detected, mode should be Http or Socks5, not Off or System
        assert!(
            matches!(config.mode, ProxyMode::Http | ProxyMode::Socks5),
            "Detected proxy mode should be Http or Socks5, got {:?}",
            config.mode
        );
    }
}

#[test]
fn test_system_proxy_detection_url_format() {
    if let Some(config) = SystemProxyDetector::detect() {
        let url = &config.url;
        // URL should not be empty if proxy detected
        assert!(!url.is_empty(), "Detected proxy URL should not be empty");

        // Should contain basic URL components
        if config.mode == ProxyMode::Http {
            assert!(
                url.starts_with("http://") || url.starts_with("https://"),
                "HTTP proxy URL should start with http:// or https://, got: {}",
                url
            );
        } else if config.mode == ProxyMode::Socks5 {
            assert!(
                url.starts_with("socks5://")
                    || url.starts_with("socks://")
                    || !url.starts_with("http"),
                "SOCKS5 proxy URL format should be valid, got: {}",
                url
            );
        }
    }
}

#[test]
fn test_system_proxy_env_variables() {
    // Test environment variable detection
    let result = SystemProxyDetector::detect_from_env();

    // Result depends on environment, just verify no panic
    match result {
        Some(config) => {
            assert!(matches!(config.mode, ProxyMode::Http | ProxyMode::Socks5));
            assert!(!config.url.is_empty());
        }
        None => {
            // No environment proxy is valid
        }
    }
}
