// 从 src/core/tasks/retry.rs 迁移的测试
use fireworks_collaboration_lib::core::{
    git::errors::{ErrorCategory, GitError},
    tasks::retry::{backoff_delay_ms, compute_retry_diff, is_retryable, RetryPlan},
};

#[test]
fn test_backoff_monotonic_no_jitter() {
    let p = RetryPlan {
        max: 3,
        base_ms: 100,
        factor: 2.0,
        jitter: false,
    };
    assert_eq!(backoff_delay_ms(&p, 0), 100);
    assert_eq!(backoff_delay_ms(&p, 1), 200);
    assert_eq!(backoff_delay_ms(&p, 2), 400);
}

#[test]
fn test_is_retryable() {
    let err_net = GitError::new(ErrorCategory::Network, "net");
    assert!(is_retryable(&err_net));
    let err_auth = GitError::new(ErrorCategory::Auth, "401");
    assert!(!is_retryable(&err_auth));
    let err_cancel = GitError::new(ErrorCategory::Cancel, "user");
    assert!(!is_retryable(&err_cancel));
}

#[test]
fn test_http_5xx_retryable_and_internal_not() {
    let err_5xx = GitError::new(ErrorCategory::Protocol, "HTTP 502 Bad Gateway");
    assert!(is_retryable(&err_5xx));

    let err_internal = GitError::new(ErrorCategory::Internal, "invalid repository url format");
    assert!(!is_retryable(&err_internal));
}

#[test]
fn test_backoff_with_jitter_range() {
    let p = RetryPlan {
        max: 5,
        base_ms: 200,
        factor: 1.5,
        jitter: true,
    };
    // attempt 0 base is 200, jitter ±50% => [100, 300]
    for _ in 0..20 {
        let d = backoff_delay_ms(&p, 0);
        assert!(d >= 100 && d <= 300, "delay {} out of range", d);
    }
}

#[test]
fn test_compute_retry_diff() {
    let a = RetryPlan {
        max: 6,
        base_ms: 300,
        factor: 1.5,
        jitter: true,
    };
    let b_same = RetryPlan {
        max: 6,
        base_ms: 300,
        factor: 1.5,
        jitter: true,
    };
    let (d0, ch0) = compute_retry_diff(&a, &b_same);
    assert!(!ch0);
    assert!(d0.changed.is_empty());
    let b_diff = RetryPlan {
        max: 3,
        base_ms: 500,
        factor: 2.0,
        jitter: false,
    };
    let (d1, ch1) = compute_retry_diff(&a, &b_diff);
    assert!(ch1);
    assert_eq!(d1.changed.len(), 4);
    assert!(d1.changed.contains(&"max"));
    assert!(d1.changed.contains(&"baseMs"));
    assert!(d1.changed.contains(&"factor"));
    assert!(d1.changed.contains(&"jitter"));
}
