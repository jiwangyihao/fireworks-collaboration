//! OAuth 命令函数测试
//!
//! 测试 `app::commands::oauth` 模块中的 parse_oauth_callback 函数

use fireworks_collaboration_lib::app::commands::oauth::parse_oauth_callback;

#[test]
fn test_parses_code_parameter() {
    let req = "GET /auth/callback?code=abc123 HTTP/1.1\r\nHost: localhost";
    let data = parse_oauth_callback(req);

    assert_eq!(data.code, Some("abc123".to_string()));
    assert!(data.state.is_none());
    assert!(data.error.is_none());
}

#[test]
fn test_parses_code_and_state() {
    let req = "GET /auth/callback?code=abc123&state=xyz789 HTTP/1.1";
    let data = parse_oauth_callback(req);

    assert_eq!(data.code, Some("abc123".to_string()));
    assert_eq!(data.state, Some("xyz789".to_string()));
}

#[test]
fn test_parses_error_parameters() {
    let req =
        "GET /auth/callback?error=access_denied&error_description=User%20denied%20access HTTP/1.1";
    let data = parse_oauth_callback(req);

    assert_eq!(data.error, Some("access_denied".to_string()));
    assert_eq!(
        data.error_description,
        Some("User denied access".to_string())
    );
    assert!(data.code.is_none());
}

#[test]
fn test_handles_url_encoded_values() {
    let req = "GET /auth/callback?code=abc%2B123%3D%3D&state=test%20state HTTP/1.1";
    let data = parse_oauth_callback(req);

    assert_eq!(data.code, Some("abc+123==".to_string()));
    assert_eq!(data.state, Some("test state".to_string()));
}

#[test]
fn test_handles_no_query_string() {
    let req = "GET /auth/callback HTTP/1.1";
    let data = parse_oauth_callback(req);

    assert!(data.code.is_none());
    assert!(data.state.is_none());
    assert!(data.error.is_none());
    assert!(data.error_description.is_none());
}

#[test]
fn test_handles_empty_query_string() {
    let req = "GET /auth/callback? HTTP/1.1";
    let data = parse_oauth_callback(req);

    assert!(data.code.is_none());
}

#[test]
fn test_ignores_unknown_parameters() {
    let req = "GET /auth/callback?code=abc&unknown=value&other=123 HTTP/1.1";
    let data = parse_oauth_callback(req);

    assert_eq!(data.code, Some("abc".to_string()));
}

#[test]
fn test_handles_malformed_key_value() {
    // Missing '=' should be skipped
    let req = "GET /auth/callback?code=abc&malformed&state=xyz HTTP/1.1";
    let data = parse_oauth_callback(req);

    assert_eq!(data.code, Some("abc".to_string()));
    assert_eq!(data.state, Some("xyz".to_string()));
}
