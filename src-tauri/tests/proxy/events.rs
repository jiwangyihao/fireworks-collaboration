//! Tests for proxy events

use fireworks_collaboration_lib::core::proxy::events::{
    ProxyFallbackEvent, ProxyHealthCheckEvent, ProxyRecoveredEvent, ProxyStateEvent,
};
use fireworks_collaboration_lib::core::proxy::state::ProxyState;

fn current_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[test]
fn test_proxy_state_event() {
    let event = ProxyStateEvent::new(
        ProxyState::Disabled,
        ProxyState::Enabled,
        Some("Configuration updated".to_string()),
    );

    assert_eq!(event.previous_state, ProxyState::Disabled);
    assert_eq!(event.current_state, ProxyState::Enabled);
    assert_eq!(event.reason, Some("Configuration updated".to_string()));
    assert!(event.timestamp > 0);
}

#[test]
fn test_proxy_fallback_event_automatic() {
    let event = ProxyFallbackEvent::automatic(
        "Failure rate exceeded threshold".to_string(),
        5,
        300,
        0.5,
        "http://proxy.example.com:8080".to_string(),
    );

    assert_eq!(event.reason, "Failure rate exceeded threshold");
    assert_eq!(event.failure_count, 5);
    assert_eq!(event.window_seconds, 300);
    assert_eq!(event.failure_rate, 0.5);
    assert!(event.is_automatic);
    assert!(event.fallback_at > 0);
}

#[test]
fn test_proxy_fallback_event_manual() {
    let event = ProxyFallbackEvent::manual(
        "User requested fallback".to_string(),
        "http://proxy.example.com:8080".to_string(),
    );

    assert_eq!(event.reason, "User requested fallback");
    assert_eq!(event.failure_count, 0);
    assert_eq!(event.window_seconds, 0);
    assert_eq!(event.failure_rate, 0.0);
    assert!(!event.is_automatic);
    assert!(event.fallback_at > 0);
}

#[test]
fn test_proxy_recovered_event_automatic() {
    let event = ProxyRecoveredEvent::automatic(
        3,
        "http://proxy.example.com:8080".to_string(),
        "exponential-backoff".to_string(),
    );

    assert_eq!(event.successful_checks, 3);
    assert!(event.is_automatic);
    assert_eq!(event.strategy, Some("exponential-backoff".to_string()));
    assert!(event.timestamp > 0);
}

#[test]
fn test_proxy_recovered_event_manual() {
    let event = ProxyRecoveredEvent::manual("http://proxy.example.com:8080".to_string());

    assert_eq!(event.successful_checks, 0);
    assert!(!event.is_automatic);
    assert_eq!(event.strategy, None);
    assert!(event.timestamp > 0);
}

#[test]
fn test_proxy_health_check_event_success() {
    let event = ProxyHealthCheckEvent::success(
        150,
        "http://proxy.example.com:8080".to_string(),
        "https://www.google.com".to_string(),
    );

    assert!(event.success);
    assert_eq!(event.response_time_ms, Some(150));
    assert_eq!(event.error, None);
    assert!(event.timestamp > 0);
}

#[test]
fn test_proxy_health_check_event_failure() {
    let event = ProxyHealthCheckEvent::failure(
        "Connection refused".to_string(),
        "http://proxy.example.com:8080".to_string(),
        "https://www.google.com".to_string(),
    );

    assert!(!event.success);
    assert_eq!(event.response_time_ms, None);
    assert_eq!(event.error, Some("Connection refused".to_string()));
    assert!(event.timestamp > 0);
}

#[test]
fn test_event_serialization() {
    // Test that events can be serialized for frontend
    let state_event = ProxyStateEvent::new(
        ProxyState::Enabled,
        ProxyState::Fallback,
        Some("Too many failures".to_string()),
    );

    let json = serde_json::to_string(&state_event).unwrap();
    // Verify camelCase serialization (previousState, not previous_state)
    assert!(json.contains("previousState"));
    assert!(json.contains("currentState"));
    assert!(!json.contains("previous_state")); // snake_case should not be present

    let fallback_event = ProxyFallbackEvent::automatic(
        "Timeout threshold exceeded".to_string(),
        3,
        300,
        0.3,
        "http://proxy.example.com".to_string(),
    );

    let json = serde_json::to_string(&fallback_event).unwrap();
    assert!(json.contains("failureCount"));
    assert!(json.contains("isAutomatic"));
    assert!(!json.contains("failure_count")); // snake_case should not be present
}

#[test]
fn test_event_timestamps_valid() {
    let state_event = ProxyStateEvent::new(ProxyState::Disabled, ProxyState::Enabled, None);

    // Timestamp should be recent (within last few seconds)
    let now = current_timestamp();
    assert!(state_event.timestamp <= now);
    assert!(state_event.timestamp > now - 10); // Should be very recent
}

