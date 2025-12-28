//! OAuth command integration tests

use std::borrow::Cow;
use std::sync::{Arc, Mutex};
use tauri::{Assets, Manager};
use tauri_utils::assets::{AssetKey, CspHash};

use fireworks_collaboration_lib::app::commands::oauth::*;
use fireworks_collaboration_lib::app::types::{OAuthCallbackData, OAuthState};

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

fn create_mock_app() -> (tauri::App<tauri::test::MockRuntime>, OAuthState) {
    let oauth_state: OAuthState = Arc::new(Mutex::new(None));
    let context = tauri::test::mock_context(MockAssets);

    let app = tauri::test::mock_builder()
        .manage::<OAuthState>(oauth_state.clone())
        .build(context)
        .expect("Failed to build mock app");

    (app, oauth_state)
}

#[tokio::test]
async fn test_start_oauth_server_command() {
    let (app, _) = create_mock_app();

    let result = start_oauth_server(app.state()).await;
    assert!(result.is_ok());
    let port = result.unwrap();
    assert!(port > 0);
    println!("OAuth server started on port: {}", port);
}

#[tokio::test]
async fn test_get_and_clear_oauth_state() {
    let (app, state) = create_mock_app();

    // Manually inject state
    {
        let mut guard = state.lock().unwrap();
        *guard = Some(OAuthCallbackData {
            code: Some("test_code".to_string()),
            state: Some("test_state".to_string()),
            error: None,
            error_description: None,
        });
    }

    // Test get (which should also clear in the provided implementation? No, code says take())
    let result = get_oauth_callback_data(app.state()).await;
    assert!(result.is_ok());
    let data = result.unwrap();
    assert!(data.is_some());
    assert_eq!(data.as_ref().unwrap().code.as_deref(), Some("test_code"));

    // Verify it was cleared (taken)
    let result2 = get_oauth_callback_data(app.state()).await;
    assert!(result2.is_ok());
    assert!(result2.unwrap().is_none());
}

#[tokio::test]
async fn test_clear_oauth_state_explicit() {
    let (app, state) = create_mock_app();

    {
        let mut guard = state.lock().unwrap();
        *guard = Some(OAuthCallbackData {
            code: None,
            state: None,
            error: Some("error".to_string()),
            error_description: None,
        });
    }

    let result = clear_oauth_state(app.state()).await;
    assert!(result.is_ok());

    {
        let guard = state.lock().unwrap();
        assert!(guard.is_none());
    }
}

// ============================================================================
// parse_oauth_callback tests for comprehensive coverage
// ============================================================================

#[test]
fn test_parse_oauth_callback_with_code() {
    let req = "GET /auth/callback?code=abc123&state=xyz HTTP/1.1\r\nHost: localhost";
    let data = parse_oauth_callback(req);

    assert_eq!(data.code, Some("abc123".to_string()));
    assert_eq!(data.state, Some("xyz".to_string()));
    assert!(data.error.is_none());
}

#[test]
fn test_parse_oauth_callback_with_error() {
    let req = "GET /auth/callback?error=access_denied&error_description=User%20denied HTTP/1.1";
    let data = parse_oauth_callback(req);

    assert!(data.code.is_none());
    assert_eq!(data.error, Some("access_denied".to_string()));
    assert_eq!(data.error_description, Some("User denied".to_string()));
}

#[test]
fn test_parse_oauth_callback_no_params() {
    let req = "GET /auth/callback HTTP/1.1";
    let data = parse_oauth_callback(req);

    assert!(data.code.is_none());
    assert!(data.state.is_none());
    assert!(data.error.is_none());
}

#[test]
fn test_parse_oauth_callback_url_encoded() {
    let req = "GET /auth/callback?code=a%2Fb%3Dc&state=x%26y HTTP/1.1";
    let data = parse_oauth_callback(req);

    assert_eq!(data.code, Some("a/b=c".to_string()));
    assert_eq!(data.state, Some("x&y".to_string()));
}

#[tokio::test]
async fn test_start_oauth_server_core() {
    let oauth_state: OAuthState = Arc::new(Mutex::new(None));
    let result = start_oauth_server_core(oauth_state).await;

    assert!(result.is_ok());
    let port = result.unwrap();
    assert!(port > 0);
}
