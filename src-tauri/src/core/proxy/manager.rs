//! Proxy manager for coordinating proxy configuration, state, and connectors
//!
//! This module provides the `ProxyManager` which serves as the central coordinator
//! for all proxy-related functionality. It will be used by the transport layer in P5.3.

use super::{
    config::{ProxyConfig, ProxyMode},
    state::{ProxyState, ProxyStateContext, StateTransition},
    system_detector::SystemProxyDetector,
    ProxyConnector, PlaceholderConnector,
};
use anyhow::Result;
use std::sync::{Arc, RwLock};

/// Proxy manager that coordinates proxy configuration, state, and connectors
/// 
/// This manager provides a unified API for:
/// - Checking if proxy is enabled
/// - Getting current proxy configuration
/// - Detecting system proxy settings
/// - Managing proxy state transitions (for P5.4/P5.5)
/// - Providing connector instances (for P5.1/P5.2)
pub struct ProxyManager {
    /// Current proxy configuration
    config: Arc<RwLock<ProxyConfig>>,
    
    /// Current proxy state context
    state: Arc<RwLock<ProxyStateContext>>,
}

impl ProxyManager {
    /// Create a new ProxyManager with the given configuration
    pub fn new(config: ProxyConfig) -> Self {
        let state = if config.is_enabled() {
            let mut ctx = ProxyStateContext::new();
            // Transition to Enabled state if proxy is configured
            let _ = ctx.transition(StateTransition::Enable, Some("Initial configuration".to_string()));
            ctx
        } else {
            ProxyStateContext::new()
        };
        
        Self {
            config: Arc::new(RwLock::new(config)),
            state: Arc::new(RwLock::new(state)),
        }
    }
    
    /// Check if proxy is currently enabled
    /// 
    /// Returns true if proxy mode is not Off and state is Enabled
    pub fn is_enabled(&self) -> bool {
        let config = self.config.read().unwrap();
        let state = self.state.read().unwrap();
        
        config.is_enabled() && state.state == ProxyState::Enabled
    }
    
    /// Get current proxy mode
    pub fn mode(&self) -> ProxyMode {
        self.config.read().unwrap().mode
    }
    
    /// Get current proxy state
    pub fn state(&self) -> ProxyState {
        self.state.read().unwrap().state
    }
    
    /// Get proxy URL (sanitized for logging)
    pub fn sanitized_url(&self) -> String {
        self.config.read().unwrap().sanitized_url()
    }
    
    /// Get proxy URL (raw, for actual connection)
    /// 
    /// **Warning**: This may contain credentials. Do not log directly.
    pub fn proxy_url(&self) -> String {
        self.config.read().unwrap().url.clone()
    }
    
    /// Check if custom transport should be disabled
    /// 
    /// Returns true if proxy is enabled, which forces disabling custom transport
    /// to avoid conflicts with Fake SNI and IP optimization.
    pub fn should_disable_custom_transport(&self) -> bool {
        let config = self.config.read().unwrap();
        // Proxy enabled forces custom transport to be disabled
        if config.is_enabled() {
            return true;
        }
        // Otherwise respect the configuration
        config.disable_custom_transport
    }
    
    /// Update proxy configuration
    /// 
    /// This supports hot-reloading. State transitions will be applied automatically.
    pub fn update_config(&self, new_config: ProxyConfig) -> Result<()> {
        new_config.validate()?;
        
        let old_enabled = self.is_enabled();
        let new_enabled = new_config.is_enabled();
        
        // Update config
        {
            let mut config = self.config.write().unwrap();
            *config = new_config;
        }
        
        // Update state based on config change
        let mut state = self.state.write().unwrap();
        
        if !old_enabled && new_enabled {
            // Proxy was disabled, now enabled
            state.transition(StateTransition::Enable, Some("Configuration updated".to_string()))?;
            tracing::info!("Proxy enabled via configuration update");
        } else if old_enabled && !new_enabled {
            // Proxy was enabled, now disabled
            state.transition(StateTransition::Disable, Some("Configuration updated".to_string()))?;
            tracing::info!("Proxy disabled via configuration update");
        }
        
        Ok(())
    }
    
    /// Detect system proxy and return configuration if found
    /// 
    /// This is a convenience method that wraps SystemProxyDetector::detect()
    pub fn detect_system_proxy() -> Option<ProxyConfig> {
        SystemProxyDetector::detect()
    }
    
