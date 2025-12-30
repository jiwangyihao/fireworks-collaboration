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

// ============================================================================
// Integration Tests
// ============================================================================

#[tokio::test]
async fn test_oauth_server_integration() {
    use fireworks_collaboration_lib::app::commands::oauth::{
        get_oauth_callback_data, start_oauth_server,
    };
    use fireworks_collaboration_lib::app::types::OAuthState;
    use std::borrow::Cow;
    use std::sync::{Arc, Mutex};
    use tauri::Assets;
    use tauri::Manager;
    use tauri_utils::assets::{AssetKey, CspHash};

    struct MockAssets;
    impl<R: tauri::Runtime> Assets<R> for MockAssets {
        fn get(&self, _key: &AssetKey) -> Option<Cow<'_, [u8]>> {
            None
        }
        fn iter(&self) -> Box<dyn Iterator<Item = (Cow<'_, str>, Cow<'_, [u8]>)> + '_> {
            Box::new(std::iter::empty())
        }
        fn csp_hashes(&self, _html_path: &AssetKey) -> Box<dyn Iterator<Item = CspHash<'_>> + '_> {
            Box::new(std::iter::empty())
        }
    }

    let oauth_state: OAuthState = Arc::new(Mutex::new(None));
    let app = tauri::test::mock_builder()
        .manage::<OAuthState>(oauth_state.clone())
        .build(tauri::test::mock_context(MockAssets))
        .expect("Failed to build mock app");

    // 1. Start server
    let port_result = start_oauth_server(app.state()).await;
    assert!(port_result.is_ok());
    let port = port_result.unwrap();
    assert!(port > 0);

    // 2. Simulate a callback request
    let client = hyper::Client::new();
    let callback_url = format!(
        "http://127.0.0.1:{}/auth/callback?code=mock_code_123&state=mock_state_456",
        port
    );
    let uri = callback_url.parse::<hyper::Uri>().unwrap();

    let resp = client.get(uri).await;
    assert!(resp.is_ok());
    assert_eq!(resp.unwrap().status(), hyper::StatusCode::OK);

    // 3. Verify state updated
    let data_result = get_oauth_callback_data(app.state()).await;
    assert!(data_result.is_ok());
    let data_opt = data_result.unwrap();
    assert!(data_opt.is_some());

    let data = data_opt.unwrap();
    assert_eq!(data.code, Some("mock_code_123".to_string()));
    assert_eq!(data.state, Some("mock_state_456".to_string()));

    // 4. Verify state was cleared (take)
    let data_again = get_oauth_callback_data(app.state()).await.unwrap();
    assert!(data_again.is_none());
}

#[tokio::test]
async fn test_oauth_clear_state() {
    use fireworks_collaboration_lib::app::commands::oauth::clear_oauth_state;
    use fireworks_collaboration_lib::app::types::{OAuthCallbackData, OAuthState};
    use std::sync::{Arc, Mutex};
    use tauri::Manager;

    let initial_data = OAuthCallbackData {
        code: Some("test".to_string()),
        state: None,
        error: None,
        error_description: None,
    };
    let oauth_state: OAuthState = Arc::new(Mutex::new(Some(initial_data)));
    let app = tauri::test::mock_builder()
        .manage::<OAuthState>(oauth_state.clone())
        .build(tauri::test::mock_context(tauri::test::noop_assets()))
        .unwrap();

    let _ = clear_oauth_state(app.state()).await.unwrap();

    let guard = oauth_state.lock().unwrap();
    assert!(guard.is_none());
}
