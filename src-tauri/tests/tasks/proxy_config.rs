//! Proxy configuration and state unit tests
//!
//! Tests for ProxyConfig, ProxyMode, ProxyState, StateTransition, and ProxyStateContext.

use std::time::Duration;

use fireworks_collaboration_lib::core::proxy::config::{ProxyConfig, ProxyMode};
use fireworks_collaboration_lib::core::proxy::state::{
    ProxyState, ProxyStateContext, StateTransition,
};

// ============ ProxyMode Tests ============

#[test]
fn test_proxy_mode_default() {
    let mode = ProxyMode::default();
    assert_eq!(mode, ProxyMode::Off);
}

#[test]
fn test_proxy_mode_display() {
    assert_eq!(format!("{}", ProxyMode::Off), "off");
    assert_eq!(format!("{}", ProxyMode::Http), "http");
    assert_eq!(format!("{}", ProxyMode::Socks5), "socks5");
    assert_eq!(format!("{}", ProxyMode::System), "system");
}

// ============ ProxyConfig Tests ============

#[test]
fn test_proxy_config_default() {
    let config = ProxyConfig::default();
    assert_eq!(config.mode, ProxyMode::Off);
    assert!(config.url.is_empty());
    assert_eq!(config.timeout_seconds, 30);
}

#[test]
fn test_proxy_config_timeout_as_duration() {
    let config = ProxyConfig {
        timeout_seconds: 45,
        ..ProxyConfig::default()
    };
    assert_eq!(config.timeout(), Duration::from_secs(45));
}

#[test]
fn test_proxy_config_is_enabled_off() {
    let config = ProxyConfig {
        mode: ProxyMode::Off,
        ..ProxyConfig::default()
    };
    assert!(!config.is_enabled());
}

#[test]
fn test_proxy_config_is_enabled_http_empty_url() {
    let config = ProxyConfig {
        mode: ProxyMode::Http,
        url: String::new(),
        ..ProxyConfig::default()
    };
    assert!(!config.is_enabled());
}

#[test]
fn test_proxy_config_is_enabled_http_with_url() {
    let config = ProxyConfig {
        mode: ProxyMode::Http,
        url: "http://proxy.example.com:8080".to_string(),
        ..ProxyConfig::default()
    };
    assert!(config.is_enabled());
}

#[test]
fn test_proxy_config_is_enabled_system_empty_url() {
    let config = ProxyConfig {
        mode: ProxyMode::System,
        url: String::new(),
        ..ProxyConfig::default()
    };
    // System mode allows empty URL
    assert!(config.is_enabled());
}

#[test]
fn test_proxy_config_validate_off_mode() {
    let config = ProxyConfig::default();
    assert!(config.validate().is_ok());
}

#[test]
fn test_proxy_config_validate_http_valid_url() {
    let config = ProxyConfig {
        mode: ProxyMode::Http,
        url: "http://proxy.example.com:8080".to_string(),
        ..ProxyConfig::default()
    };
    assert!(config.validate().is_ok());
}

#[test]
fn test_proxy_config_validate_http_invalid_url() {
    let config = ProxyConfig {
        mode: ProxyMode::Http,
        url: "not-a-valid-url".to_string(),
        ..ProxyConfig::default()
    };
    assert!(config.validate().is_err());
}

#[test]
fn test_proxy_config_validate_timeout_zero() {
    let config = ProxyConfig {
        mode: ProxyMode::System, // Use System mode to trigger validation
        timeout_seconds: 0,
        ..ProxyConfig::default()
    };
    assert!(config.validate().is_err());
}

#[test]
fn test_proxy_config_validate_timeout_too_large() {
    let config = ProxyConfig {
        mode: ProxyMode::System, // Use System mode to trigger validation
        timeout_seconds: 400,    // > 300s max
        ..ProxyConfig::default()
    };
    assert!(config.validate().is_err());
}

#[test]
fn test_proxy_config_sanitized_url_no_password() {
    let config = ProxyConfig {
        url: "http://proxy.example.com:8080".to_string(),
        ..ProxyConfig::default()
    };
    assert_eq!(config.sanitized_url(), "http://proxy.example.com:8080");
}

#[test]
fn test_proxy_config_sanitized_url_with_password() {
    let config = ProxyConfig {
        url: "http://user:secret@proxy.example.com:8080".to_string(),
        ..ProxyConfig::default()
    };
    let sanitized = config.sanitized_url();
    assert!(!sanitized.contains("secret"));
    assert!(sanitized.contains("*"));
}

// ============ ProxyState Tests ============

#[test]
fn test_proxy_state_default() {
    let state = ProxyState::default();
    assert_eq!(state, ProxyState::Disabled);
}

#[test]
fn test_proxy_state_display() {
    assert_eq!(format!("{}", ProxyState::Enabled), "enabled");
    assert_eq!(format!("{}", ProxyState::Disabled), "disabled");
    assert_eq!(format!("{}", ProxyState::Fallback), "fallback");
    assert_eq!(format!("{}", ProxyState::Recovering), "recovering");
}

#[test]
fn test_proxy_state_can_transition_disabled_to_enabled() {
    let state = ProxyState::Disabled;
    assert!(state.can_transition_to(ProxyState::Enabled));
}

#[test]
fn test_proxy_state_can_transition_enabled_to_disabled() {
    let state = ProxyState::Enabled;
    assert!(state.can_transition_to(ProxyState::Disabled));
}

#[test]
fn test_proxy_state_can_transition_enabled_to_fallback() {
    let state = ProxyState::Enabled;
    assert!(state.can_transition_to(ProxyState::Fallback));
}

