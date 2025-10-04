//! Proxy manager for coordinating proxy configuration, state, and connectors
//!
//! This module provides the `ProxyManager` which serves as the central coordinator
//! for all proxy-related functionality. It will be used by the transport layer in P5.3.

use super::{
    config::{ProxyConfig, ProxyMode},
    detector::ProxyFailureDetector,
    events::{ProxyFallbackEvent, ProxyHealthCheckEvent, ProxyRecoveredEvent},
    health_checker::{ProbeResult, ProxyHealthChecker},
    state::{ProxyState, ProxyStateContext, StateTransition},
    system_detector::SystemProxyDetector,
    HttpProxyConnector, PlaceholderConnector, ProxyConnector, Socks5ProxyConnector,
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
/// - Monitoring proxy failures and triggering fallback (P5.4)
/// - Health checking and automatic recovery (P5.5)
pub struct ProxyManager {
    /// Current proxy configuration
    config: Arc<RwLock<ProxyConfig>>,

    /// Current proxy state context
    state: Arc<RwLock<ProxyStateContext>>,

    /// Failure detector for automatic fallback (P5.4)
    failure_detector: ProxyFailureDetector,

    /// Health checker for automatic recovery (P5.5)
    health_checker: Arc<RwLock<ProxyHealthChecker>>,
}

impl ProxyManager {
    /// Create a new `ProxyManager` with the given configuration
    pub fn new(config: ProxyConfig) -> Self {
        let state = if config.is_enabled() {
            let mut ctx = ProxyStateContext::new();
            // Transition to Enabled state if proxy is configured
            let _ = ctx.transition(
                StateTransition::Enable,
                Some("Initial configuration".to_string()),
            );
            ctx
        } else {
            ProxyStateContext::new()
        };

        // Create failure detector from config
        let failure_detector =
            ProxyFailureDetector::new(config.fallback_window_seconds, config.fallback_threshold);

        // Create health checker from config
        let health_checker = ProxyHealthChecker::from_proxy_config(&config);

        Self {
            config: Arc::new(RwLock::new(config)),
            state: Arc::new(RwLock::new(state)),
            failure_detector,
            health_checker: Arc::new(RwLock::new(health_checker)),
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
        let current_state = self.state();

        // Update config
        {
            let mut config = self.config.write().unwrap();
            *config = new_config;
        }

        // Update state based on config change
        // Only transition if we're in Disabled or Enabled states
        // Don't interfere with Fallback/Recovering states
        if current_state == ProxyState::Disabled || current_state == ProxyState::Enabled {
            let mut state = self.state.write().unwrap();

            if !old_enabled && new_enabled {
                // Proxy was disabled, now enabled
                state.transition(
                    StateTransition::Enable,
                    Some("Configuration updated".to_string()),
                )?;
                tracing::info!("Proxy enabled via configuration update");
            } else if old_enabled && !new_enabled {
                // Proxy was enabled, now disabled
                state.transition(
                    StateTransition::Disable,
                    Some("Configuration updated".to_string()),
                )?;
                tracing::info!("Proxy disabled via configuration update");
            }
        } else {
            // In Fallback or Recovering state, just update config without state transition
            tracing::debug!(
                "Config updated while in {:?} state, preserving current state",
                current_state
            );
        }

        Ok(())
    }

    /// Detect system proxy and return configuration if found
    ///
    /// This is a convenience method that wraps `SystemProxyDetector::detect()`
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
    /// Returns the appropriate connector based on the current proxy mode:
    /// - Off: `PlaceholderConnector` (direct connection)
    /// - Http: `HttpProxyConnector` (P5.1)
    /// - Socks5: `PlaceholderConnector` (P5.2 will implement `Socks5ProxyConnector`)
    /// - System: Based on detected system proxy type
    pub fn get_connector(&self) -> Result<Box<dyn ProxyConnector>> {
        let config = self.config.read().unwrap();

        match config.mode {
            ProxyMode::Off => {
                // No proxy, return placeholder (falls back to direct)
                tracing::debug!("Proxy mode is Off, using direct connection");
                Ok(Box::new(PlaceholderConnector))
            }
            ProxyMode::Http => {
                // P5.1: Return HttpProxyConnector
                tracing::debug!(
                    "Creating HTTP proxy connector for {}",
                    config.sanitized_url()
                );

                let connector = HttpProxyConnector::new(
                    config.url.clone(),
                    config.username.clone(),
                    config.password.clone(),
                    config.timeout(),
                );

                Ok(Box::new(connector))
            }
            ProxyMode::Socks5 => {
                // P5.2: Return Socks5ProxyConnector
                tracing::debug!(
                    "Creating SOCKS5 proxy connector for {}",
                    config.sanitized_url()
                );

                let connector = Socks5ProxyConnector::new(
                    config.url.clone(),
                    config.username.clone(),
                    config.password.clone(),
                    config.timeout(),
                )?;

                Ok(Box::new(connector))
            }
            ProxyMode::System => {
                // Use system-detected proxy type
                // For now, fall back to placeholder
                // P5.2: Will implement proper system proxy handling after detection
                tracing::debug!(
                    "System proxy mode configured, but using PlaceholderConnector (system proxy resolution pending)"
                );
                Ok(Box::new(PlaceholderConnector))
            }
        }
    }

    /// Record a proxy connection failure
    ///
    /// P5.4: Integrated with `FailureDetector` for automatic fallback
    pub fn report_failure(&self, reason: &str) {
        // Update state counters
        {
            let mut state = self.state.write().unwrap();
            state.record_failure();

            tracing::warn!(
                "Proxy connection failure recorded: {} (consecutive failures: {})",
                reason,
                state.consecutive_failures
            );
        }

        // Report to failure detector
        self.failure_detector.report_failure();

        // Log detector stats at debug level
        let stats = self.failure_detector.get_stats();
        tracing::debug!(
            "Failure detector updated: {}/{} attempts failed ({:.1}%), threshold={:.1}%",
            stats.failures,
            stats.total_attempts,
            stats.failure_rate * 100.0,
            stats.threshold * 100.0
        );

        // Check if fallback should be triggered
        if self.failure_detector.should_fallback() {
            self.trigger_automatic_fallback(reason);
        }
    }

    /// Record a proxy connection success
    ///
    /// This will be used by P5.5 for automatic recovery detection.
    /// Currently just logs and updates counters.
    pub fn report_success(&self) {
        // Update state counters
        {
            let mut state = self.state.write().unwrap();
            state.record_success();

            tracing::debug!(
                "Proxy connection success recorded (consecutive successes: {})",
                state.consecutive_successes
            );
        }

        // Report to failure detector
        self.failure_detector.report_success();

        // P5.5 will add automatic recovery logic here based on strategy
    }

    /// Trigger automatic fallback to direct connection
    ///
    /// Internal method called when failure rate exceeds threshold.
    /// Emits `ProxyFallbackEvent`.
    fn trigger_automatic_fallback(&self, last_error: &str) {
        // Get failure stats
        let stats = self.failure_detector.get_stats();

        // Mark fallback as triggered to prevent repeated triggers
        self.failure_detector.mark_fallback_triggered();

        // Transition state to Fallback
        {
            let mut state = self.state.write().unwrap();
            let reason = format!(
                "Failure rate {:.1}% exceeded threshold {:.1}% ({}/{} attempts in {}s window)",
                stats.failure_rate * 100.0,
                stats.threshold * 100.0,
                stats.failures,
                stats.total_attempts,
                stats.window_seconds
            );

            if let Err(e) = state.transition(StateTransition::TriggerFallback, Some(reason.clone()))
            {
                tracing::error!("Failed to transition to fallback state: {}", e);
                return;
            }

            tracing::warn!("Automatic proxy fallback triggered: {}", reason);
        }

        // Emit fallback event (P5.6 will hook this to frontend)
        let event = ProxyFallbackEvent::automatic(
            last_error.to_string(),
            stats.failures,
            stats.window_seconds,
            stats.failure_rate,
            self.sanitized_url(),
        );

        tracing::info!(
            "Proxy fallback event emitted: failures={}, rate={:.2}%, window={}s",
            event.failure_count,
            event.failure_rate * 100.0,
            event.window_seconds
        );

        // TODO P5.6: Publish event to frontend
        // crate::events::publish_global(ProxyEvent::Fallback(event));

        // Record fallback in health checker to start cooldown period (P5.5)
        {
            let mut health_checker = self.health_checker.write().unwrap();
            health_checker.record_fallback();
        }
    }

    /// Manually trigger fallback to direct connection
    ///
    /// This is for manual intervention or testing. P5.4 adds automatic fallback.
    pub fn manual_fallback(&self, reason: &str) -> Result<()> {
        let mut state = self.state.write().unwrap();
        state.transition(StateTransition::TriggerFallback, Some(reason.to_string()))?;

        // Mark detector as fallback triggered
        self.failure_detector.mark_fallback_triggered();

        // Record fallback in health checker (P5.5)
        {
            let mut health_checker = self.health_checker.write().unwrap();
            health_checker.record_fallback();
        }

        tracing::warn!("Manual proxy fallback triggered: {}", reason);

        // Emit manual fallback event
        let event = ProxyFallbackEvent::manual(reason.to_string(), self.sanitized_url());

        tracing::info!("Manual proxy fallback event emitted: {}", event.reason);

        // TODO P5.6: Publish event to frontend
        // crate::events::publish_global(ProxyEvent::Fallback(event));

        Ok(())
    }

    /// Perform a health check probe (P5.5)
    ///
    /// This should be called periodically (e.g., every 60 seconds) when in Fallback state.
    /// Returns `ProbeResult` indicating success/failure/skipped.
    ///
    /// If recovery threshold is met, automatically transitions to Recovering → Enabled.
    pub fn health_check(&self) -> Result<ProbeResult> {
        // Only perform health checks in Fallback or Recovering state
        let current_state = self.state();
        if current_state != ProxyState::Fallback && current_state != ProxyState::Recovering {
            tracing::debug!(
                "Health check skipped: not in fallback or recovering state (current: {:?})",
                current_state
            );
            return Ok(ProbeResult::Skipped {
                remaining_seconds: 0,
            });
        }

        // Get connector for probing
        let connector = self.get_connector()?;

        // Perform probe
        let result = {
            let mut health_checker = self.health_checker.write().unwrap();
            health_checker.probe(connector.as_ref())
        };

        // Emit health check event
        let event = match &result {
            ProbeResult::Success { latency_ms } => ProxyHealthCheckEvent::success(
                *latency_ms,
                self.sanitized_url(),
                "www.github.com:443".to_string(),
            ),
            ProbeResult::Failure { error } => ProxyHealthCheckEvent::failure(
                error.clone(),
                self.sanitized_url(),
                "www.github.com:443".to_string(),
            ),
            ProbeResult::Skipped { remaining_seconds } => ProxyHealthCheckEvent::failure(
                format!("Cooldown not expired: {remaining_seconds}s remaining"),
                self.sanitized_url(),
                "www.github.com:443".to_string(),
            ),
        };

        tracing::debug!(
            "Health check probe completed: success={}, response_time={:?}ms",
            event.success,
            event.response_time_ms
        );

        // TODO P5.6: Publish event to frontend
        // crate::events::publish_global(ProxyEvent::HealthCheck(event));

        // Check if recovery should be triggered
        if result.is_success() {
            let should_recover = {
                let health_checker = self.health_checker.read().unwrap();
                health_checker.should_recover()
            };

            if should_recover {
                self.trigger_automatic_recovery()?;
            }
        }

        Ok(result)
    }

    /// Trigger automatic recovery (P5.5)
    ///
    /// Internal method called when health check success threshold is met.
    /// Transitions state from Fallback → Recovering → Enabled.
    /// Emits `ProxyRecoveredEvent`.
    fn trigger_automatic_recovery(&self) -> Result<()> {
        let mut state = self.state.write().unwrap();

        // Get recovery info for event
        let consecutive_successes = {
            let health_checker = self.health_checker.read().unwrap();
            health_checker.consecutive_successes()
        };

        tracing::info!(
            "Automatic proxy recovery triggered: {} consecutive successes",
            consecutive_successes
        );

        // Transition to Recovering if not already
        if state.state == ProxyState::Fallback {
            state.transition(
                StateTransition::StartRecovery,
                Some(format!(
                    "Health check succeeded: {consecutive_successes} consecutive"
                )),
            )?;
            tracing::debug!("State transitioned to Recovering");
        }

        // Complete recovery immediately (health checks already validated proxy works)
        state.transition(
            StateTransition::CompleteRecovery,
            Some(format!(
                "Automatic recovery: {consecutive_successes} consecutive successes"
            )),
        )?;

        tracing::info!(
            "Automatic proxy recovery completed, state: {:?}",
            state.state
        );

        // Reset failure detector and health checker
        self.failure_detector.reset();
        {
            let mut health_checker = self.health_checker.write().unwrap();
            health_checker.reset();
        }

        // Emit recovery event
        let event = ProxyRecoveredEvent::automatic(consecutive_successes, self.sanitized_url(), {
            let config = self.config.read().unwrap();
            config.recovery_strategy.clone()
        });

        tracing::info!(
            "Proxy recovery event emitted: successful_checks={}, strategy={:?}",
            event.successful_checks,
            event.strategy
        );

        // TODO P5.6: Publish event to frontend
        // crate::events::publish_global(ProxyEvent::Recovered(event));

        Ok(())
    }

    /// Manually recover from fallback
    ///
    /// This is for manual intervention or testing. P5.5 adds automatic recovery.
    pub fn manual_recover(&self) -> Result<()> {
        let mut state = self.state.write().unwrap();

        tracing::info!(
            "Starting manual proxy recovery from state: {:?}",
            state.state
        );

        // Start recovery process
        state.transition(
            StateTransition::StartRecovery,
            Some("Manual recovery requested".to_string()),
        )?;

        tracing::debug!("Recovery phase initiated, state: {:?}", state.state);

        // Immediately complete recovery
        state.transition(
            StateTransition::CompleteRecovery,
            Some("Manual recovery".to_string()),
        )?;

        // Reset failure detector and health checker on successful recovery
        self.failure_detector.reset();
        {
            let mut health_checker = self.health_checker.write().unwrap();
            health_checker.reset();
        }

        tracing::info!("Manual proxy recovery completed, state: {:?}", state.state);
        Ok(())
    }

    /// Force proxy fallback (P5.6 frontend command)
    ///
    /// Alias for `manual_fallback` for frontend convenience
    pub fn force_fallback(&mut self, reason: &str) -> Result<()> {
        self.manual_fallback(reason)
    }

    /// Force proxy recovery (P5.6 frontend command)
    ///
    /// Alias for `manual_recover` for frontend convenience
    pub fn force_recovery(&mut self) -> Result<()> {
        self.manual_recover()
    }

    /// Get health check interval for periodic scheduling (P5.5)
    pub fn health_check_interval(&self) -> std::time::Duration {
        let health_checker = self.health_checker.read().unwrap();
        health_checker.interval()
    }

    /// Check if health checker is in cooldown period (P5.5)
    pub fn is_in_cooldown(&self) -> bool {
        let health_checker = self.health_checker.read().unwrap();
        !health_checker.is_cooldown_expired()
    }

    /// Get remaining cooldown seconds (P5.5)
    pub fn remaining_cooldown_seconds(&self) -> u64 {
        let health_checker = self.health_checker.read().unwrap();
        health_checker.remaining_cooldown_seconds()
    }

    /// Get current state context for diagnostics
    pub fn get_state_context(&self) -> ProxyStateContext {
        self.state.read().unwrap().clone()
    }

    /// Get current failure statistics (P5.4)
    ///
    /// Returns statistics from the failure detector including:
    /// - Total attempts in window
    /// - Number of failures
    /// - Current failure rate
    /// - Whether fallback was triggered
    pub fn get_failure_stats(&self) -> super::detector::FailureStats {
        self.failure_detector.get_stats()
    }
}

impl Default for ProxyManager {
    fn default() -> Self {
        Self::new(ProxyConfig::default())
    }
}

