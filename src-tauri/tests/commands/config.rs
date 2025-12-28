//! Config command integration tests (Direct Command Call)

use std::borrow::Cow;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::{Assets, Manager};
use tauri_utils::assets::{AssetKey, CspHash};

use fireworks_collaboration_lib::app::commands::config::*;
use fireworks_collaboration_lib::core::config::model::AppConfig;
use fireworks_collaboration_lib::core::ip_pool::{config::EffectiveIpPoolConfig, IpPool};
use fireworks_collaboration_lib::core::proxy::config::ProxyMode;
use fireworks_collaboration_lib::core::workspace::model::WorkspaceConfig;
use fireworks_collaboration_lib::core::workspace::status::WorkspaceStatusService;

use fireworks_collaboration_lib::app::types::{
    ConfigBaseDir, SharedConfig, SharedIpPool, SharedWorkspaceStatusService,
};

// Include MockAssets definition
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

fn create_mock_app(
    base_dir: PathBuf,
) -> (
    tauri::App<tauri::test::MockRuntime>,
    SharedConfig,
    SharedIpPool,
) {
    let config: SharedConfig = Arc::new(Mutex::new(AppConfig::default()));
    let base: ConfigBaseDir = base_dir;
    let pool: SharedIpPool = Arc::new(Mutex::new(IpPool::new(EffectiveIpPoolConfig::default())));
    let ws_config = WorkspaceConfig::default();
    let status_service: SharedWorkspaceStatusService =
        Arc::new(WorkspaceStatusService::new(&ws_config));

    let context = tauri::test::mock_context(MockAssets);

    let app = tauri::test::mock_builder()
        .manage::<SharedConfig>(config.clone())
        .manage::<ConfigBaseDir>(base)
        .manage::<SharedIpPool>(pool.clone())
        .manage::<SharedWorkspaceStatusService>(status_service)
        .build(context)
        .expect("Failed to build mock app");

    (app, config, pool)
}

#[tokio::test]
async fn test_get_config_returns_default() {
    let temp = tempfile::tempdir().unwrap();
    let (app, _, _) = create_mock_app(temp.path().to_path_buf());

    let result = get_config(app.state()).await;
    assert!(result.is_ok());

    let config = result.unwrap();
    // Should return default config (proxy mode is Disabled by default)
    assert!(matches!(config.proxy.mode, ProxyMode::Off));
}

#[tokio::test]
async fn test_set_config_updates_state() {
    let temp = tempfile::tempdir().unwrap();
    let (app, config_state, _) = create_mock_app(temp.path().to_path_buf());

    // Modify config - use mode instead of enabled
    let mut new_config = AppConfig::default();
    new_config.proxy.mode = ProxyMode::System;
    new_config.proxy.fallback_threshold = 5.0;

    let result = set_config(
        new_config.clone(),
        app.state(),
        app.state(),
        app.state(),
        app.state(),
    )
    .await;

    assert!(result.is_ok());

    // Verify state was updated
    let guard = config_state.lock().unwrap();
    assert!(matches!(guard.proxy.mode, ProxyMode::System));
    assert_eq!(guard.proxy.fallback_threshold, 5.0);
}

#[tokio::test]
async fn test_set_config_saves_to_disk() {
    let temp = tempfile::tempdir().unwrap();
    let (app, _, _) = create_mock_app(temp.path().to_path_buf());

    let mut new_config = AppConfig::default();
    new_config.proxy.mode = ProxyMode::System;

    let result = set_config(
        new_config,
        app.state(),
        app.state(),
        app.state(),
        app.state(),
    )
    .await;

    assert!(result.is_ok());

    // Verify file was created (config/config.json under base_dir)
    let config_path = temp.path().join("config").join("config.json");
    assert!(config_path.exists());
}

#[tokio::test]
async fn test_greet_command() {
    let greeting = greet("Test User");
    assert!(greeting.contains("Test User"));
    assert!(greeting.contains("Hello"));
}

#[tokio::test]
async fn test_export_team_config_template() {
    let temp = tempfile::tempdir().unwrap();
    let (app, _, _) = create_mock_app(temp.path().to_path_buf());

    let result = export_team_config_template(None, None, app.state(), app.state()).await;

    assert!(result.is_ok());
    let path = result.unwrap();
    assert!(path.contains("team-config-template.json"));
    assert!(std::path::Path::new(&path).exists());
}

#[tokio::test]
async fn test_import_team_config_template_missing_file() {
    let temp = tempfile::tempdir().unwrap();
    let (app, _, _) = create_mock_app(temp.path().to_path_buf());

    // Attempt to import non-existent template
    let result = import_team_config_template(
        Some("/nonexistent/path/template.json".to_string()),
        None,
        app.state(),
        app.state(),
        app.state(),
    )
    .await;

    // Should fail because file doesn't exist
    assert!(result.is_err());
}

#[tokio::test]
async fn test_export_then_import_roundtrip() {
    let temp = tempfile::tempdir().unwrap();
    let (app, config_state, _) = create_mock_app(temp.path().to_path_buf());

    // First, set a custom config
    let mut custom_config = AppConfig::default();
    custom_config.proxy.mode = ProxyMode::System;
    custom_config.proxy.fallback_threshold = 10.0;
    {
        let mut guard = config_state.lock().unwrap();
        *guard = custom_config.clone();
    }

    // Export
    let export_result = export_team_config_template(None, None, app.state(), app.state()).await;
    assert!(export_result.is_ok());
    let template_path = export_result.unwrap();

    // Reset config
    {
        let mut guard = config_state.lock().unwrap();
        *guard = AppConfig::default();
    }

    // Import
    let import_result = import_team_config_template(
        Some(template_path),
        None,
        app.state(),
        app.state(),
        app.state(),
    )
    .await;

    assert!(import_result.is_ok());
    let report = import_result.unwrap();
    // Report has applied and skipped Vec fields
    assert!(!report.applied.is_empty() || !report.skipped.is_empty());
}

#[tokio::test]
async fn test_import_invalid_json() {
    let temp = tempfile::tempdir().unwrap();
    let (app, _, _) = create_mock_app(temp.path().to_path_buf());

    let path = temp.path().join("invalid.json");
    std::fs::write(&path, "{ bad json").unwrap();

    let result = import_team_config_template(
        Some(path.to_string_lossy().to_string()),
        None,
        app.state(),
        app.state(),
        app.state(),
    )
    .await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_set_config_io_error() {
    // To trigger I/O error, we give a base path that is invalid or readonly?
    // On Windows, tempdir is usually okay.
    // If we deliberately pass a non-existent base path that cannot be created?
    // ConfigBaseDir is passed in.
    let temp = tempfile::tempdir().unwrap();
    // It might create it.
    // Let's create a FILE where the dir should be.
    let block_file = temp.path().join("block");
    std::fs::write(&block_file, "").unwrap();
    let blocked_dir = block_file.join("config"); // This should fail as parent is file

    let (app, _, _) = create_mock_app(blocked_dir);

    let result = set_config(
        AppConfig::default(),
        app.state(),
        app.state(),
        app.state(),
        app.state(),
    )
    .await;

    // Should fail to save
    assert!(result.is_err());
}
