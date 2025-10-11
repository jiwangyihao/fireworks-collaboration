//! Core 模块综合测试
//! 合并了 `core/app_tests.rs`, `core/logging_tests.rs`, `core/tasks/retry_tests.rs`,
//! `core/tls/spki_tests.rs`, `core/tls/util_tests.rs`

use fireworks_collaboration_lib::core::config::model::AppConfig;

// ============================================================================
// app_tests.rs 的测试
// ============================================================================

#[cfg(feature = "tauri-app")]
use fireworks_collaboration_lib::app::{
    classify_error_msg, host_in_whitelist, redact_auth_in_headers,
};

#[cfg(feature = "tauri-app")]
#[test]
fn test_redact_auth_in_headers_case_insensitive() {
    let mut h = std::collections::HashMap::new();
    h.insert("Authorization".to_string(), "Bearer abc".to_string());
    h.insert("x-other".to_string(), "1".to_string());
    let out = redact_auth_in_headers(h, true);
    assert_eq!(out.get("Authorization").unwrap(), "REDACTED");
    assert_eq!(out.get("x-other").unwrap(), "1");

    let mut h2 = std::collections::HashMap::new();
    h2.insert("aUtHoRiZaTiOn".to_string(), "token".to_string());
    let out2 = redact_auth_in_headers(h2, true);
    assert_eq!(out2.get("aUtHoRiZaTiOn").unwrap(), "REDACTED");
}

#[cfg(feature = "tauri-app")]
#[test]
fn test_redact_auth_no_mask_keeps_original() {
    let mut h = std::collections::HashMap::new();
    h.insert("Authorization".to_string(), "Bearer xyz".to_string());
    let out = redact_auth_in_headers(h, false);
    assert_eq!(out.get("Authorization").unwrap(), "Bearer xyz");
}

#[cfg(feature = "tauri-app")]
#[test]
fn test_host_in_whitelist_exact_and_wildcard() {
    let mut cfg = AppConfig::default();
    cfg.http.fake_sni_target_hosts = vec!["github.com".into(), "*.github.com".into()];
    assert!(host_in_whitelist("github.com", &cfg));
    assert!(host_in_whitelist("api.github.com", &cfg));
    assert!(!host_in_whitelist("example.com", &cfg));

    // empty whitelist -> reject any
    cfg.http.fake_sni_target_hosts.clear();
    assert!(!host_in_whitelist("github.com", &cfg));
}

#[cfg(feature = "tauri-app")]
#[test]
fn test_classify_error_msg_mapping() {
    let cases = vec![
        ("SAN whitelist mismatch", "Verify"),
        ("Tls: tls handshake", "Tls"),
        ("connect timeout", "Network"),
        ("connect error", "Network"),
        ("read body", "Network"),
        ("only https", "Input"),
        ("invalid URL", "Input"),
        ("url host missing", "Input"),
        ("some other error", "Internal"),
    ];
    for (msg, cat) in cases {
        let (got, _m) = classify_error_msg(msg);
        assert_eq!(got, cat, "msg={}", msg);
    }
}

// ============================================================================
// logging_tests.rs 的测试
// ============================================================================

use fireworks_collaboration_lib::logging::init_logging;

#[test]
fn test_init_logging_idempotent() {
    // 调用两次不应 panic
    init_logging();
    init_logging();
    // 发一条日志确保不会崩
    tracing::info!(target = "app", "test log after init");
}

// ============================================================================
// tasks/retry_tests.rs 的测试
// ============================================================================

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
        assert!((100..=300).contains(&d), "delay {d} out of range");
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

// ============================================================================
// tls/spki_tests.rs 的测试
// ============================================================================

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use fireworks_collaboration_lib::core::tls::spki::{
    compute_fingerprint_bundle, compute_spki_sha256_b64, SpkiSource,
};
use rcgen::generate_simple_self_signed;
use ring::digest::{digest, SHA256};
use rustls::Certificate;

#[test]
fn test_extract_spki_exact() {
    let cert = generate_simple_self_signed(vec!["example.com".into()]).unwrap();
    let der = cert.serialize_der().unwrap();
    let rustls_cert = Certificate(der.clone());

    let (spki, source) = compute_spki_sha256_b64(&rustls_cert);
    assert_eq!(source, SpkiSource::Exact);
    assert_eq!(spki.len(), 43);
}

