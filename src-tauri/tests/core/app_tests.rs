// 从 src/app.rs 迁移的测试
#[cfg(feature = "tauri-app")]
use fireworks_collaboration_lib::app::{
    classify_error_msg, host_in_whitelist, redact_auth_in_headers,
};
#[cfg(feature = "tauri-app")]
use fireworks_collaboration_lib::core::config::model::AppConfig;

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
    // default has github.com and *.github.com
    assert!(host_in_whitelist("github.com", &cfg));
    assert!(host_in_whitelist("api.github.com", &cfg));
    assert!(!host_in_whitelist("example.com", &cfg));

    // empty whitelist -> reject any
    cfg.tls.san_whitelist.clear();
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
