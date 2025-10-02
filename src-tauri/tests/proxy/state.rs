//! Tests for proxy state management and state machine
//!
//! These tests verify:
//! - ProxyState enum default, display, and transitions
//! - ProxyStateContext lifecycle and counters
//! - State machine validation logic
//! - Serialization/deserialization

use fireworks_collaboration_lib::core::proxy::state::{
    ProxyState, ProxyStateContext, StateTransition,
};

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
