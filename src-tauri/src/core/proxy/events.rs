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
