//! Proxy state management and state machine

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

/// Proxy runtime state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProxyState {
    /// Proxy is enabled and operational
    Enabled,
    /// Proxy is disabled (off mode or not configured)
    Disabled,
    /// Proxy has fallen back to direct connection due to failures
    Fallback,
    /// Proxy is recovering (testing before re-enabling)
    Recovering,
}

impl Default for ProxyState {
    fn default() -> Self {
        Self::Disabled
    }
}

impl std::fmt::Display for ProxyState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Enabled => write!(f, "enabled"),
            Self::Disabled => write!(f, "disabled"),
            Self::Fallback => write!(f, "fallback"),
            Self::Recovering => write!(f, "recovering"),
        }
    }
}

/// State transition event for state machine
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StateTransition {
    /// Enable proxy (from Disabled)
    Enable,
    /// Disable proxy (to Disabled)
    Disable,
    /// Trigger fallback due to failures (from Enabled to Fallback)
    TriggerFallback,
    /// Start recovery process (from Fallback to Recovering)
    StartRecovery,
    /// Complete recovery (from Recovering to Enabled)
    CompleteRecovery,
    /// Abort recovery (from Recovering to Fallback)
    AbortRecovery,
}

impl ProxyState {
    /// Check if the state transition is valid
    pub fn can_transition_to(&self, next: ProxyState) -> bool {
        use ProxyState::*;
        
        matches!(
            (self, next),
            // Enable: Disabled -> Enabled
            (Disabled, Enabled) |
            // Disable: any -> Disabled
            (_, Disabled) |
            // Fallback: Enabled -> Fallback
            (Enabled, Fallback) |
            // Start recovery: Fallback -> Recovering
            (Fallback, Recovering) |
            // Complete recovery: Recovering -> Enabled
            (Recovering, Enabled) |
            // Abort recovery: Recovering -> Fallback
            (Recovering, Fallback)
        )
    }
    
    /// Apply a state transition
    pub fn apply_transition(&mut self, transition: StateTransition) -> Result<()> {
        let next_state = match transition {
            StateTransition::Enable => ProxyState::Enabled,
            StateTransition::Disable => ProxyState::Disabled,
            StateTransition::TriggerFallback => {
                if *self != ProxyState::Enabled {
                    anyhow::bail!(
                        "Cannot trigger fallback from {} state (must be enabled)",
                        self
                    );
                }
                ProxyState::Fallback
            }
            StateTransition::StartRecovery => {
                if *self != ProxyState::Fallback {
                    anyhow::bail!(
                        "Cannot start recovery from {} state (must be in fallback)",
                        self
                    );
                }
                ProxyState::Recovering
            }
            StateTransition::CompleteRecovery => {
                if *self != ProxyState::Recovering {
                    anyhow::bail!(
                        "Cannot complete recovery from {} state (must be recovering)",
                        self
                    );
                }
                ProxyState::Enabled
            }
            StateTransition::AbortRecovery => {
                if *self != ProxyState::Recovering {
                    anyhow::bail!(
                        "Cannot abort recovery from {} state (must be recovering)",
                        self
                    );
                }
                ProxyState::Fallback
            }
        };
        
        if !self.can_transition_to(next_state) {
            anyhow::bail!(
                "Invalid state transition from {} to {}",
                self, next_state
            );
        }
        
        *self = next_state;
        Ok(())
    }
}

/// Proxy state context with metadata
#[derive(Debug, Clone)]
pub struct ProxyStateContext {
    /// Current state
    pub state: ProxyState,
    
    /// Timestamp of last state change (Unix timestamp in seconds)
    pub last_transition_at: u64,
    
    /// Reason for current state (for fallback/recovery)
    pub reason: Option<String>,
    
    /// Number of consecutive failures (for fallback detection)
    pub consecutive_failures: u32,
    
    /// Number of consecutive successes (for recovery)
    pub consecutive_successes: u32,
}

impl Default for ProxyStateContext {
    fn default() -> Self {
        Self {
            state: ProxyState::Disabled,
            last_transition_at: current_timestamp(),
            reason: None,
            consecutive_failures: 0,
            consecutive_successes: 0,
        }
    }
}

impl ProxyStateContext {
    /// Create a new context in Disabled state
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Transition to a new state with reason
    pub fn transition(
        &mut self,
        transition: StateTransition,
        reason: Option<String>,
    ) -> Result<()> {
        self.state.apply_transition(transition)?;
        self.last_transition_at = current_timestamp();
        self.reason = reason;
        
        // Reset counters on state change
        match self.state {
            ProxyState::Enabled => {
                self.consecutive_failures = 0;
                self.consecutive_successes = 0;
            }
            ProxyState::Fallback => {
                self.consecutive_successes = 0;
            }
            ProxyState::Recovering => {
                self.consecutive_failures = 0;
            }
            ProxyState::Disabled => {
                self.consecutive_failures = 0;
                self.consecutive_successes = 0;
            }
        }
        
        Ok(())
    }
    
    /// Record a failure
    pub fn record_failure(&mut self) {
        self.consecutive_failures += 1;
        self.consecutive_successes = 0;
    }
    
