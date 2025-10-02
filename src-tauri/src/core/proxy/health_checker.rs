//! Proxy health checker for automatic recovery
//!
//! This module provides health checking functionality to test proxy availability
//! during recovery phase. It supports:
//! - Periodic health checks with configurable interval
//! - Cooldown period after fallback before starting checks
//! - Multiple probe strategies (TCP connect, HTTP HEAD)
//! - Latency measurement for diagnostics

use super::{ProxyConnector, ProxyConfig, ProxyError};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Result of a single health check probe
#[derive(Debug, Clone, PartialEq)]
pub enum ProbeResult {
    /// Probe succeeded
    Success {
        /// Latency in milliseconds
        latency_ms: u64,
    },
    /// Probe failed
    Failure {
        /// Error message
        error: String,
    },
    /// Probe skipped due to cooldown
    Skipped {
        /// Remaining cooldown seconds
        remaining_seconds: u64,
    },
}

impl ProbeResult {
    /// Check if probe was successful
    pub fn is_success(&self) -> bool {
        matches!(self, ProbeResult::Success { .. })
    }
    
    /// Check if probe failed
    pub fn is_failure(&self) -> bool {
        matches!(self, ProbeResult::Failure { .. })
    }
    
    /// Check if probe was skipped
    pub fn is_skipped(&self) -> bool {
        matches!(self, ProbeResult::Skipped { .. })
    }
    
    /// Get latency if successful
    pub fn latency_ms(&self) -> Option<u64> {
        match self {
            ProbeResult::Success { latency_ms } => Some(*latency_ms),
            _ => None,
        }
    }
    
    /// Get error message if failed
    pub fn error(&self) -> Option<&str> {
        match self {
            ProbeResult::Failure { error } => Some(error.as_str()),
            _ => None,
        }
    }
}

/// Proxy health checker
/// 
/// Performs periodic health checks to detect when a proxy becomes available
/// after being in fallback state. Supports cooldown period and multiple
/// probe strategies.
pub struct ProxyHealthChecker {
    /// Configuration for health checking
    config: HealthCheckConfig,
    
    /// Timestamp of last fallback (Unix seconds)
    /// Used to enforce cooldown period
    fallback_at: Option<u64>,
    
    /// Consecutive successful probes
    consecutive_successes: u32,
    
    /// Consecutive failed probes
    consecutive_failures: u32,
}

/// Health check configuration
#[derive(Debug, Clone)]
pub struct HealthCheckConfig {
    /// Check interval in seconds (default: 60)
    pub interval_seconds: u64,
    
    /// Cooldown period in seconds before starting checks (default: 300)
    pub cooldown_seconds: u64,
    
    /// Recovery strategy: "immediate", "consecutive", "exponential-backoff"
    pub strategy: String,
    
    /// Probe timeout in seconds (default: 10)
    pub probe_timeout_seconds: u64,
    
    /// Probe target (default: "www.github.com:443")
    /// A reliable target that should always be reachable if proxy works
    pub probe_target: String,
    
    /// Number of consecutive successes required for recovery (default: 3)
    /// Used by "consecutive" strategy
    pub consecutive_threshold: u32,
}

impl Default for HealthCheckConfig {
    fn default() -> Self {
        Self {
            interval_seconds: 60,
            cooldown_seconds: 300,
            strategy: "consecutive".to_string(),
            probe_timeout_seconds: 10,
            probe_target: "www.github.com:443".to_string(),
            consecutive_threshold: 3,
        }
    }
}

impl HealthCheckConfig {
    /// Create from ProxyConfig
    pub fn from_proxy_config(config: &ProxyConfig) -> Self {
        Self {
            interval_seconds: config.health_check_interval_seconds,
            cooldown_seconds: config.recovery_cooldown_seconds,
            strategy: config.recovery_strategy.clone(),
            probe_timeout_seconds: config.probe_timeout_seconds,
            probe_target: config.probe_url.clone(),
            consecutive_threshold: config.recovery_consecutive_threshold,
        }
    }
}

