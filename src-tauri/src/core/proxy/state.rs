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