#[test]
fn test_empty_cert_falls_back() {
    let cert = Certificate(Vec::new());
    let (spki, source) = compute_spki_sha256_b64(&cert);
    assert_eq!(source, SpkiSource::WholeCertFallback);
    assert_eq!(spki.len(), 43);
}

#[test]
fn test_fingerprint_bundle_contains_cert_hash() {
    let cert = generate_simple_self_signed(vec!["bundle.example".into()]).unwrap();
    let der = cert.serialize_der().unwrap();
    let rustls_cert = Certificate(der.clone());

    let bundle = compute_fingerprint_bundle(&rustls_cert);
    assert_eq!(bundle.spki_sha256.len(), 43);
    assert_eq!(bundle.cert_sha256.len(), 43);

    let expected_cert_sha = URL_SAFE_NO_PAD.encode(digest(&SHA256, &der).as_ref());
    assert_eq!(bundle.cert_sha256, expected_cert_sha);
    assert_eq!(bundle.spki_source, SpkiSource::Exact);
}

// ============================================================================
// tls/util_tests.rs 的测试
// ============================================================================

use fireworks_collaboration_lib::core::tls::util::{
    decide_sni_host_with_proxy, match_domain, set_last_good_sni, should_use_fake,
};

#[test]
fn test_should_use_fake() {
    let mut cfg = AppConfig::default();
    cfg.http.fake_sni_enabled = true;
    cfg.http.fake_sni_target_hosts = vec!["github.com".into()];
    assert!(should_use_fake(&cfg, false, "github.com"));
    assert!(!should_use_fake(&cfg, false, "example.com"));
    assert!(!should_use_fake(&cfg, true, "github.com"));
    cfg.http.fake_sni_enabled = false;
    assert!(!should_use_fake(&cfg, false, "github.com"));
}

#[test]
fn test_match_domain_exact_and_wildcard() {
    assert!(match_domain("github.com", "github.com"));
    assert!(!match_domain("github.com", "api.github.com"));
    assert!(match_domain("*.github.com", "api.github.com"));
    assert!(!match_domain("*.github.com", "github.com"));
    assert!(!match_domain("*.github.com", "x.ygithub.com"));
}

#[test]
fn test_match_domain_case_insensitive_and_multi_sub() {
    assert!(match_domain("GITHUB.COM", "github.com"));
    assert!(match_domain("*.GitHub.com", "API.GitHub.Com"));
    assert!(match_domain("*.github.com", "a.b.github.com"));
    assert!(!match_domain("*.*.github.com", "a.b.github.com"));
}

#[test]
fn test_decide_sni_host_with_proxy_and_candidates() {
    let mut cfg = AppConfig::default();
    cfg.http.fake_sni_enabled = true;
    cfg.http.fake_sni_hosts = vec!["a.com".into(), "b.com".into(), "c.com".into()];
    cfg.http.fake_sni_target_hosts = vec!["github.com".into()];
    let (sni, used_fake) = decide_sni_host_with_proxy(&cfg, false, "github.com", false);
    assert!(used_fake);
    assert!(sni == "a.com" || sni == "b.com" || sni == "c.com");

    let (sni2, used2) = decide_sni_host_with_proxy(&cfg, false, "github.com", true);
    assert_eq!(sni2, "github.com");
    assert!(!used2);

    cfg.http.fake_sni_enabled = false;
    let (sni3, used3) = decide_sni_host_with_proxy(&cfg, false, "github.com", false);
    assert_eq!(sni3, "github.com");
    assert!(!used3);
}

#[test]
fn test_last_good_preferred_when_present() {
    let mut cfg = AppConfig::default();
    cfg.http.fake_sni_enabled = true;
    cfg.http.fake_sni_hosts = vec!["x.com".into(), "y.com".into()];
    cfg.http.fake_sni_target_hosts = vec!["github.com".into()];
    set_last_good_sni("github.com", "y.com");
    let (sni, used_fake) = decide_sni_host_with_proxy(&cfg, false, "github.com", false);
    assert!(used_fake);
    assert_eq!(sni, "y.com");
}
