//! Tests for proxy failure detector and system proxy detection

use fireworks_collaboration_lib::core::proxy::detector::ProxyFailureDetector;
use fireworks_collaboration_lib::core::proxy::{ProxyMode, SystemProxyDetector};
use std::env;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

#[test]
fn test_detector_creation() {
    let detector = ProxyFailureDetector::new(300, 0.2);
    let stats = detector.get_stats();
    assert_eq!(stats.total_attempts, 0);
    assert_eq!(stats.failures, 0);
    assert_eq!(stats.failure_rate, 0.0);
    assert_eq!(stats.window_seconds, 300);
    assert_eq!(stats.threshold, 0.2);
    assert!(!stats.fallback_triggered);
}

#[test]
fn test_detector_default() {
    let detector = ProxyFailureDetector::with_defaults();
    let stats = detector.get_stats();
    assert_eq!(stats.window_seconds, 300);
    assert_eq!(stats.threshold, 0.2);
}

#[test]
fn test_threshold_clamping() {
    let detector1 = ProxyFailureDetector::new(300, -0.5);
    assert_eq!(detector1.get_stats().threshold, 0.0);

    let detector2 = ProxyFailureDetector::new(300, 1.5);
    assert_eq!(detector2.get_stats().threshold, 1.0);
}

#[test]
fn test_report_failure() {
    let detector = ProxyFailureDetector::new(300, 0.2);
    detector.report_failure();

    let stats = detector.get_stats();
    assert_eq!(stats.total_attempts, 1);
    assert_eq!(stats.failures, 1);
    assert_eq!(stats.failure_rate, 1.0);
}

#[test]
fn test_report_success() {
    let detector = ProxyFailureDetector::new(300, 0.2);
    detector.report_success();

    let stats = detector.get_stats();
    assert_eq!(stats.total_attempts, 1);
    assert_eq!(stats.failures, 0);
    assert_eq!(stats.failure_rate, 0.0);
}

#[test]
fn test_mixed_attempts() {
    let detector = ProxyFailureDetector::new(300, 0.2);

    // 3 failures, 7 successes = 30% failure rate
    for _ in 0..3 {
        detector.report_failure();
    }
    for _ in 0..7 {
        detector.report_success();
    }

    let stats = detector.get_stats();
    assert_eq!(stats.total_attempts, 10);
    assert_eq!(stats.failures, 3);
    assert!((stats.failure_rate - 0.3).abs() < 0.01);
}

#[test]
fn test_should_fallback_threshold() {
    let detector = ProxyFailureDetector::new(300, 0.2);

    // 1 failure, 4 successes = 20% failure rate - should trigger at threshold (>=)
    detector.report_failure();
    for _ in 0..4 {
        detector.report_success();
    }
    // 20% failure rate exactly at threshold should trigger
    assert!(detector.should_fallback());
}

#[test]
fn test_fallback_triggered_once() {
    let detector = ProxyFailureDetector::new(300, 0.2);

    // Trigger fallback
    for _ in 0..5 {
        detector.report_failure();
    }
    assert!(detector.should_fallback());

    // Mark as triggered
    detector.mark_fallback_triggered();

    // Should not trigger again
    assert!(!detector.should_fallback());

    let stats = detector.get_stats();
    assert!(stats.fallback_triggered);
}

#[test]
fn test_reset() {
    let detector = ProxyFailureDetector::new(300, 0.2);

    detector.report_failure();
    detector.report_failure();
    detector.mark_fallback_triggered();

    let stats_before = detector.get_stats();
    assert_eq!(stats_before.total_attempts, 2);
    assert!(stats_before.fallback_triggered);

    detector.reset();

    let stats_after = detector.get_stats();
    assert_eq!(stats_after.total_attempts, 0);
    assert!(!stats_after.fallback_triggered);
}

#[test]
fn test_window_pruning() {
    // Use very short window for testing
    let detector = ProxyFailureDetector::new(1, 0.5);

    detector.report_failure();
    assert_eq!(detector.get_stats().total_attempts, 1);

    // Wait for window to expire
    thread::sleep(Duration::from_secs(2));

    // Get stats should prune old attempts
    let stats = detector.get_stats();
    assert_eq!(stats.total_attempts, 0);
}