    /// Apply system proxy configuration
    /// 
    /// Detects system proxy and updates the manager's configuration if found.
    /// Returns true if system proxy was detected and applied.
    pub fn apply_system_proxy(&self) -> Result<bool> {
        if let Some(mut detected_config) = Self::detect_system_proxy() {
            // Ensure mode is set to System (not the detected type)
            detected_config.mode = ProxyMode::System;
            
            tracing::info!(
                "Applying detected system proxy: {}",
                detected_config.sanitized_url()
            );
            
            self.update_config(detected_config)?;
            Ok(true)
        } else {
            tracing::debug!("No system proxy detected");
            Ok(false)
        }
    }
    
    /// Get a proxy connector instance
    /// 
    /// In P5.0, this returns a PlaceholderConnector.
    /// In P5.1/P5.2, this will return actual HTTP or SOCKS5 connectors.
    pub fn get_connector(&self) -> Result<Box<dyn ProxyConnector>> {
        let config = self.config.read().unwrap();
        
        match config.mode {
            ProxyMode::Off => {
                // No proxy, return placeholder (falls back to direct)
                Ok(Box::new(PlaceholderConnector))
            }
            ProxyMode::Http | ProxyMode::Socks5 | ProxyMode::System => {
                // P5.0: Return placeholder
                // P5.1: Will return HttpProxyConnector
                // P5.2: Will return Socks5ProxyConnector based on config.mode
                tracing::debug!(
                    "Proxy mode {:?} configured, but returning PlaceholderConnector (P5.0)",
                    config.mode
                );
                Ok(Box::new(PlaceholderConnector))
            }
        }
    }
    
    /// Record a proxy connection failure
    /// 
    /// This will be used by P5.4 for automatic fallback detection.
    /// Currently just logs and updates counters.
    pub fn report_failure(&self, reason: &str) {
        let mut state = self.state.write().unwrap();
        state.record_failure();
        
        tracing::warn!(
            "Proxy connection failure recorded: {} (consecutive failures: {})",
            reason,
            state.consecutive_failures
        );
        
        // P5.4 will add automatic fallback logic here based on thresholds
    }
    
    /// Record a proxy connection success
    /// 
    /// This will be used by P5.5 for automatic recovery detection.
    /// Currently just logs and updates counters.
    pub fn report_success(&self) {
        let mut state = self.state.write().unwrap();
        state.record_success();
        
        tracing::debug!(
            "Proxy connection success recorded (consecutive successes: {})",
            state.consecutive_successes
        );
        
        // P5.5 will add automatic recovery logic here based on strategy
    }
    
    /// Manually trigger fallback to direct connection
    /// 
    /// This is for manual intervention or testing. P5.4 will add automatic fallback.
    pub fn manual_fallback(&self, reason: &str) -> Result<()> {
        let mut state = self.state.write().unwrap();
        state.transition(
            StateTransition::TriggerFallback,
            Some(reason.to_string()),
        )?;
        
        tracing::warn!("Manual proxy fallback triggered: {}", reason);
        Ok(())
    }
    
    /// Manually recover from fallback
    /// 
    /// This is for manual intervention or testing. P5.5 will add automatic recovery.
    pub fn manual_recover(&self) -> Result<()> {
        let mut state = self.state.write().unwrap();
        
        // Start recovery process
        state.transition(
            StateTransition::StartRecovery,
            Some("Manual recovery requested".to_string()),
        )?;
        
        // Immediately complete recovery (in P5.5, health checks will determine this)
        state.transition(
            StateTransition::CompleteRecovery,
            Some("Manual recovery".to_string()),
        )?;
        
        tracing::info!("Manual proxy recovery completed");
        Ok(())
    }
    
    /// Get current state context for diagnostics
    pub fn get_state_context(&self) -> ProxyStateContext {
        self.state.read().unwrap().clone()
    }
}

