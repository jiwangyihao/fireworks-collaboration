//! Proxy event structures for frontend integration
//!
//! These events will be emitted to the frontend in P5.6 for real-time
//! monitoring of proxy state changes, fallback events, and health checks.

use super::state::ProxyState;
use serde::{Deserialize, Serialize};

/// Proxy state change event
/// 
/// Emitted whenever the proxy state transitions between:
/// Disabled ↔ Enabled ↔ Fallback ↔ Recovering
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyStateEvent {
    /// Previous state
    pub previous_state: ProxyState,
    
    /// New current state
    pub current_state: ProxyState,
    
    /// Reason for the state change
    pub reason: Option<String>,
    
    /// Timestamp of the transition (Unix epoch seconds)
    pub timestamp: u64,
}

impl ProxyStateEvent {
    /// Create a new proxy state event
    pub fn new(
        previous_state: ProxyState,
        current_state: ProxyState,
        reason: Option<String>,
    ) -> Self {
        Self {
            previous_state,
            current_state,
            reason,
            timestamp: current_timestamp(),
        }
    }
}

/// Proxy fallback event
/// 
/// Emitted when proxy falls back to direct connection due to failures.
/// This is triggered by P5.4 automatic fallback logic.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyFallbackEvent {
    /// Reason for fallback (e.g., "Failure rate exceeded threshold")
    pub reason: String,
    
    /// Total number of failures in the sliding window
    pub failure_count: usize,
    
    /// Sliding window duration in seconds
    pub window_seconds: u64,
    
    /// Timestamp when fallback was triggered (Unix epoch seconds)
    pub fallback_at: u64,
    
    /// Failure rate that triggered the fallback (0.0 to 1.0)
    pub failure_rate: f64,
    
    /// Proxy URL (sanitized, no credentials)
    pub proxy_url: String,
    
    /// Whether fallback was automatic or manual
    pub is_automatic: bool,
}

impl ProxyFallbackEvent {
    /// Create a new automatic fallback event from failure detector stats
    pub fn automatic(
        reason: String,
        failure_count: usize,
        window_seconds: u64,
        failure_rate: f64,
        proxy_url: String,
    ) -> Self {
        Self {
            reason,
            failure_count,
            window_seconds,
            fallback_at: current_timestamp(),
            failure_rate,
            proxy_url,
            is_automatic: true,
        }
    }
    
    /// Create a new manual fallback event
    pub fn manual(reason: String, proxy_url: String) -> Self {
        Self {
            reason,
            failure_count: 0,
            window_seconds: 0,
            fallback_at: current_timestamp(),
            failure_rate: 0.0,
            proxy_url,
            is_automatic: false,
        }
    }
}

/// Proxy recovered event
/// 
/// Emitted when proxy successfully recovers from fallback.
/// This is triggered by P5.5 automatic recovery logic.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyRecoveredEvent {
    /// Number of successful health checks that confirmed recovery
    pub successful_checks: u32,
    
    /// Proxy URL (sanitized, no credentials)
    pub proxy_url: String,
    
    /// Whether recovery was automatic or manual
    pub is_automatic: bool,
    
    /// Recovery strategy used (if automatic)
    pub strategy: Option<String>,
    
    /// Timestamp of the recovery (Unix epoch seconds)
    pub timestamp: u64,
}

impl ProxyRecoveredEvent {
    /// Create a new automatic recovery event
    pub fn automatic(successful_checks: u32, proxy_url: String, strategy: String) -> Self {
        Self {
            successful_checks,
            proxy_url,
            is_automatic: true,
            strategy: Some(strategy),
            timestamp: current_timestamp(),
        }
    }
    
    /// Create a new manual recovery event
    pub fn manual(proxy_url: String) -> Self {
        Self {
            successful_checks: 0,
            proxy_url,
            is_automatic: false,
            strategy: None,
            timestamp: current_timestamp(),
        }
    }
}

/// Proxy health check event
/// 
/// Emitted when a health check is performed (P5.5).
/// Useful for monitoring and debugging proxy connectivity.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyHealthCheckEvent {
    /// Whether the health check succeeded
    pub success: bool,
    
    /// Response time in milliseconds (if successful)
    pub response_time_ms: Option<u64>,
    
    /// Error message (if failed)
    pub error: Option<String>,
    
    /// Proxy URL (sanitized, no credentials)
    pub proxy_url: String,
    
    /// Test URL used for health check
    pub test_url: String,
    
    /// Timestamp of the health check (Unix epoch seconds)
    pub timestamp: u64,
}

impl ProxyHealthCheckEvent {
    /// Create a successful health check event
    pub fn success(response_time_ms: u64, proxy_url: String, test_url: String) -> Self {
        Self {
            success: true,
            response_time_ms: Some(response_time_ms),
            error: None,
            proxy_url,
            test_url,
            timestamp: current_timestamp(),
        }
    }
    
    /// Create a failed health check event
    pub fn failure(error: String, proxy_url: String, test_url: String) -> Self {
        Self {
            success: false,
            response_time_ms: None,
            error: Some(error),
            proxy_url,
            test_url,
            timestamp: current_timestamp(),
        }
    }
}

/// Get current Unix timestamp in seconds
fn current_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let state_event = ProxyStateEvent::new(
            ProxyState::Disabled,
            ProxyState::Enabled,
            None,
        );
        
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
}
