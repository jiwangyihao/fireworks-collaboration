//! IP Pool command integration tests

use std::borrow::Cow;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::{Assets, Manager};
use tauri_utils::assets::{AssetKey, CspHash};

use fireworks_collaboration_lib::app::commands::ip_pool::*;
use fireworks_collaboration_lib::app::types::{ConfigBaseDir, SharedConfig, SharedIpPool};
use fireworks_collaboration_lib::core::config::model::AppConfig;
use fireworks_collaboration_lib::core::ip_pool::{
    EffectiveIpPoolConfig, IpPool, IpPoolFileConfig, IpPoolRuntimeConfig,
};

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
    temp_dir: &PathBuf,
) -> (
    tauri::App<tauri::test::MockRuntime>,
    SharedIpPool,
    SharedConfig,
    ConfigBaseDir,
) {
    let config: SharedConfig = Arc::new(Mutex::new(AppConfig::default()));

    // Initialize IpPool with default config
    let effective = EffectiveIpPoolConfig::from_parts(
        IpPoolRuntimeConfig::default(),
        IpPoolFileConfig::default(),
    );
    let ip_pool = IpPool::new(effective);
    let shared_pool: SharedIpPool = Arc::new(Mutex::new(ip_pool));

    let base_dir = temp_dir.clone();

    let context = tauri::test::mock_context(MockAssets);

    let app = tauri::test::mock_builder()
        .manage::<SharedIpPool>(shared_pool.clone())
        .manage::<SharedConfig>(config.clone())
        .manage::<ConfigBaseDir>(base_dir.clone())
        .build(context)
        .expect("Failed to build mock app");

    (app, shared_pool, config, base_dir)
}

#[tokio::test]
async fn test_ip_pool_get_snapshot() {
    let temp = tempfile::tempdir().unwrap();
    let (app, _, _, _) = create_mock_app(&temp.path().to_path_buf());

    let result = ip_pool_get_snapshot(app.state()).await;
    assert!(result.is_ok());
    let snapshot = result.unwrap();
    assert!(snapshot.enabled); // Default is true
}

#[tokio::test]
async fn test_ip_pool_update_config() {
    let temp = tempfile::tempdir().unwrap();
    let (app, pool, _, _) = create_mock_app(&temp.path().to_path_buf());

    let runtime_config = IpPoolRuntimeConfig {
        enabled: false,
        ..Default::default()
    };
    let file_config = IpPoolFileConfig::default();

    let result = ip_pool_update_config(
        runtime_config,
        file_config,
        app.state(),
        app.state(), // base dir
        app.state(), // pool
    )
    .await;

    assert!(result.is_ok());
    let snapshot = result.unwrap();
    assert!(!snapshot.enabled);

    // Verify state updated
    {
        let guard = pool.lock().unwrap();
        assert!(!guard.is_enabled());
    }
}

#[tokio::test]
async fn test_ip_pool_manual_actions() {
    let temp = tempfile::tempdir().unwrap();
    let (app, _pool, _, _) = create_mock_app(&temp.path().to_path_buf());

    // Refresh
    let res_refresh = ip_pool_request_refresh(app.state()).await;
    assert!(res_refresh.is_ok());

    // Clear auto disabled
    let res_clear = ip_pool_clear_auto_disabled(app.state()).await;
    assert!(res_clear.is_ok());

    // Start preheater
    let res_preheat = ip_pool_start_preheater(app.state()).await;
    assert!(res_preheat.is_ok());
}

#[tokio::test]
async fn test_ip_pool_pick_best() {
    let temp = tempfile::tempdir().unwrap();
    let (app, _pool, _, _) = create_mock_app(&temp.path().to_path_buf());

    // Pick best for a random host
    let result = ip_pool_pick_best("example.com".to_string(), 443, app.state()).await;
    assert!(result.is_ok());

    let selection = result.unwrap();
    assert_eq!(selection.host, "example.com");
    // Since pool might perform on-demand sampling, it could return Cached or System
    // just verify it's one of them
    assert!(selection.strategy == "system" || selection.strategy == "cached");
}