impl Default for ProxyManager {
    fn default() -> Self {
        Self::new(ProxyConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_proxy_manager_default() {
        let manager = ProxyManager::default();
        assert!(!manager.is_enabled());
        assert_eq!(manager.mode(), ProxyMode::Off);
        assert_eq!(manager.state(), ProxyState::Disabled);
    }

    #[test]
    fn test_proxy_manager_enabled() {
        let config = ProxyConfig {
            mode: ProxyMode::Http,
            url: "http://proxy.example.com:8080".to_string(),
            ..Default::default()
        };
        
        let manager = ProxyManager::new(config);
        assert!(manager.is_enabled());
        assert_eq!(manager.mode(), ProxyMode::Http);
        assert_eq!(manager.state(), ProxyState::Enabled);
    }

    #[test]
    fn test_proxy_manager_should_disable_custom_transport() {
        // Proxy disabled - respect config
        let manager = ProxyManager::default();
        assert!(!manager.should_disable_custom_transport());
        
        // Proxy enabled - force disable custom transport
        let config = ProxyConfig {
            mode: ProxyMode::Http,
            url: "http://proxy.example.com:8080".to_string(),
            disable_custom_transport: false, // User set to false
            ..Default::default()
        };
        let manager = ProxyManager::new(config);
        assert!(manager.should_disable_custom_transport()); // But we force it to true
    }

    #[test]
    fn test_proxy_manager_update_config() {
        let manager = ProxyManager::default();
        assert!(!manager.is_enabled());
        
        // Enable proxy
        let new_config = ProxyConfig {
            mode: ProxyMode::Http,
            url: "http://proxy.example.com:8080".to_string(),
            ..Default::default()
        };
        
        manager.update_config(new_config).unwrap();
        assert!(manager.is_enabled());
        assert_eq!(manager.state(), ProxyState::Enabled);
        
        // Disable proxy
        manager.update_config(ProxyConfig::default()).unwrap();
        assert!(!manager.is_enabled());
        assert_eq!(manager.state(), ProxyState::Disabled);
    }

    #[test]
    fn test_proxy_manager_sanitized_url() {
        let config = ProxyConfig {
            mode: ProxyMode::Http,
            url: "http://user:pass@proxy.example.com:8080".to_string(),
            ..Default::default()
        };
        
        let manager = ProxyManager::new(config);
        let sanitized = manager.sanitized_url();
        
        // Should hide credentials
        assert!(sanitized.contains("***"));
        assert!(!sanitized.contains("pass"));
    }

    #[test]
    fn test_proxy_manager_get_connector() {
        let manager = ProxyManager::default();
        
        // Should return placeholder connector
        let connector = manager.get_connector().unwrap();
        assert_eq!(connector.proxy_type(), "placeholder");
    }

    #[test]
    fn test_proxy_manager_failure_reporting() {
        let config = ProxyConfig {
            mode: ProxyMode::Http,
            url: "http://proxy.example.com:8080".to_string(),
            ..Default::default()
        };
        
        let manager = ProxyManager::new(config);
        
        // Record failures
        manager.report_failure("Connection timeout");
        manager.report_failure("Connection refused");
        
        let context = manager.get_state_context();
        assert_eq!(context.consecutive_failures, 2);
        assert_eq!(context.consecutive_successes, 0);
        
        // Record success resets failure counter
        manager.report_success();
        let context = manager.get_state_context();
        assert_eq!(context.consecutive_failures, 0);
        assert_eq!(context.consecutive_successes, 1);
    }

    #[test]
    fn test_proxy_manager_manual_fallback_recovery() {
        let config = ProxyConfig {
            mode: ProxyMode::Http,
            url: "http://proxy.example.com:8080".to_string(),
            ..Default::default()
        };
        
        let manager = ProxyManager::new(config);
        assert_eq!(manager.state(), ProxyState::Enabled);
        
        // Manual fallback
        manager.manual_fallback("Testing fallback").unwrap();
        assert_eq!(manager.state(), ProxyState::Fallback);
        
        // Manual recovery
        manager.manual_recover().unwrap();
        assert_eq!(manager.state(), ProxyState::Enabled);
    }

    #[test]
    fn test_proxy_manager_detect_system_proxy() {
        // Just verify it doesn't panic
        let result = ProxyManager::detect_system_proxy();
        // Result depends on actual system configuration
        if let Some(config) = result {
            assert!(config.validate().is_ok());
        }
    }

    #[test]
    fn test_proxy_manager_apply_system_proxy() {
        let manager = ProxyManager::default();
        
        // Try to apply system proxy
        let applied = manager.apply_system_proxy().unwrap();
        
        // If system proxy was detected and applied
        if applied {
            assert_eq!(manager.mode(), ProxyMode::System);
            assert!(manager.is_enabled());
        } else {
            // No system proxy, should remain disabled
            assert!(!manager.is_enabled());
        }
    }

    #[test]
    fn test_proxy_manager_concurrent_reads() {
        use std::sync::Arc;
        use std::thread;
        
        let config = ProxyConfig {
            mode: ProxyMode::Http,
            url: "http://proxy.example.com:8080".to_string(),
            ..Default::default()
        };
        
        let manager = Arc::new(ProxyManager::new(config));
        let mut handles = vec![];
        
        // Spawn multiple threads reading state
        for _ in 0..10 {
            let manager_clone = Arc::clone(&manager);
            let handle = thread::spawn(move || {
                assert!(manager_clone.is_enabled());
                assert_eq!(manager_clone.mode(), ProxyMode::Http);
                let _ = manager_clone.sanitized_url();
                let _ = manager_clone.state();
            });
            handles.push(handle);
        }
        
        for handle in handles {
            handle.join().unwrap();
        }
    }

    #[test]
    fn test_proxy_manager_concurrent_state_updates() {
        use std::sync::Arc;
        use std::thread;
        
        let config = ProxyConfig {
            mode: ProxyMode::Http,
            url: "http://proxy.example.com:8080".to_string(),
            ..Default::default()
        };
        
        let manager = Arc::new(ProxyManager::new(config));
        let mut handles = vec![];
        
        // Spawn threads reporting failures and successes
        for i in 0..5 {
            let manager_clone = Arc::clone(&manager);
            let handle = thread::spawn(move || {
                if i % 2 == 0 {
                    manager_clone.report_failure("Test failure");
                } else {
                    manager_clone.report_success();
                }
            });
            handles.push(handle);
        }
        
        for handle in handles {
            handle.join().unwrap();
        }
        
        // Just verify no panics occurred
        let context = manager.get_state_context();
        assert!(context.consecutive_failures > 0 || context.consecutive_successes > 0);
    }

    #[test]
    fn test_proxy_manager_invalid_config_update() {
        let manager = ProxyManager::default();
        
        let invalid_config = ProxyConfig {
            mode: ProxyMode::Http,
            url: "http://proxy.example.com:8080".to_string(),
            timeout_seconds: 0, // Invalid!
            ..Default::default()
        };
        
        // Should reject invalid config
        assert!(manager.update_config(invalid_config).is_err());
        
        // State should remain unchanged
        assert!(!manager.is_enabled());
        assert_eq!(manager.state(), ProxyState::Disabled);
    }

    #[test]
    fn test_proxy_manager_state_synchronization() {
        let manager = ProxyManager::default();
        
        // Enable proxy
        let config = ProxyConfig {
            mode: ProxyMode::Http,
            url: "http://proxy.example.com:8080".to_string(),
            ..Default::default()
        };
        manager.update_config(config).unwrap();
        
        // Both config and state should be enabled
        assert!(manager.is_enabled());
        assert_eq!(manager.mode(), ProxyMode::Http);
        assert_eq!(manager.state(), ProxyState::Enabled);
        
        // Disable via config update
        manager.update_config(ProxyConfig::default()).unwrap();
        
        // Both should be disabled
        assert!(!manager.is_enabled());
        assert_eq!(manager.mode(), ProxyMode::Off);
        assert_eq!(manager.state(), ProxyState::Disabled);
    }

    #[test]
    fn test_proxy_manager_raw_url_warning() {
        let config = ProxyConfig {
            mode: ProxyMode::Http,
            url: "http://user:secret123@proxy.example.com:8080".to_string(),
            ..Default::default()
        };
        
        let manager = ProxyManager::new(config);
        
        // Raw URL should contain credentials (for actual connection)
        let raw = manager.proxy_url();
        assert!(raw.contains("secret123"));
        
        // Sanitized should not
        let sanitized = manager.sanitized_url();
        assert!(!sanitized.contains("secret123"));
        assert!(sanitized.contains("***"));
    }

    #[test]
    fn test_proxy_manager_mode_transitions() {
        let manager = ProxyManager::default();
        
        // Off -> Http
        let config = ProxyConfig {
            mode: ProxyMode::Http,
            url: "http://proxy.example.com:8080".to_string(),
            ..Default::default()
        };
        manager.update_config(config).unwrap();
        assert_eq!(manager.mode(), ProxyMode::Http);
        assert_eq!(manager.state(), ProxyState::Enabled);
        
        // Http -> Socks5 (should keep enabled state)
        let config = ProxyConfig {
            mode: ProxyMode::Socks5,
            url: "socks5://proxy.example.com:1080".to_string(),
            ..Default::default()
        };
        manager.update_config(config).unwrap();
        assert_eq!(manager.mode(), ProxyMode::Socks5);
        assert_eq!(manager.state(), ProxyState::Enabled);
    }

    #[test]
    fn test_proxy_manager_get_state_context() {
        let config = ProxyConfig {
            mode: ProxyMode::Http,
            url: "http://proxy.example.com:8080".to_string(),
            ..Default::default()
        };
        
        let manager = ProxyManager::new(config);
        
        manager.report_failure("Error 1");
        manager.report_failure("Error 2");
        
        let context = manager.get_state_context();
        assert_eq!(context.state, ProxyState::Enabled);
        assert_eq!(context.consecutive_failures, 2);
        assert_eq!(context.consecutive_successes, 0);
    }
}
