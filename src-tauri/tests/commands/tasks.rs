//! Tasks command integration tests

use std::borrow::Cow;
use std::sync::Arc;
use tauri::{Assets, Manager};
use tauri_utils::assets::{AssetKey, CspHash};

use fireworks_collaboration_lib::app::commands::tasks::*;
use fireworks_collaboration_lib::core::tasks::registry::TaskRegistry;

use fireworks_collaboration_lib::app::types::TaskRegistryState;

// MockAssets definition
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

fn create_mock_app() -> (tauri::App<tauri::test::MockRuntime>, TaskRegistryState) {
    let registry: TaskRegistryState = Arc::new(TaskRegistry::new());

    let context = tauri::test::mock_context(MockAssets);

    let app = tauri::test::mock_builder()
        .manage::<TaskRegistryState>(registry.clone())
        .build(context)
        .expect("Failed to build mock app");

    (app, registry)
}

#[tokio::test]
async fn test_task_list_empty() {
    let (app, _) = create_mock_app();

    let result = task_list(app.state()).await;
    assert!(result.is_ok());

    let tasks = result.unwrap();
    // Initially should be empty
    assert!(tasks.is_empty());
}

#[tokio::test]
async fn test_task_snapshot_not_found() {
    let (app, _) = create_mock_app();

    // Generate a random UUID that doesn't exist
    let random_id = uuid::Uuid::new_v4().to_string();
    let result = task_snapshot(random_id, app.state()).await;

    assert!(result.is_ok());
    assert!(result.unwrap().is_none());
}

#[tokio::test]
async fn test_task_snapshot_invalid_uuid() {
    let (app, _) = create_mock_app();

    let result = task_snapshot("not-a-valid-uuid".to_string(), app.state()).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_task_cancel_not_found() {
    let (app, _) = create_mock_app();

    let random_id = uuid::Uuid::new_v4().to_string();
    let result = task_cancel(random_id, app.state()).await;

    assert!(result.is_ok());
    // Cancel returns false if task not found
    assert!(!result.unwrap());
}

#[tokio::test]
async fn test_task_cancel_invalid_uuid() {
    let (app, _) = create_mock_app();

    let result = task_cancel("invalid-uuid-format".to_string(), app.state()).await;
    assert!(result.is_err());
}
