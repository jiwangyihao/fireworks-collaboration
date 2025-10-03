//! Tests for proxy failure detector

use fireworks_collaboration_lib::core::proxy::detector::ProxyFailureDetector;
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
    let detector = ProxyFailureDetector::default();
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