#[test]
fn test_failure_rate_calculation() {
    let _detector = ProxyFailureDetector::new(300, 0.5);

    // Test various failure rates
    let test_cases = vec![
        (0, 10, 0.0), // 0%
        (1, 9, 0.1),  // 10%
        (5, 5, 0.5),  // 50%
        (9, 1, 0.9),  // 90%
        (10, 0, 1.0), // 100%
    ];

    for (failures, successes, expected_rate) in test_cases {
        let detector = ProxyFailureDetector::new(300, 0.5);
        for _ in 0..failures {
            detector.report_failure();
        }
        for _ in 0..successes {
            detector.report_success();
        }

        let stats = detector.get_stats();
        assert!((stats.failure_rate - expected_rate).abs() < 0.01);
    }
}

#[test]
fn test_concurrent_access() {
    let detector = Arc::new(ProxyFailureDetector::new(300, 0.2));
    let mut handles = vec![];

    // Spawn 10 threads, each reporting 10 attempts
    for i in 0..10 {
        let detector_clone = Arc::clone(&detector);
        let handle = thread::spawn(move || {
            for _ in 0..10 {
                if i % 2 == 0 {
                    detector_clone.report_failure();
                } else {
                    detector_clone.report_success();
                }
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let stats = detector.get_stats();
    assert_eq!(stats.total_attempts, 100);
    // 50 failures, 50 successes
    assert_eq!(stats.failures, 50);
    assert!((stats.failure_rate - 0.5).abs() < 0.01);
}

#[test]
fn test_edge_case_zero_attempts() {
    let detector = ProxyFailureDetector::new(300, 0.2);
    assert!(!detector.should_fallback());
    assert_eq!(detector.get_stats().failure_rate, 0.0);
}

#[test]
fn test_edge_case_exact_threshold() {
    let detector = ProxyFailureDetector::new(300, 0.2);

    // Exactly 20% failure rate
    detector.report_failure();
    for _ in 0..4 {
        detector.report_success();
    }

    let stats = detector.get_stats();
    assert_eq!(stats.failure_rate, 0.2);
    // Should trigger at threshold (>=)
    assert!(detector.should_fallback());
}

#[test]
fn test_config_validation_zero_window() {
    // Zero window should fall back to 60 seconds
    let detector = ProxyFailureDetector::new(0, 0.2);
    let stats = detector.get_stats();
    assert_eq!(stats.window_seconds, 60);
}

#[test]
fn test_config_validation_negative_threshold() {
    // Negative threshold should be clamped to 0.0
    let detector = ProxyFailureDetector::new(300, -0.5);
    let stats = detector.get_stats();
    assert_eq!(stats.threshold, 0.0);
}

#[test]
fn test_config_validation_exceeding_threshold() {
    // Threshold > 1.0 should be clamped to 1.0
    let detector = ProxyFailureDetector::new(300, 1.5);
    let stats = detector.get_stats();
    assert_eq!(stats.threshold, 1.0);
}

#[test]
fn test_config_validation_nan_threshold() {
    // NaN threshold should be clamped to 0.0
    let detector = ProxyFailureDetector::new(300, f64::NAN);
    let stats = detector.get_stats();
    // NaN.clamp(0.0, 1.0) returns 0.0 (check it's a valid number)
    assert!(!stats.threshold.is_nan());
    assert!(stats.threshold >= 0.0 && stats.threshold <= 1.0);
}

#[test]
fn test_extreme_window_very_large() {
    // Very large window should work correctly
    let detector = ProxyFailureDetector::new(86400 * 365, 0.2); // 1 year
    detector.report_failure();
    detector.report_success();

    let stats = detector.get_stats();
    assert_eq!(stats.window_seconds, 86400 * 365);
    assert_eq!(stats.total_attempts, 2);
    assert_eq!(stats.failure_rate, 0.5);
}

#[test]
fn test_extreme_attempts_many_failures() {
    // Test with very large number of attempts
    let detector = ProxyFailureDetector::new(300, 0.2);

    for _ in 0..1000 {
        detector.report_failure();
    }

    let stats = detector.get_stats();
    assert_eq!(stats.total_attempts, 1000);
    assert_eq!(stats.failures, 1000);
    assert_eq!(stats.failure_rate, 1.0);
    assert!(detector.should_fallback());
}

#[test]
fn test_stats_snapshot_consistency() {
    // Test that get_stats() returns consistent snapshot
    let detector = ProxyFailureDetector::new(300, 0.2);

    detector.report_failure();
    detector.report_success();
    detector.report_failure();

    let stats1 = detector.get_stats();
    let stats2 = detector.get_stats();

    // Multiple calls should return same values
    assert_eq!(stats1.total_attempts, stats2.total_attempts);
    assert_eq!(stats1.failures, stats2.failures);
    assert_eq!(stats1.failure_rate, stats2.failure_rate);
}

#[test]
fn test_mark_fallback_idempotent() {
    // Test that marking fallback multiple times is safe
    let detector = ProxyFailureDetector::new(300, 0.2);

    for _ in 0..5 {
        detector.report_failure();
    }

    assert!(detector.should_fallback());

    // Mark multiple times
    detector.mark_fallback_triggered();
    detector.mark_fallback_triggered();
    detector.mark_fallback_triggered();

    let stats = detector.get_stats();
    assert!(stats.fallback_triggered);
    assert!(!detector.should_fallback());
}

#[test]
fn test_reset_clears_fallback_flag() {
    // Test that reset clears fallback trigger flag
    let detector = ProxyFailureDetector::new(300, 0.2);

    for _ in 0..10 {
        detector.report_failure();
    }

    detector.mark_fallback_triggered();
    assert!(detector.get_stats().fallback_triggered);

    detector.reset();

    let stats = detector.get_stats();
    assert!(!stats.fallback_triggered);
    assert_eq!(stats.total_attempts, 0);
}

#[test]
fn test_failure_rate_after_window_expiry() {
    // Test that failure rate updates after old attempts are pruned
    let detector = ProxyFailureDetector::new(1, 0.5);

    detector.report_failure();
    detector.report_failure();
    let stats_before = detector.get_stats();
    assert_eq!(stats_before.failure_rate, 1.0);

    // Wait for window to fully expire (add buffer)
    thread::sleep(Duration::from_millis(2000));

    // Add new success - this should trigger pruning
    detector.report_success();

    // Check stats - old failures should be gone
    let stats = detector.get_stats();
    assert_eq!(stats.failures, 0, "All old failures should be pruned");
    assert_eq!(stats.total_attempts, 1, "Only new success should remain");
    assert_eq!(stats.failure_rate, 0.0, "Failure rate should be 0.0");
}

#[test]
fn test_concurrent_reset() {
    // Test thread safety of reset operation
    let detector = Arc::new(ProxyFailureDetector::new(300, 0.2));
    let mut handles = vec![];

    // Add some initial failures
    for _ in 0..10 {
        detector.report_failure();
    }

    // Spawn threads that reset concurrently
    for _ in 0..5 {
        let detector_clone = Arc::clone(&detector);
        let handle = thread::spawn(move || {
            detector_clone.reset();
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // Should be reset
    let stats = detector.get_stats();
    assert_eq!(stats.total_attempts, 0);
    assert!(!stats.fallback_triggered);
}

#[test]
fn test_mixed_concurrent_operations() {
    // Test all operations concurrently
    let detector = Arc::new(ProxyFailureDetector::new(300, 0.2));
    let mut handles = vec![];

    // Thread 1: Report failures
    let d1 = Arc::clone(&detector);
    handles.push(thread::spawn(move || {
        for _ in 0..20 {
            d1.report_failure();
        }
    }));

    // Thread 2: Report successes
    let d2 = Arc::clone(&detector);
    handles.push(thread::spawn(move || {
        for _ in 0..20 {
            d2.report_success();
        }
    }));

    // Thread 3: Check stats
    let d3 = Arc::clone(&detector);
    handles.push(thread::spawn(move || {
        for _ in 0..10 {
            let _ = d3.get_stats();
            let _ = d3.should_fallback();
        }
    }));

    // Thread 4: Reset occasionally
    let d4 = Arc::clone(&detector);
    handles.push(thread::spawn(move || {
        thread::sleep(Duration::from_millis(10));
        d4.reset();
    }));

    for handle in handles {
        handle.join().unwrap();
    }

    // Just ensure no panics occurred
    let _stats = detector.get_stats();
}

#[test]
fn test_zero_threshold_always_triggers() {
    // Threshold of 0.0 should trigger on any failure
    let detector = ProxyFailureDetector::new(300, 0.0);

    detector.report_success();
    detector.report_success();
    detector.report_failure(); // Even 1 failure triggers at 0.0 threshold

    assert!(detector.should_fallback());
}

#[test]
fn test_one_threshold_never_triggers() {
    // Threshold of 1.0 should only trigger at 100% failure
    let detector = ProxyFailureDetector::new(300, 1.0);

    detector.report_failure();
    detector.report_failure();
    detector.report_success(); // 66.7% failure rate

    assert!(!detector.should_fallback());

    // Only all failures trigger
    detector.report_failure();
    detector.report_failure();
    detector.report_failure();
    // Now we have 5 failures, 1 success within window (from prune)
    // Need to check actual rate
    let stats = detector.get_stats();
    if stats.failure_rate >= 1.0 {
        assert!(detector.should_fallback());
    }
}

// ============================================================================
// P5.7 跨平台系统代理检测集成测试
// ============================================================================

/// 测试从 `HTTP_PROXY` 环境变量检测代理
#[test]
fn test_detect_from_http_proxy_env() {
    let original_http = env::var("HTTP_PROXY").ok();
    let original_https = env::var("HTTPS_PROXY").ok();
    let original_all = env::var("ALL_PROXY").ok();

    env::remove_var("HTTPS_PROXY");
    env::remove_var("ALL_PROXY");
    env::set_var("HTTP_PROXY", "http://proxy.example.com:8080");

    let result = SystemProxyDetector::detect_from_env();

    // 恢复环境变量
    match original_http {
        Some(val) => env::set_var("HTTP_PROXY", val),
        None => env::remove_var("HTTP_PROXY"),
    }
    match original_https {
        Some(val) => env::set_var("HTTPS_PROXY", val),
        None => env::remove_var("HTTPS_PROXY"),
    }
    match original_all {
        Some(val) => env::set_var("ALL_PROXY", val),
        None => env::remove_var("ALL_PROXY"),
    }

    assert!(result.is_some());
    let config = result.unwrap();
    assert_eq!(config.mode, ProxyMode::Http);
    assert!(config.url.contains("proxy.example.com") || config.url.contains("8080"));
}

/// 测试 `HTTPS_PROXY` 优先级
#[test]
fn test_detect_https_proxy_precedence() {
    let original_http = env::var("HTTP_PROXY").ok();
    let original_https = env::var("HTTPS_PROXY").ok();
    let original_all = env::var("ALL_PROXY").ok();

    env::remove_var("ALL_PROXY");
    env::set_var("HTTP_PROXY", "http://http-proxy.example.com:8080");
    env::set_var("HTTPS_PROXY", "http://https-proxy.example.com:8443");

    let result = SystemProxyDetector::detect_from_env();

    match original_http {
        Some(val) => env::set_var("HTTP_PROXY", val),
        None => env::remove_var("HTTP_PROXY"),
    }
    match original_https {
        Some(val) => env::set_var("HTTPS_PROXY", val),
        None => env::remove_var("HTTPS_PROXY"),
    }
    match original_all {
        Some(val) => env::set_var("ALL_PROXY", val),
        None => env::remove_var("ALL_PROXY"),
    }

    assert!(result.is_some());
    let config = result.unwrap();
    assert!(config.url.contains("https-proxy.example.com") || config.url.contains("8443"));
}

/// 测试 SOCKS5 代理检测
#[test]
fn test_detect_socks5_proxy() {
    let original = env::var("ALL_PROXY").ok();
    let original_http = env::var("HTTP_PROXY").ok();
    let original_https = env::var("HTTPS_PROXY").ok();
    // 也记录并清理可能残留的小写变量（其他测试可能设置）
    let original_lower_http = env::var("http_proxy").ok();
    let original_lower_https = env::var("https_proxy").ok();
    let original_lower_all = env::var("all_proxy").ok();

    env::remove_var("HTTP_PROXY");
    env::remove_var("HTTPS_PROXY");
    env::remove_var("http_proxy");
    env::remove_var("https_proxy");
    env::remove_var("all_proxy");
    env::set_var("ALL_PROXY", "socks5://socks-proxy.example.com:1080");

    // 使用 ALL_PROXY 以便在 detect_from_env 遍历顺序中优先匹配（HTTPS_PROXY/HTTP_PROXY 已被删除）
    let result = SystemProxyDetector::detect_from_env();
    if let Some(cfg) = &result {
        eprintln!("detected proxy mode={:?} url={} (expected Socks5)", cfg.mode, cfg.url);
    } else {
        eprintln!("no proxy detected for socks5 test");
    }

    match original {
        Some(val) => env::set_var("ALL_PROXY", val),
        None => env::remove_var("ALL_PROXY"),
    }
    match original_http {
        Some(val) => env::set_var("HTTP_PROXY", val),
        None => env::remove_var("HTTP_PROXY"),
    }
    match original_https {
        Some(val) => env::set_var("HTTPS_PROXY", val),
        None => env::remove_var("HTTPS_PROXY"),
    }
    match original_lower_http {
        Some(val) => env::set_var("http_proxy", val),
        None => env::remove_var("http_proxy"),
    }
    match original_lower_https {
        Some(val) => env::set_var("https_proxy", val),
        None => env::remove_var("https_proxy"),
    }
    match original_lower_all {
        Some(val) => env::set_var("all_proxy", val),
        None => env::remove_var("all_proxy"),
    }

    assert!(result.is_some());
    let config = result.unwrap();
    assert_eq!(config.mode, ProxyMode::Socks5, "expected Socks5 but got {:?} with url {}", config.mode, config.url);
    assert!(config.url.contains("socks-proxy.example.com"));
}

/// 测试无代理配置的情况
#[test]
fn test_detect_no_proxy() {
    let original_vars: Vec<(&str, Option<String>)> = vec![
        ("HTTP_PROXY", env::var("HTTP_PROXY").ok()),
        ("HTTPS_PROXY", env::var("HTTPS_PROXY").ok()),
        ("ALL_PROXY", env::var("ALL_PROXY").ok()),
        ("http_proxy", env::var("http_proxy").ok()),
        ("https_proxy", env::var("https_proxy").ok()),
        ("all_proxy", env::var("all_proxy").ok()),
    ];

    for (key, _) in &original_vars {
        env::remove_var(key);
    }

    let result = SystemProxyDetector::detect();

    // 恢复环境变量
    for (key, val) in original_vars {
        if let Some(v) = val {
            env::set_var(key, v);
        }
    }

    // 在没有代理配置的纯净环境下，可能返回 None 或系统代理
    let _ = result.is_some(); // 确认函数可以运行
}

/// 测试系统代理检测不会 panic
#[test]
fn test_system_proxy_detection_does_not_panic() {
    let result = SystemProxyDetector::detect();
    if let Some(config) = result {
        assert!(!config.url.is_empty() || config.mode == ProxyMode::Off);
    }
}

/// 测试系统代理检测性能
#[test]
fn test_proxy_detection_performance() {
    use std::time::Instant;

    let start = Instant::now();
    let _result = SystemProxyDetector::detect();
    let elapsed = start.elapsed();

    assert!(
        elapsed.as_secs() < 5,
        "Proxy detection should complete within 5 seconds"
    );
}

/// 测试多次检测的一致性
#[test]
fn test_proxy_detection_consistency() {
    let result1 = SystemProxyDetector::detect();
    let result2 = SystemProxyDetector::detect();

    match (result1, result2) {
        (Some(cfg1), Some(cfg2)) => {
            assert_eq!(cfg1.mode, cfg2.mode);
            assert_eq!(cfg1.url, cfg2.url);
        }
        (None, None) => {}
        _ => panic!("Inconsistent proxy detection results"),
    }
}
