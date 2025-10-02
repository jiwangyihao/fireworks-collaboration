use fireworks_collaboration_lib::core::ip_pool::circuit_breaker::{
    CircuitBreaker, CircuitBreakerConfig, CircuitState,
};
use std::net::IpAddr;

#[test]
fn consecutive_failures_trigger_circuit_open() {
    let breaker = CircuitBreaker::new(CircuitBreakerConfig {
        enabled: true,
        consecutive_failure_threshold: 3,
        ..Default::default()
    });

    let ip = "192.0.2.1".parse().unwrap();

    // 前两次失败不应触发
    breaker.record_failure(ip);
    assert!(!breaker.is_tripped(ip));
    breaker.record_failure(ip);
    assert!(!breaker.is_tripped(ip));

    // 第三次失败触发熔断
    breaker.record_failure(ip);
    assert!(breaker.is_tripped(ip));

    let stats = breaker.get_stats(ip).unwrap();
    assert_eq!(stats.state, CircuitState::Cooldown);
    assert_eq!(stats.consecutive_failures, 3);
}

#[test]
fn success_resets_consecutive_failures() {
    let breaker = CircuitBreaker::new(CircuitBreakerConfig {
        enabled: true,
        consecutive_failure_threshold: 3,
        failure_rate_threshold: 0.9, // 高阈值，避免失败率触发
        min_samples_in_window: 10,   // 高样本数要求
        ..Default::default()
    });

    let ip = "192.0.2.1".parse().unwrap();

    breaker.record_failure(ip);
    breaker.record_failure(ip);
    breaker.record_success(ip); // 重置连续失败计数
    breaker.record_failure(ip);
    breaker.record_failure(ip);

    // 不应触发，因为中间有成功，连续失败被重置
    assert!(!breaker.is_tripped(ip));
}

#[test]
fn failure_rate_triggers_circuit_open() {
    let breaker = CircuitBreaker::new(CircuitBreakerConfig {
        enabled: true,
        consecutive_failure_threshold: 100, // 不通过连续失败触发
        failure_rate_threshold: 0.5,
        min_samples_in_window: 5,
        ..Default::default()
    });

    let ip = "192.0.2.1".parse().unwrap();

    // 2 成功 + 3 失败 = 5 样本，失败率 60%
    breaker.record_success(ip);
    breaker.record_success(ip);
    breaker.record_failure(ip);
    breaker.record_failure(ip);
    breaker.record_failure(ip);

    // 应触发熔断
    assert!(breaker.is_tripped(ip));
}

#[test]
fn manual_reset_clears_circuit_state() {
    let breaker = CircuitBreaker::new(CircuitBreakerConfig {
        enabled: true,
        consecutive_failure_threshold: 2,
        ..Default::default()
    });

    let ip = "192.0.2.1".parse().unwrap();

    breaker.record_failure(ip);
    breaker.record_failure(ip);
    assert!(breaker.is_tripped(ip));

    breaker.reset_ip(ip);
    assert!(!breaker.is_tripped(ip));
}

#[test]
fn disabled_breaker_never_trips() {
    let breaker = CircuitBreaker::new(CircuitBreakerConfig {
        enabled: false,
        consecutive_failure_threshold: 1,
        ..Default::default()
    });

    let ip = "192.0.2.1".parse().unwrap();

    breaker.record_failure(ip);
    breaker.record_failure(ip);
    breaker.record_failure(ip);

    assert!(!breaker.is_tripped(ip));
}

#[test]
fn get_tripped_ips_returns_only_tripped() {
    let breaker = CircuitBreaker::new(CircuitBreakerConfig {
        enabled: true,
        consecutive_failure_threshold: 2,
        ..Default::default()
    });

    let ip1: IpAddr = "192.0.2.1".parse().unwrap();
    let ip2: IpAddr = "192.0.2.2".parse().unwrap();
    let ip3: IpAddr = "192.0.2.3".parse().unwrap();

    breaker.record_failure(ip1);
    breaker.record_failure(ip1); // ip1 熔断

    breaker.record_failure(ip2);
    breaker.record_success(ip2); // ip2 正常

    breaker.record_failure(ip3);
    breaker.record_failure(ip3); // ip3 熔断

    let tripped = breaker.get_tripped_ips();
    assert_eq!(tripped.len(), 2);
    assert!(tripped.contains(&ip1));
    assert!(tripped.contains(&ip3));
    assert!(!tripped.contains(&ip2));
}