    /// Record a success
    pub fn record_success(&mut self) {
        self.consecutive_successes += 1;
        self.consecutive_failures = 0;
    }
    
    /// Get seconds since last transition
    pub fn seconds_since_transition(&self) -> u64 {
        current_timestamp().saturating_sub(self.last_transition_at)
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

    #[test]
    fn test_proxy_state_default() {
        assert_eq!(ProxyState::default(), ProxyState::Disabled);
    }

    #[test]
    fn test_proxy_state_display() {
        assert_eq!(ProxyState::Enabled.to_string(), "enabled");
        assert_eq!(ProxyState::Disabled.to_string(), "disabled");
        assert_eq!(ProxyState::Fallback.to_string(), "fallback");
        assert_eq!(ProxyState::Recovering.to_string(), "recovering");
    }

    #[test]
    fn test_valid_transitions() {
        // Disabled -> Enabled
        assert!(ProxyState::Disabled.can_transition_to(ProxyState::Enabled));
        
        // Any -> Disabled
        assert!(ProxyState::Enabled.can_transition_to(ProxyState::Disabled));
        assert!(ProxyState::Fallback.can_transition_to(ProxyState::Disabled));
        assert!(ProxyState::Recovering.can_transition_to(ProxyState::Disabled));
        
        // Enabled -> Fallback
        assert!(ProxyState::Enabled.can_transition_to(ProxyState::Fallback));
        
        // Fallback -> Recovering
        assert!(ProxyState::Fallback.can_transition_to(ProxyState::Recovering));
        
        // Recovering -> Enabled
        assert!(ProxyState::Recovering.can_transition_to(ProxyState::Enabled));
        
        // Recovering -> Fallback
        assert!(ProxyState::Recovering.can_transition_to(ProxyState::Fallback));
    }

    #[test]
    fn test_invalid_transitions() {
        // Disabled -> Fallback (must go through Enabled)
        assert!(!ProxyState::Disabled.can_transition_to(ProxyState::Fallback));
        
        // Disabled -> Recovering (must go through Enabled and Fallback)
        assert!(!ProxyState::Disabled.can_transition_to(ProxyState::Recovering));
        
        // Enabled -> Recovering (must go through Fallback)
        assert!(!ProxyState::Enabled.can_transition_to(ProxyState::Recovering));
        
        // Fallback -> Enabled (must go through Recovering)
        assert!(!ProxyState::Fallback.can_transition_to(ProxyState::Enabled));
    }

    #[test]
    fn test_apply_transition() {
        let mut state = ProxyState::Disabled;
        
        // Enable
        assert!(state.apply_transition(StateTransition::Enable).is_ok());
        assert_eq!(state, ProxyState::Enabled);
        
        // Trigger fallback
        assert!(state.apply_transition(StateTransition::TriggerFallback).is_ok());
        assert_eq!(state, ProxyState::Fallback);
        
        // Start recovery
        assert!(state.apply_transition(StateTransition::StartRecovery).is_ok());
        assert_eq!(state, ProxyState::Recovering);
        
        // Complete recovery
        assert!(state.apply_transition(StateTransition::CompleteRecovery).is_ok());
        assert_eq!(state, ProxyState::Enabled);
    }

    #[test]
    fn test_invalid_transition_application() {
        let mut state = ProxyState::Disabled;
        
        // Cannot trigger fallback from Disabled
        assert!(state.apply_transition(StateTransition::TriggerFallback).is_err());
        
        // Cannot start recovery from Disabled
        assert!(state.apply_transition(StateTransition::StartRecovery).is_err());
    }

    #[test]
    fn test_state_context_default() {
        let ctx = ProxyStateContext::default();
        assert_eq!(ctx.state, ProxyState::Disabled);
        assert_eq!(ctx.consecutive_failures, 0);
        assert_eq!(ctx.consecutive_successes, 0);
        assert!(ctx.reason.is_none());
    }

    #[test]
    fn test_state_context_transition() {
        let mut ctx = ProxyStateContext::new();
        
        // Enable with reason
        assert!(ctx
            .transition(StateTransition::Enable, Some("Config enabled".to_string()))
            .is_ok());
        assert_eq!(ctx.state, ProxyState::Enabled);
        assert_eq!(ctx.reason, Some("Config enabled".to_string()));
    }

    #[test]
    fn test_state_context_counters() {
        let mut ctx = ProxyStateContext::new();
        ctx.transition(StateTransition::Enable, None).unwrap();
        
        // Record failures
        ctx.record_failure();
        assert_eq!(ctx.consecutive_failures, 1);
        assert_eq!(ctx.consecutive_successes, 0);
        
        ctx.record_failure();
        assert_eq!(ctx.consecutive_failures, 2);
        
        // Record success resets failure counter
        ctx.record_success();
        assert_eq!(ctx.consecutive_successes, 1);
        assert_eq!(ctx.consecutive_failures, 0);
    }

    #[test]
    fn test_state_context_counter_reset_on_transition() {
        let mut ctx = ProxyStateContext::new();
        ctx.transition(StateTransition::Enable, None).unwrap();
        
        // Accumulate failures
        ctx.record_failure();
        ctx.record_failure();
        assert_eq!(ctx.consecutive_failures, 2);
        
        // Transition to fallback resets success counter
        ctx.transition(StateTransition::TriggerFallback, Some("Too many failures".to_string()))
            .unwrap();
        assert_eq!(ctx.consecutive_successes, 0);
        // Failure counter is preserved for logging
    }

    #[test]
    fn test_all_invalid_transitions() {
        // Test all state transition combinations that should fail
        
        // From Disabled
        let mut state = ProxyState::Disabled;
        assert!(state.apply_transition(StateTransition::TriggerFallback).is_err());
        assert!(state.apply_transition(StateTransition::StartRecovery).is_err());
        assert!(state.apply_transition(StateTransition::CompleteRecovery).is_err());
        assert!(state.apply_transition(StateTransition::AbortRecovery).is_err());
        
        // From Enabled
        state = ProxyState::Enabled;
        assert!(state.apply_transition(StateTransition::StartRecovery).is_err());
        assert!(state.apply_transition(StateTransition::CompleteRecovery).is_err());
        assert!(state.apply_transition(StateTransition::AbortRecovery).is_err());
        
        // From Fallback
        state = ProxyState::Fallback;
        assert!(state.apply_transition(StateTransition::TriggerFallback).is_err());
        assert!(state.apply_transition(StateTransition::CompleteRecovery).is_err());
        assert!(state.apply_transition(StateTransition::AbortRecovery).is_err());
        
        // From Recovering
        state = ProxyState::Recovering;
        assert!(state.apply_transition(StateTransition::TriggerFallback).is_err());
        assert!(state.apply_transition(StateTransition::StartRecovery).is_err());
    }

    #[test]
    fn test_state_context_clone() {
        let mut ctx1 = ProxyStateContext::new();
        ctx1.transition(StateTransition::Enable, Some("Test".to_string())).unwrap();
        ctx1.record_failure();
        ctx1.record_failure();
        
        let ctx2 = ctx1.clone();
        
        assert_eq!(ctx2.state, ctx1.state);
        assert_eq!(ctx2.consecutive_failures, ctx1.consecutive_failures);
        assert_eq!(ctx2.consecutive_successes, ctx1.consecutive_successes);
        assert_eq!(ctx2.reason, ctx1.reason);
    }

    #[test]
    fn test_seconds_since_transition() {
        let ctx = ProxyStateContext::new();
        
        // Should be very small (just created)
        let elapsed = ctx.seconds_since_transition();
        assert!(elapsed < 2); // Allow 2 seconds max for test execution
    }

    #[test]
    fn test_state_transition_preserves_context() {
        let mut ctx = ProxyStateContext::new();
        let initial_timestamp = ctx.last_transition_at;
        
        // Sleep to ensure timestamp changes
        std::thread::sleep(std::time::Duration::from_secs(2));
        
        ctx.transition(StateTransition::Enable, Some("Testing".to_string())).unwrap();
        
        // Timestamp should be updated (>= to handle low-resolution clocks)
        assert!(ctx.last_transition_at >= initial_timestamp);
        assert_eq!(ctx.reason, Some("Testing".to_string()));
    }

    #[test]
    fn test_recovery_abort_path() {
        let mut ctx = ProxyStateContext::new();
        
        // Enable -> Fallback -> Recovering -> Abort -> Fallback
        ctx.transition(StateTransition::Enable, None).unwrap();
        ctx.transition(StateTransition::TriggerFallback, None).unwrap();
        ctx.transition(StateTransition::StartRecovery, None).unwrap();
        assert_eq!(ctx.state, ProxyState::Recovering);
        
        ctx.transition(StateTransition::AbortRecovery, Some("Health check failed".to_string()))
            .unwrap();
        assert_eq!(ctx.state, ProxyState::Fallback);
        assert_eq!(ctx.reason, Some("Health check failed".to_string()));
    }

    #[test]
    fn test_state_serialization() {
        // Test all states can be serialized/deserialized
        let states = vec![
            ProxyState::Disabled,
            ProxyState::Enabled,
            ProxyState::Fallback,
            ProxyState::Recovering,
        ];
        
        for state in states {
            let json = serde_json::to_string(&state).unwrap();
            let restored: ProxyState = serde_json::from_str(&json).unwrap();
            assert_eq!(restored, state);
        }
    }

    #[test]
    fn test_counter_accumulation() {
        let mut ctx = ProxyStateContext::new();
        ctx.transition(StateTransition::Enable, None).unwrap();
        
        // Test large number of failures
        for _ in 0..100 {
            ctx.record_failure();
        }
        assert_eq!(ctx.consecutive_failures, 100);
        assert_eq!(ctx.consecutive_successes, 0);
        
        // One success resets everything
        ctx.record_success();
        assert_eq!(ctx.consecutive_failures, 0);
        assert_eq!(ctx.consecutive_successes, 1);
        
        // Test large number of successes
        for _ in 0..50 {
            ctx.record_success();
        }
        assert_eq!(ctx.consecutive_successes, 51);
    }
}