impl ProxyHealthChecker {
    /// Create a new health checker with the given configuration
    pub fn new(config: HealthCheckConfig) -> Self {
        Self {
            config,
            fallback_at: None,
            consecutive_successes: 0,
            consecutive_failures: 0,
        }
    }
    
    /// Create from ProxyConfig
    pub fn from_proxy_config(proxy_config: &ProxyConfig) -> Self {
        Self::new(HealthCheckConfig::from_proxy_config(proxy_config))
    }
    
    /// Record fallback timestamp
    /// 
    /// This starts the cooldown period. Health checks will be skipped until
    /// cooldown expires.
    pub fn record_fallback(&mut self) {
        self.fallback_at = Some(current_timestamp());
        self.consecutive_successes = 0;
        self.consecutive_failures = 0;
        
        tracing::debug!(
            "Health checker recorded fallback at {}, cooldown={}s",
            self.fallback_at.unwrap(),
            self.config.cooldown_seconds
        );
    }
    
    /// Check if cooldown period has expired
    pub fn is_cooldown_expired(&self) -> bool {
        if let Some(fallback_at) = self.fallback_at {
            let elapsed = current_timestamp().saturating_sub(fallback_at);
            elapsed >= self.config.cooldown_seconds
        } else {
            // No fallback recorded, cooldown is expired by default
            true
        }
    }
    
    /// Get remaining cooldown seconds
    pub fn remaining_cooldown_seconds(&self) -> u64 {
        if let Some(fallback_at) = self.fallback_at {
            let elapsed = current_timestamp().saturating_sub(fallback_at);
            self.config.cooldown_seconds.saturating_sub(elapsed)
        } else {
            0
        }
    }
    
    /// Perform a health check probe
    /// 
    /// This attempts to connect through the proxy to a known-good target.
    /// Returns ProbeResult indicating success/failure/skipped.
    pub fn probe(&mut self, connector: &dyn ProxyConnector) -> ProbeResult {
        // Check cooldown
        if !self.is_cooldown_expired() {
            let remaining = self.remaining_cooldown_seconds();
            tracing::debug!("Health check skipped: cooldown not expired ({}s remaining)", remaining);
            return ProbeResult::Skipped {
                remaining_seconds: remaining,
            };
        }
        
        // Parse target host and port
        let (host, port) = match self.parse_probe_target() {
            Ok(target) => target,
            Err(e) => {
                tracing::error!("Invalid probe target: {}", e);
                return ProbeResult::Failure {
                    error: format!("Invalid probe target: {}", e),
                };
            }
        };
        
        tracing::debug!(
            "Starting health check probe to {}:{} via {} proxy",
            host,
            port,
            connector.proxy_type()
        );
        
        // Perform probe with timeout
        let start = Instant::now();
        let result = self.probe_with_timeout(connector, host, port);
        let latency_ms = start.elapsed().as_millis() as u64;
        
        match result {
            Ok(_) => {
                self.consecutive_successes += 1;
                self.consecutive_failures = 0;
                
                tracing::info!(
                    "Health check probe succeeded ({}ms, consecutive: {})",
                    latency_ms,
                    self.consecutive_successes
                );
                
                ProbeResult::Success { latency_ms }
            }
            Err(e) => {
                self.consecutive_failures += 1;
                self.consecutive_successes = 0;
                
                let error = e.to_string();
                tracing::warn!(
                    "Health check probe failed: {} (consecutive: {})",
                    error,
                    self.consecutive_failures
                );
                
                ProbeResult::Failure { error }
            }
        }
    }
    
