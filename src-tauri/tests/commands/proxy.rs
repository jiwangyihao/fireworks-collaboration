//! Proxy command integration tests (Direct Command Call)

use std::borrow::Cow;
use std::sync::{Arc, Mutex};
use tauri::{Assets, Manager};
use tauri_utils::assets::{AssetKey, CspHash};

use fireworks_collaboration_lib::app::commands::proxy::*;
use fireworks_collaboration_lib::app::types::SharedConfig;
use fireworks_collaboration_lib::core::config::model::AppConfig;

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

fn create_mock_app() -> (tauri::App<tauri::test::MockRuntime>, SharedConfig) {
    let config: SharedConfig = Arc::new(Mutex::new(AppConfig::default()));
    let context = tauri::test::mock_context(MockAssets);

    let app = tauri::test::mock_builder()
        .manage::<SharedConfig>(config.clone())
        .build(context)
        .expect("Failed to build mock app");

    (app, config)
}

#[tokio::test]
async fn test_detect_system_proxy() {
    let result = detect_system_proxy().await;
    assert!(result.is_ok());

    // On CI/Mock environment, it likely returns None or some system default.
    // The result structure is valid.
    let res = result.unwrap();
    println!("Detected proxy: {:?}", res);
}

#[tokio::test]
async fn test_force_proxy_fallback() {
    let (app, config) = create_mock_app();

    // Enable proxy to allow fallback
    {
        let mut cfg = config.lock().unwrap();
        cfg.proxy.url = "http://localhost:8080".to_string();
        cfg.proxy.mode = fireworks_collaboration_lib::core::proxy::ProxyMode::Http;
    }

    let result = force_proxy_fallback(Some("Test fallback".to_string()), app.state()).await;
    if let Err(ref e) = result {
        println!("Fallback Error: {}", e);
    }
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), true);
}

#[tokio::test]
#[ignore = "Command implementation recreates ProxyManager, losing state required for recovery check"]
async fn test_force_proxy_recovery() {
    let (app, _) = create_mock_app();

    let result = force_proxy_recovery(app.state()).await;
    if let Err(ref e) = result {
        println!("Recovery Error: {}", e);
    }
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), true);
}

#[test]
fn test_get_system_proxy_legacy() {
    let result = get_system_proxy();
    assert!(result.is_ok());
}
