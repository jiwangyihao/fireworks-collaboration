//! HTTP 命令函数测试
//!
//! 测试 `app::commands::http` 模块中的辅助函数

use fireworks_collaboration_lib::app::commands::http::{
    classify_error_msg, process_redirect, redact_auth_in_headers, update_request_for_redirect,
    validate_url,
};
use fireworks_collaboration_lib::core::http::types::HttpRequestInput;
use std::collections::HashMap;

// ============================================================================
// redact_auth_in_headers tests
// ============================================================================

#[test]
fn test_redact_auth_in_headers_masks_authorization() {
    let mut headers = HashMap::new();
    headers.insert(
        "Authorization".to_string(),
        "Bearer secret-token".to_string(),
    );
    headers.insert("Content-Type".to_string(), "application/json".to_string());

    let result = redact_auth_in_headers(headers, true);

    assert_eq!(result.get("Authorization").unwrap(), "REDACTED");
    assert_eq!(result.get("Content-Type").unwrap(), "application/json");
}

#[test]
fn test_redact_auth_in_headers_case_insensitive() {
    let mut headers = HashMap::new();
    headers.insert("authorization".to_string(), "Bearer token".to_string());

    let result = redact_auth_in_headers(headers, true);

    assert_eq!(result.get("authorization").unwrap(), "REDACTED");
}

#[test]
fn test_redact_auth_in_headers_no_mask() {
    let mut headers = HashMap::new();
    headers.insert("Authorization".to_string(), "Bearer secret".to_string());

    let result = redact_auth_in_headers(headers, false);

    assert_eq!(result.get("Authorization").unwrap(), "Bearer secret");
}

#[test]
fn test_redact_auth_in_headers_empty() {
    let headers = HashMap::new();
    let result = redact_auth_in_headers(headers, true);
    assert!(result.is_empty());
}

// ============================================================================
// classify_error_msg tests
// ============================================================================

#[test]
fn test_classify_error_msg_verify() {
    let (cat, _msg) = classify_error_msg("SAN whitelist mismatch for host");
    assert_eq!(cat, "Verify");
}

#[test]
fn test_classify_error_msg_tls() {
    let (cat, _msg) = classify_error_msg("tls handshake failed");
    assert_eq!(cat, "Tls");
}

#[test]
fn test_classify_error_msg_network_timeout() {
    let (cat, _msg) = classify_error_msg("connect timeout after 30s");
    assert_eq!(cat, "Network");
}

#[test]
fn test_classify_error_msg_network_connect() {
    let (cat, _msg) = classify_error_msg("connect error: connection refused");
    assert_eq!(cat, "Network");
}

#[test]
fn test_classify_error_msg_input() {
    let (cat, _msg) = classify_error_msg("only https is supported");
    assert_eq!(cat, "Input");
}

#[test]
fn test_classify_error_msg_internal() {
    let (cat, _msg) = classify_error_msg("some unknown error occurred");
    assert_eq!(cat, "Internal");
}

// ============================================================================
// validate_url tests
// ============================================================================

#[test]
fn test_validate_url_valid_https() {
    let result = validate_url("https://example.com/path");
    assert!(result.is_ok());
    let (_, host) = result.unwrap();
    assert_eq!(host, "example.com");
}

#[test]
fn test_validate_url_rejects_http() {
    let result = validate_url("http://example.com");
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("only https"));
}

#[test]
fn test_validate_url_invalid_format() {
    let result = validate_url("not a valid url");
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("invalid URL"));
}

#[test]
fn test_validate_url_missing_host() {
    // This particular malformed URL should fail
    let result = validate_url("https:///path");
    assert!(result.is_err());
}

#[test]
fn test_validate_url_with_port() {
    let result = validate_url("https://example.com:8443/api");
    assert!(result.is_ok());
    let (_, host) = result.unwrap();
    assert_eq!(host, "example.com");
}

// ============================================================================
// process_redirect tests
// ============================================================================

#[test]
fn test_process_redirect_absolute_url() {
    let mut headers = HashMap::new();
    headers.insert("location".to_string(), "https://other.com/new".to_string());

    let result = process_redirect(&headers, "https://example.com/old");
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "https://other.com/new");
}

#[test]
fn test_process_redirect_relative_url() {
    let mut headers = HashMap::new();
    headers.insert("location".to_string(), "/new/path".to_string());

    let result = process_redirect(&headers, "https://example.com/old");
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "https://example.com/new/path");
}

#[test]
fn test_process_redirect_no_location() {
    let headers = HashMap::new();

    let result = process_redirect(&headers, "https://example.com");
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Location header"));
}

// ============================================================================
// update_request_for_redirect tests
// ============================================================================

fn create_test_request() -> HttpRequestInput {
    HttpRequestInput {
        url: "https://example.com".to_string(),
        method: "POST".to_string(),
        headers: HashMap::new(),
        body_base64: Some("dGVzdA==".to_string()),
        follow_redirects: true,
        max_redirects: 5,
        timeout_ms: 30000,
        force_real_sni: false,
    }
}

#[test]
fn test_update_request_for_redirect_301() {
    let mut input = create_test_request();
    update_request_for_redirect(&mut input, 301, "https://new.com".to_string());

    assert_eq!(input.url, "https://new.com");
    assert_eq!(input.method, "GET");
    assert!(input.body_base64.is_none());
}

#[test]
fn test_update_request_for_redirect_302() {
    let mut input = create_test_request();
    update_request_for_redirect(&mut input, 302, "https://new.com".to_string());

    assert_eq!(input.method, "GET");
    assert!(input.body_base64.is_none());
}

#[test]
fn test_update_request_for_redirect_307_preserves_method() {
    let mut input = create_test_request();
    update_request_for_redirect(&mut input, 307, "https://new.com".to_string());

    assert_eq!(input.method, "POST");
    assert!(input.body_base64.is_some());
}

#[test]
fn test_update_request_for_redirect_308_preserves_method() {
    let mut input = create_test_request();
    update_request_for_redirect(&mut input, 308, "https://new.com".to_string());

    assert_eq!(input.method, "POST");
    assert!(input.body_base64.is_some());
}