    /// Check if recovery should be triggered based on probe results
    /// 
    /// Decision depends on recovery strategy:
    /// - "immediate": First success triggers recovery
    /// - "consecutive": Need N consecutive successes (configurable via consecutive_threshold)
    /// - "exponential-backoff": Future extension (P5.6+)
    pub fn should_recover(&self) -> bool {
        match self.config.strategy.as_str() {
            "immediate" => {
                // Any success triggers recovery
                self.consecutive_successes > 0
            }
            "consecutive" => {
                // Need N consecutive successes (configurable)
                self.consecutive_successes >= self.config.consecutive_threshold
            }
            "exponential-backoff" => {
                // For now, same as consecutive
                // Future: implement backoff logic
                self.consecutive_successes >= self.config.consecutive_threshold
            }
            _ => {
                // Unknown strategy, use conservative default
                tracing::warn!("Unknown recovery strategy: {}, using consecutive", self.config.strategy);
                self.consecutive_successes >= self.config.consecutive_threshold
            }
        }
    }
    
    /// Reset health checker state
    /// 
    /// Called after successful recovery to clear counters.
    pub fn reset(&mut self) {
        self.fallback_at = None;
        self.consecutive_successes = 0;
        self.consecutive_failures = 0;
        
        tracing::debug!("Health checker reset");
    }
    
    /// Get consecutive success count
    pub fn consecutive_successes(&self) -> u32 {
        self.consecutive_successes
    }
    
    /// Get consecutive failure count
    pub fn consecutive_failures(&self) -> u32 {
        self.consecutive_failures
    }
    
    /// Get health check interval
    pub fn interval(&self) -> Duration {
        Duration::from_secs(self.config.interval_seconds)
    }
    
    // Private helper methods
    
    /// Parse probe target into (host, port)
    fn parse_probe_target(&self) -> Result<(&str, u16), String> {
        let target = &self.config.probe_target;
        
        if let Some(colon_pos) = target.rfind(':') {
            let host = &target[..colon_pos];
            let port_str = &target[colon_pos + 1..];
            
            match port_str.parse::<u16>() {
                Ok(port) => Ok((host, port)),
                Err(_) => Err(format!("Invalid port in target: {}", port_str)),
            }
        } else {
            Err("Target must be in format 'host:port'".to_string())
        }
    }
    
    /// Probe with timeout
    fn probe_with_timeout(
        &self,
        connector: &dyn ProxyConnector,
        host: &str,
        port: u16,
    ) -> Result<(), ProxyError> {
        // Note: connector.connect() already has timeout built-in
        // We just need to call it and handle the result
        
        match connector.connect(host, port) {
            Ok(_stream) => {
                // Connection successful, drop stream immediately
                // We only care about reachability, not actual communication
                Ok(())
            }
            Err(e) => {
                // Connection failed
                Err(e)
            }
        }
    }
}

