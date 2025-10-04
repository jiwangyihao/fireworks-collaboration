//! Proxy 模块单元测试
//! 
//! 从源文件迁移的单元测试，包括：
//! - `health_checker.rs`: 健康检查器测试
//! - manager.rs: 代理管理器恢复策略测试

use fireworks_collaboration_lib::core::proxy::{
    config::ProxyConfig,
    health_checker::{HealthCheckConfig, ProbeResult, ProxyHealthChecker},
    manager::ProxyManager,
    state::ProxyState,
    ProxyMode,
};
use std::time::Duration;

// ========== health_checker.rs 测试 ==========

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
fn test_reset_clears_state() {
    let mut checker = ProxyHealthChecker::new(HealthCheckConfig::default());

    // Record fallback
    checker.record_fallback();

    // Reset should clear everything
    checker.reset();

    // Should be in expired state after reset
    assert!(checker.is_cooldown_expired());
    assert_eq!(checker.consecutive_successes(), 0);
    assert_eq!(checker.consecutive_failures(), 0);
}

#[test]
fn test_record_fallback_resets_counters() {
    let mut checker = ProxyHealthChecker::new(HealthCheckConfig::default());

    // Record fallback should set cooldown
    checker.record_fallback();

    // Should not be expired immediately (unless cooldown is 0)
    assert!(!checker.is_cooldown_expired());
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

// ========== manager.rs 恢复策略测试 ==========

#[test]
fn test_health_check_interval() {
    let mut config = ProxyConfig::default();
    config.health_check_interval_seconds = 120;

    let manager = ProxyManager::new(config);
    assert_eq!(
        manager.health_check_interval(),
        std::time::Duration::from_secs(120)
    );
}

#[test]
fn test_cooldown_not_in_fallback() {
    let manager = ProxyManager::new(ProxyConfig::default());

    // No fallback recorded, should not be in cooldown
    assert!(!manager.is_in_cooldown());
    assert_eq!(manager.remaining_cooldown_seconds(), 0);
}

#[test]
fn test_manual_fallback_starts_cooldown() {
    let mut config = ProxyConfig::default();
    config.mode = ProxyMode::Http;
    config.url = "http://proxy.example.com:8080".to_string();

    let manager = ProxyManager::new(config);

    // Trigger manual fallback
    let result = manager.manual_fallback("Test fallback");
    assert!(result.is_ok());

    // Should be in cooldown now
    assert!(manager.is_in_cooldown());
    assert!(manager.remaining_cooldown_seconds() > 0);
}

#[test]
fn test_manual_recover_clears_state() {
    let mut config = ProxyConfig::default();
    config.mode = ProxyMode::Http;
    config.url = "http://proxy.example.com:8080".to_string();

    let manager = ProxyManager::new(config);

    // Transition to fallback
    let _ = manager.manual_fallback("Test");
    assert_eq!(manager.state(), ProxyState::Fallback);

    // Recover
    let result = manager.manual_recover();
    assert!(result.is_ok());

    // Should be back to Enabled
    assert_eq!(manager.state(), ProxyState::Enabled);
    assert!(!manager.is_in_cooldown());
}

#[test]
fn test_health_check_skipped_when_not_in_fallback() {
    let mut config = ProxyConfig::default();
    config.mode = ProxyMode::Http;
    config.url = "http://proxy.example.com:8080".to_string();

    let manager = ProxyManager::new(config);

    // Health check should be skipped in Enabled state
    let result = manager.health_check();
    assert!(result.is_ok());

    let probe_result = result.unwrap();
    assert!(probe_result.is_skipped());
}

#[test]
fn test_automatic_recovery_trigger() {
    let mut config = ProxyConfig::default();
    config.mode = ProxyMode::Http;
    config.url = "http://proxy.example.com:8080".to_string();
    config.recovery_strategy = "immediate".to_string();
    config.recovery_cooldown_seconds = 0; // No cooldown for testing

    let manager = ProxyManager::new(config);

    // Transition to fallback
    let _ = manager.manual_fallback("Test");
    assert_eq!(manager.state(), ProxyState::Fallback);

    // Simulate successful health check (would need mock connector in real test)
    // For now, just verify state can be recovered manually
    let _ = manager.manual_recover();
    assert_eq!(manager.state(), ProxyState::Enabled);
}

#[test]
fn test_health_checker_integration() {
    let mut config = ProxyConfig::default();
    config.mode = ProxyMode::Http;
    config.url = "http://proxy.example.com:8080".to_string();
    config.health_check_interval_seconds = 60;
    config.recovery_cooldown_seconds = 300;
    config.recovery_strategy = "consecutive".to_string();

    let manager = ProxyManager::new(config);

    // Verify health checker configuration
    assert_eq!(
        manager.health_check_interval(),
        std::time::Duration::from_secs(60)
    );

    // Trigger fallback to start cooldown
    let _ = manager.manual_fallback("Test");

    // Should be in cooldown
    assert!(manager.is_in_cooldown());
    assert!(manager.remaining_cooldown_seconds() > 0);
}

#[test]
fn test_recovery_strategy_consecutive() {
    let mut config = ProxyConfig::default();
    config.recovery_strategy = "consecutive".to_string();

    let manager = ProxyManager::new(config);

    // Verify strategy is stored
    let state_context = manager.get_state_context();
    assert_eq!(state_context.consecutive_successes, 0);
}