#[test]
fn test_all_event_types_serializable() {
    // State event
    let state_event = ProxyStateEvent::new(
        ProxyState::Enabled,
        ProxyState::Fallback,
        Some("Test".to_string()),
    );
    let json = serde_json::to_string(&state_event).unwrap();
    let _restored: ProxyStateEvent = serde_json::from_str(&json).unwrap();

    // Fallback event (automatic)
    let fallback_auto = ProxyFallbackEvent::automatic(
        "Error threshold exceeded".to_string(),
        5,
        300,
        0.5,
        "http://proxy.example.com".to_string(),
    );
    let json = serde_json::to_string(&fallback_auto).unwrap();
    let _restored: ProxyFallbackEvent = serde_json::from_str(&json).unwrap();

    // Fallback event (manual)
    let fallback_manual = ProxyFallbackEvent::manual(
        "User triggered".to_string(),
        "http://proxy.example.com".to_string(),
    );
    let json = serde_json::to_string(&fallback_manual).unwrap();
    let _restored: ProxyFallbackEvent = serde_json::from_str(&json).unwrap();

    // Recovered event (automatic)
    let recovered_auto = ProxyRecoveredEvent::automatic(
        3,
        "http://proxy.example.com".to_string(),
        "consecutive".to_string(),
    );
    let json = serde_json::to_string(&recovered_auto).unwrap();
    let _restored: ProxyRecoveredEvent = serde_json::from_str(&json).unwrap();

    // Recovered event (manual)
    let recovered_manual = ProxyRecoveredEvent::manual("http://proxy.example.com".to_string());
    let json = serde_json::to_string(&recovered_manual).unwrap();
    let _restored: ProxyRecoveredEvent = serde_json::from_str(&json).unwrap();

    // Health check (success)
    let health_success = ProxyHealthCheckEvent::success(
        150,
        "http://proxy.example.com".to_string(),
        "https://example.com".to_string(),
    );
    let json = serde_json::to_string(&health_success).unwrap();
    let _restored: ProxyHealthCheckEvent = serde_json::from_str(&json).unwrap();

    // Health check (failure)
    let health_failure = ProxyHealthCheckEvent::failure(
        "Timeout".to_string(),
        "http://proxy.example.com".to_string(),
        "https://example.com".to_string(),
    );
    let json = serde_json::to_string(&health_failure).unwrap();
    let _restored: ProxyHealthCheckEvent = serde_json::from_str(&json).unwrap();
}

#[test]
fn test_fallback_event_fields() {
    let auto_event = ProxyFallbackEvent::automatic(
        "Connection timeout threshold exceeded".to_string(),
        10,
        300,
        0.4,
        "http://proxy.example.com:8080".to_string(),
    );

    assert_eq!(auto_event.reason, "Connection timeout threshold exceeded");
    assert_eq!(auto_event.failure_count, 10);
    assert_eq!(auto_event.window_seconds, 300);
    assert_eq!(auto_event.failure_rate, 0.4);
    assert_eq!(auto_event.proxy_url, "http://proxy.example.com:8080");
    assert!(auto_event.is_automatic);

    let manual_event = ProxyFallbackEvent::manual(
        "User intervention".to_string(),
        "http://proxy.example.com:8080".to_string(),
    );

    assert_eq!(manual_event.reason, "User intervention");
    assert_eq!(manual_event.failure_count, 0);
    assert_eq!(manual_event.window_seconds, 0);
    assert_eq!(manual_event.failure_rate, 0.0);
    assert!(!manual_event.is_automatic);
}

#[test]
fn test_recovered_event_fields() {
    let auto_event = ProxyRecoveredEvent::automatic(
        5,
        "http://proxy.example.com:8080".to_string(),
        "exponential-backoff".to_string(),
    );

    assert_eq!(auto_event.successful_checks, 5);
    assert!(auto_event.is_automatic);
    assert_eq!(auto_event.strategy, Some("exponential-backoff".to_string()));

    let manual_event = ProxyRecoveredEvent::manual("http://proxy.example.com:8080".to_string());

    assert_eq!(manual_event.successful_checks, 0);
    assert!(!manual_event.is_automatic);
    assert_eq!(manual_event.strategy, None);
}

#[test]
fn test_health_check_event_response_time() {
    let fast = ProxyHealthCheckEvent::success(
        50,
        "http://proxy.example.com".to_string(),
        "https://example.com".to_string(),
    );
    assert_eq!(fast.response_time_ms, Some(50));

    let slow = ProxyHealthCheckEvent::success(
        5000,
        "http://proxy.example.com".to_string(),
        "https://example.com".to_string(),
    );
    assert_eq!(slow.response_time_ms, Some(5000));

    let failed = ProxyHealthCheckEvent::failure(
        "Timeout".to_string(),
        "http://proxy.example.com".to_string(),
        "https://example.com".to_string(),
    );
    assert_eq!(failed.response_time_ms, None);
}

#[test]
fn test_event_clone() {
    let original = ProxyStateEvent::new(
        ProxyState::Enabled,
        ProxyState::Fallback,
        Some("Test".to_string()),
    );

    let cloned = original.clone();

    assert_eq!(cloned.previous_state, original.previous_state);
    assert_eq!(cloned.current_state, original.current_state);
    assert_eq!(cloned.reason, original.reason);
    assert_eq!(cloned.timestamp, original.timestamp);
}

#[test]
fn test_state_event_all_transitions() {
    // Test creating events for all state transitions
    let transitions = vec![
        (ProxyState::Disabled, ProxyState::Enabled),
        (ProxyState::Enabled, ProxyState::Fallback),
        (ProxyState::Fallback, ProxyState::Recovering),
        (ProxyState::Recovering, ProxyState::Enabled),
        (ProxyState::Recovering, ProxyState::Fallback),
        (ProxyState::Enabled, ProxyState::Disabled),
    ];

    for (prev, curr) in transitions {
        let event = ProxyStateEvent::new(prev, curr, Some("Transition test".to_string()));
        assert_eq!(event.previous_state, prev);
        assert_eq!(event.current_state, curr);
    }
}