#[test]
fn test_proxy_state_can_transition_fallback_to_recovering() {
    let state = ProxyState::Fallback;
    assert!(state.can_transition_to(ProxyState::Recovering));
}

#[test]
fn test_proxy_state_can_transition_recovering_to_enabled() {
    let state = ProxyState::Recovering;
    assert!(state.can_transition_to(ProxyState::Enabled));
}

#[test]
fn test_proxy_state_can_transition_recovering_to_fallback() {
    let state = ProxyState::Recovering;
    assert!(state.can_transition_to(ProxyState::Fallback));
}

#[test]
fn test_proxy_state_cannot_transition_disabled_to_fallback() {
    let state = ProxyState::Disabled;
    assert!(!state.can_transition_to(ProxyState::Fallback));
}

#[test]
fn test_proxy_state_apply_transition_enable() {
    let mut state = ProxyState::Disabled;
    assert!(state.apply_transition(StateTransition::Enable).is_ok());
    assert_eq!(state, ProxyState::Enabled);
}

#[test]
fn test_proxy_state_apply_transition_disable() {
    let mut state = ProxyState::Enabled;
    assert!(state.apply_transition(StateTransition::Disable).is_ok());
    assert_eq!(state, ProxyState::Disabled);
}

#[test]
fn test_proxy_state_apply_transition_trigger_fallback() {
    let mut state = ProxyState::Enabled;
    assert!(state
        .apply_transition(StateTransition::TriggerFallback)
        .is_ok());
    assert_eq!(state, ProxyState::Fallback);
}

#[test]
fn test_proxy_state_apply_transition_trigger_fallback_invalid() {
    let mut state = ProxyState::Disabled;
    assert!(state
        .apply_transition(StateTransition::TriggerFallback)
        .is_err());
}

#[test]
fn test_proxy_state_apply_transition_start_recovery() {
    let mut state = ProxyState::Fallback;
    assert!(state
        .apply_transition(StateTransition::StartRecovery)
        .is_ok());
    assert_eq!(state, ProxyState::Recovering);
}

#[test]
fn test_proxy_state_apply_transition_complete_recovery() {
    let mut state = ProxyState::Recovering;
    assert!(state
        .apply_transition(StateTransition::CompleteRecovery)
        .is_ok());
    assert_eq!(state, ProxyState::Enabled);
}

#[test]
fn test_proxy_state_apply_transition_abort_recovery() {
    let mut state = ProxyState::Recovering;
    assert!(state
        .apply_transition(StateTransition::AbortRecovery)
        .is_ok());
    assert_eq!(state, ProxyState::Fallback);
}

// ============ ProxyStateContext Tests ============

#[test]
fn test_proxy_state_context_new() {
    let ctx = ProxyStateContext::new();
    assert_eq!(ctx.state, ProxyState::Disabled);
    assert_eq!(ctx.consecutive_failures, 0);
    assert_eq!(ctx.consecutive_successes, 0);
    assert!(ctx.reason.is_none());
}

#[test]
fn test_proxy_state_context_transition() {
    let mut ctx = ProxyStateContext::new();
    let result = ctx.transition(StateTransition::Enable, Some("user request".to_string()));
    assert!(result.is_ok());
    assert_eq!(ctx.state, ProxyState::Enabled);
    assert_eq!(ctx.reason, Some("user request".to_string()));
}

#[test]
fn test_proxy_state_context_record_failure() {
    let mut ctx = ProxyStateContext::new();
    ctx.transition(StateTransition::Enable, None).unwrap();

    ctx.record_failure();
    assert_eq!(ctx.consecutive_failures, 1);
    assert_eq!(ctx.consecutive_successes, 0);

    ctx.record_failure();
    assert_eq!(ctx.consecutive_failures, 2);
}

#[test]
fn test_proxy_state_context_record_success() {
    let mut ctx = ProxyStateContext::new();
    ctx.transition(StateTransition::Enable, None).unwrap();

    ctx.record_success();
    assert_eq!(ctx.consecutive_successes, 1);
    assert_eq!(ctx.consecutive_failures, 0);

    ctx.record_success();
    assert_eq!(ctx.consecutive_successes, 2);
}

#[test]
fn test_proxy_state_context_failure_resets_success() {
    let mut ctx = ProxyStateContext::new();
    ctx.transition(StateTransition::Enable, None).unwrap();

    ctx.record_success();
    ctx.record_success();
    assert_eq!(ctx.consecutive_successes, 2);

    ctx.record_failure();
    assert_eq!(ctx.consecutive_failures, 1);
    assert_eq!(ctx.consecutive_successes, 0);
}

#[test]
fn test_proxy_state_context_success_resets_failure() {
    let mut ctx = ProxyStateContext::new();
    ctx.transition(StateTransition::Enable, None).unwrap();

    ctx.record_failure();
    ctx.record_failure();
    assert_eq!(ctx.consecutive_failures, 2);

    ctx.record_success();
    assert_eq!(ctx.consecutive_successes, 1);
    assert_eq!(ctx.consecutive_failures, 0);
}

#[test]
fn test_proxy_state_context_transition_resets_counters() {
    let mut ctx = ProxyStateContext::new();
    ctx.transition(StateTransition::Enable, None).unwrap();

    ctx.record_failure();
    ctx.record_failure();
    assert_eq!(ctx.consecutive_failures, 2);

    // Transition to fallback should reset successes
    ctx.transition(StateTransition::TriggerFallback, Some("test".to_string()))
        .unwrap();
    assert_eq!(ctx.consecutive_successes, 0);
}
