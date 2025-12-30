//! Tasks command integration tests

use std::borrow::Cow;
use std::sync::Arc;
use tauri::{Assets, Manager};
use tauri_utils::assets::{AssetKey, CspHash};

use fireworks_collaboration_lib::app::commands::tasks::*;
use fireworks_collaboration_lib::core::tasks::registry::TaskRegistry;
use fireworks_collaboration_lib::core::tasks::{TaskKind, TaskState};

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

#[tokio::test]
async fn test_task_lifecycle_integration() {
    let (app, _) = create_mock_app();

    // 1. Start a sleep task (1000ms)
    let id_result = task_start_sleep(1000, app.state(), app.handle().clone()).await;
    assert!(id_result.is_ok());
    let id = id_result.unwrap();

    // 2. Verify it's in the list
    let list = task_list(app.state()).await.unwrap();
    assert!(list.iter().any(|t| t.id.to_string() == id));

    // 3. Get snapshot
    let snapshot = task_snapshot(id.clone(), app.state()).await.unwrap();
    assert!(snapshot.is_some());
    let s = snapshot.unwrap();
    assert_eq!(s.id.to_string(), id);

    // 4. Cancel the task
    let cancel_result = task_cancel(id.clone(), app.state()).await;
    assert!(cancel_result.is_ok());
    assert!(cancel_result.unwrap());

    // 5. Verify it's marked as cancelled (Retry a few times as state update is async)
    let mut success = false;
    for _ in 0..10 {
        let snapshot_after = task_snapshot(id.clone(), app.state()).await.unwrap();
        if let Some(s) = snapshot_after {
            if s.state == TaskState::Canceled {
                success = true;
                break;
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    assert!(success, "Task did not transition to Canceled state");
}

#[tokio::test]
async fn test_task_panic_state_resilience() {
    let (_app, registry) = create_mock_app();

    // Directly spawn a task that panics
    let (id, _token) = registry.create(TaskKind::Sleep { ms: 100 });

    // Simulate what a real task runner would do
    let reg_clone = registry.clone();
    let id_clone = id.clone();
    let _handle = tokio::spawn(async move {
        reg_clone.set_state_noemit(&id_clone, TaskState::Running);
        panic!("Simulated task panic");
    });

    // Wait for the task to finish (panicked)
    let _ = _handle.await;

    // Check state
    let snapshot = registry.snapshot(&id).unwrap();

    // Expected behavior: Ideally it SHOULD NOT be stuck in Running.
    // However, without a panic handler, it will be.
    // We want to verify the CURRENT behavior first.
    // If it's Running, we found a hardening opportunity.
    assert_eq!(snapshot.state, TaskState::Running);
}