/// Get current Unix timestamp in seconds
fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::net::TcpStream;
    
    /// Mock connector for testing
    struct MockConnector {
        should_succeed: Arc<AtomicBool>,
    }
    
    impl MockConnector {
        fn new(should_succeed: bool) -> Self {
            Self {
                should_succeed: Arc::new(AtomicBool::new(should_succeed)),
            }
        }
        
        fn set_should_succeed(&self, value: bool) {
            self.should_succeed.store(value, Ordering::Relaxed);
        }
    }
    
    impl ProxyConnector for MockConnector {
        fn connect(&self, _host: &str, _port: u16) -> Result<TcpStream, ProxyError> {
            if self.should_succeed.load(Ordering::Relaxed) {
                // Can't create a real TcpStream in tests, so we'll return an error
                // but the test will check ProbeResult directly
                Ok(TcpStream::connect("127.0.0.1:1").unwrap_or_else(|_| {
                    // This is a hack for testing - we can't create a real stream
                    // but we need to return something
                    panic!("Mock connector success path should not reach here in real tests")
                }))
            } else {
                Err(ProxyError::network("Mock connection failed".to_string()))
            }
        }
        
        fn proxy_type(&self) -> &str {
            "mock"
        }
    }
    
    #[test]
    fn test_health_check_config_default() {
        let config = HealthCheckConfig::default();
        assert_eq!(config.interval_seconds, 60);
        assert_eq!(config.cooldown_seconds, 300);
        assert_eq!(config.strategy, "consecutive");
        assert_eq!(config.probe_timeout_seconds, 10);
        assert_eq!(config.probe_target, "www.github.com:443");
    }
    
    #[test]
    fn test_health_check_config_from_proxy_config() {
        let mut proxy_config = ProxyConfig::default();
        proxy_config.health_check_interval_seconds = 120;
        proxy_config.recovery_cooldown_seconds = 600;
        proxy_config.recovery_strategy = "immediate".to_string();
        
        let config = HealthCheckConfig::from_proxy_config(&proxy_config);
        assert_eq!(config.interval_seconds, 120);
        assert_eq!(config.cooldown_seconds, 600);
        assert_eq!(config.strategy, "immediate");
    }
    
    #[test]
    fn test_probe_result_is_success() {
        let result = ProbeResult::Success { latency_ms: 100 };
        assert!(result.is_success());
        assert!(!result.is_failure());
        assert!(!result.is_skipped());
        assert_eq!(result.latency_ms(), Some(100));
    }
    
    #[test]
    fn test_probe_result_is_failure() {
        let result = ProbeResult::Failure {
            error: "Connection refused".to_string(),
        };
        assert!(!result.is_success());
        assert!(result.is_failure());
        assert!(!result.is_skipped());
        assert_eq!(result.error(), Some("Connection refused"));
    }
    
    #[test]
    fn test_probe_result_is_skipped() {
        let result = ProbeResult::Skipped {
            remaining_seconds: 60,
        };
        assert!(!result.is_success());
        assert!(!result.is_failure());
        assert!(result.is_skipped());
    }
    
    #[test]
    fn test_cooldown_not_expired() {
        let config = HealthCheckConfig {
            cooldown_seconds: 300,
            ..Default::default()
        };
        
        let mut checker = ProxyHealthChecker::new(config);
        checker.record_fallback();
        
        // Cooldown should not be expired immediately after fallback
        assert!(!checker.is_cooldown_expired());
        assert!(checker.remaining_cooldown_seconds() > 0);
    }
    
    #[test]
    fn test_cooldown_expired_with_no_fallback() {
        let checker = ProxyHealthChecker::new(HealthCheckConfig::default());
        
        // No fallback recorded, cooldown should be considered expired
        assert!(checker.is_cooldown_expired());
        assert_eq!(checker.remaining_cooldown_seconds(), 0);
    }
    
    #[test]
    fn test_cooldown_expired_after_delay() {
        let config = HealthCheckConfig {
            cooldown_seconds: 0, // Zero cooldown for testing
            ..Default::default()
        };
        
        let mut checker = ProxyHealthChecker::new(config);
        checker.record_fallback();
        
        // With zero cooldown, should be expired immediately
        assert!(checker.is_cooldown_expired());
        assert_eq!(checker.remaining_cooldown_seconds(), 0);
    }
    
    #[test]
    fn test_parse_probe_target_valid() {
        let checker = ProxyHealthChecker::new(HealthCheckConfig::default());
        
        let result = checker.parse_probe_target();
        assert!(result.is_ok());
        
        let (host, port) = result.unwrap();
        assert_eq!(host, "www.github.com");
        assert_eq!(port, 443);
    }
    
    #[test]
    fn test_parse_probe_target_invalid_no_port() {
        let config = HealthCheckConfig {
            probe_target: "www.github.com".to_string(),
            ..Default::default()
        };
        
        let checker = ProxyHealthChecker::new(config);
        let result = checker.parse_probe_target();
        assert!(result.is_err());
    }
    
    #[test]
    fn test_parse_probe_target_invalid_port() {
        let config = HealthCheckConfig {
            probe_target: "www.github.com:abc".to_string(),
            ..Default::default()
        };
        
        let checker = ProxyHealthChecker::new(config);
        let result = checker.parse_probe_target();
        assert!(result.is_err());
    }
    
    #[test]
    fn test_should_recover_immediate_strategy() {
        let config = HealthCheckConfig {
            strategy: "immediate".to_string(),
            ..Default::default()
        };
        
        let mut checker = ProxyHealthChecker::new(config);
        
        // No successes yet
        assert!(!checker.should_recover());
        
        // One success should trigger recovery
        checker.consecutive_successes = 1;
        assert!(checker.should_recover());
    }
    
    #[test]
    fn test_should_recover_consecutive_strategy() {
        let config = HealthCheckConfig {
            strategy: "consecutive".to_string(),
            consecutive_threshold: 3,
            ..Default::default()
        };
        
        let mut checker = ProxyHealthChecker::new(config);
        
        // Less than 3 successes
        checker.consecutive_successes = 2;
        assert!(!checker.should_recover());
        
        // Exactly 3 successes should trigger recovery
        checker.consecutive_successes = 3;
        assert!(checker.should_recover());
        
        // More than 3 also works
        checker.consecutive_successes = 5;
        assert!(checker.should_recover());
    }
    
    #[test]
    fn test_should_recover_consecutive_strategy_custom_threshold() {
        // Test with custom threshold of 5
        let config = HealthCheckConfig {
            strategy: "consecutive".to_string(),
            consecutive_threshold: 5,
            ..Default::default()
        };
        
        let mut checker = ProxyHealthChecker::new(config);
        
        // Less than 5 successes should not trigger
        checker.consecutive_successes = 4;
        assert!(!checker.should_recover());
        
        // Exactly 5 successes should trigger
        checker.consecutive_successes = 5;
        assert!(checker.should_recover());
        
        // More than 5 also works
        checker.consecutive_successes = 7;
        assert!(checker.should_recover());
    }
    
    #[test]
    fn test_should_recover_consecutive_strategy_threshold_one() {
        // Edge case: threshold of 1 (acts like immediate)
        let config = HealthCheckConfig {
            strategy: "consecutive".to_string(),
            consecutive_threshold: 1,
            ..Default::default()
        };
        
        let mut checker = ProxyHealthChecker::new(config);
        
        // One success should trigger
        checker.consecutive_successes = 1;
        assert!(checker.should_recover());
    }
    
    #[test]
    fn test_reset_clears_state() {
        let mut checker = ProxyHealthChecker::new(HealthCheckConfig::default());
        
        // Set some state
        checker.record_fallback();
        checker.consecutive_successes = 5;
        checker.consecutive_failures = 3;
        
        // Reset should clear everything
        checker.reset();
        
        assert_eq!(checker.fallback_at, None);
        assert_eq!(checker.consecutive_successes, 0);
        assert_eq!(checker.consecutive_failures, 0);
    }
    
    #[test]
    fn test_record_fallback_resets_counters() {
        let mut checker = ProxyHealthChecker::new(HealthCheckConfig::default());
        
        // Set some counters
        checker.consecutive_successes = 5;
        checker.consecutive_failures = 3;
        
        // Record fallback should reset counters
        checker.record_fallback();
        
        assert_eq!(checker.consecutive_successes, 0);
        assert_eq!(checker.consecutive_failures, 0);
        assert!(checker.fallback_at.is_some());
    }
    
    #[test]
    fn test_consecutive_counts() {
        let checker = ProxyHealthChecker::new(HealthCheckConfig::default());
        
        assert_eq!(checker.consecutive_successes(), 0);
        assert_eq!(checker.consecutive_failures(), 0);
    }
    
    #[test]
    fn test_interval() {
        let config = HealthCheckConfig {
            interval_seconds: 120,
            ..Default::default()
        };
        
        let checker = ProxyHealthChecker::new(config);
        assert_eq!(checker.interval(), Duration::from_secs(120));
    }
}
