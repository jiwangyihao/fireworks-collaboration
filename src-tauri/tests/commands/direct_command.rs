// Direct Command Function Testing
//
// 这个测试验证直接调用 #[tauri::command] 函数的可行性
// 关键发现: 在 tauri-core 模式下，TauriRuntime = MockRuntime

use std::borrow::Cow;
use std::sync::Arc;
use tauri::{Assets, Manager};
use tauri_utils::assets::{AssetKey, CspHash};

use fireworks_collaboration_lib::app::commands::git::{
    git_add, git_clone, git_commit, git_fetch, git_init,
};
use fireworks_collaboration_lib::app::types::TaskRegistryState;
use fireworks_collaboration_lib::core::tasks::TaskRegistry;

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

/// Helper to create mock app with TaskRegistry
fn create_mock_app() -> (tauri::App<tauri::test::MockRuntime>, TaskRegistryState) {
    let registry: TaskRegistryState = Arc::new(TaskRegistry::new());
    let context = tauri::test::mock_context(MockAssets);

    let app = tauri::test::mock_builder()
        .manage::<TaskRegistryState>(registry.clone())
        .build(context)
        .expect("Failed to build mock app");

    (app, registry)
}

// ============================================================
// Git Clone Command Tests
// ============================================================

#[tokio::test]
async fn test_git_clone_command_basic() {
    let (app, registry) = create_mock_app();
    let state: tauri::State<TaskRegistryState> = app.state();
    let handle = app.handle().clone();

    let result = git_clone(
        "https://github.com/test/repo.git".to_string(),
        "/tmp/clone_test".to_string(),
        None,
        None,
        None,
        None,
        state,
        handle,
    )
    .await;

    assert!(result.is_ok());
    let task_id = result.unwrap();
    let uuid = uuid::Uuid::parse_str(&task_id).unwrap();
    assert!(registry.snapshot(&uuid).is_some());
}

#[tokio::test]
async fn test_git_clone_command_with_depth() {
    let (app, registry) = create_mock_app();
    let state: tauri::State<TaskRegistryState> = app.state();
    let handle = app.handle().clone();

    let result = git_clone(
        "https://github.com/test/repo.git".to_string(),
        "/tmp/clone_shallow".to_string(),
        Some(serde_json::json!(1)), // depth = 1
        None,
        None,
        None,
        state,
        handle,
    )
    .await;

    assert!(result.is_ok());
    let uuid = uuid::Uuid::parse_str(&result.unwrap()).unwrap();
    let snapshot = registry.snapshot(&uuid).unwrap();
    assert_eq!(snapshot.kind, "GitClone");
}

#[tokio::test]
async fn test_git_clone_command_with_submodules() {
    let (app, registry) = create_mock_app();
    let state: tauri::State<TaskRegistryState> = app.state();
    let handle = app.handle().clone();

    let result = git_clone(
        "https://github.com/test/repo.git".to_string(),
        "/tmp/clone_submodules".to_string(),
        None,
        None,
        None,
        Some(true), // recurse_submodules
        state,
        handle,
    )
    .await;

    assert!(result.is_ok());
    let uuid = uuid::Uuid::parse_str(&result.unwrap()).unwrap();
    assert!(registry.snapshot(&uuid).is_some());
}

// ============================================================
// Git Fetch Command Tests
// ============================================================

#[tokio::test]
async fn test_git_fetch_command_basic() {
    let (app, registry) = create_mock_app();
    let state: tauri::State<TaskRegistryState> = app.state();
    let handle = app.handle().clone();

    let result = git_fetch(
        "origin".to_string(),
        "/tmp/repo".to_string(),
        None,
        None,
        None,
        None,
        state,
        handle,
    )
    .await;

    assert!(result.is_ok());
    let uuid = uuid::Uuid::parse_str(&result.unwrap()).unwrap();
    let snapshot = registry.snapshot(&uuid).unwrap();
    assert_eq!(snapshot.kind, "GitFetch");
}

#[tokio::test]
async fn test_git_fetch_command_with_depth() {
    let (app, registry) = create_mock_app();
    let state: tauri::State<TaskRegistryState> = app.state();
    let handle = app.handle().clone();

    let result = git_fetch(
        "".to_string(),
        "/tmp/repo".to_string(),
        Some("default".to_string()),   // preset
        Some(serde_json::json!(10)),   // depth
        Some("blob:none".to_string()), // filter
        None,
        state,
        handle,
    )
    .await;

    assert!(result.is_ok());
    let uuid = uuid::Uuid::parse_str(&result.unwrap()).unwrap();
    assert!(registry.snapshot(&uuid).is_some());
}

// ============================================================
// Git Init Command Tests
// ============================================================

#[tokio::test]
async fn test_git_init_command() {
    let (app, registry) = create_mock_app();
    let state: tauri::State<TaskRegistryState> = app.state();
    let handle = app.handle().clone();

    let result = git_init("/tmp/new_repo".to_string(), state, handle).await;

    assert!(result.is_ok());
    let uuid = uuid::Uuid::parse_str(&result.unwrap()).unwrap();
    let snapshot = registry.snapshot(&uuid).unwrap();
    assert_eq!(snapshot.kind, "GitInit");
}

// ============================================================
// Git Add Command Tests
// ============================================================

#[tokio::test]
async fn test_git_add_command_single_file() {
    let (app, registry) = create_mock_app();
    let state: tauri::State<TaskRegistryState> = app.state();
    let handle = app.handle().clone();

    let result = git_add(
        "/tmp/repo".to_string(),
        vec!["file.txt".to_string()],
        state,
        handle,
    )
    .await;

    assert!(result.is_ok());
    let uuid = uuid::Uuid::parse_str(&result.unwrap()).unwrap();
    let snapshot = registry.snapshot(&uuid).unwrap();
    assert_eq!(snapshot.kind, "GitAdd");
}

#[tokio::test]
async fn test_git_add_command_multiple_files() {
    let (app, registry) = create_mock_app();
    let state: tauri::State<TaskRegistryState> = app.state();
    let handle = app.handle().clone();

    let result = git_add(
        "/tmp/repo".to_string(),
        vec![
            "a.txt".to_string(),
            "b.txt".to_string(),
            "src/c.rs".to_string(),
        ],
        state,
        handle,
    )
    .await;

    assert!(result.is_ok());
    let uuid = uuid::Uuid::parse_str(&result.unwrap()).unwrap();
    assert!(registry.snapshot(&uuid).is_some());
}

// ============================================================
// Git Commit Command Tests
// ============================================================

#[tokio::test]
async fn test_git_commit_command_basic() {
    let (app, registry) = create_mock_app();
    let state: tauri::State<TaskRegistryState> = app.state();
    let handle = app.handle().clone();

    let result = git_commit(
        "/tmp/repo".to_string(),
        "Initial commit".to_string(),
        None,
        None,
        None,
        state,
        handle,
    )
    .await;

    assert!(result.is_ok());
    let uuid = uuid::Uuid::parse_str(&result.unwrap()).unwrap();
    let snapshot = registry.snapshot(&uuid).unwrap();
    assert_eq!(snapshot.kind, "GitCommit");
}

#[tokio::test]
async fn test_git_commit_command_with_author() {
    let (app, registry) = create_mock_app();
    let state: tauri::State<TaskRegistryState> = app.state();
    let handle = app.handle().clone();

    let result = git_commit(
        "/tmp/repo".to_string(),
        "Commit with custom author".to_string(),
        Some(false), // allow_empty
        Some("Test Author".to_string()),
        Some("test@example.com".to_string()),
        state,
        handle,
    )
    .await;

    assert!(result.is_ok());
    let uuid = uuid::Uuid::parse_str(&result.unwrap()).unwrap();
    assert!(registry.snapshot(&uuid).is_some());
}

#[tokio::test]
async fn test_git_commit_command_allow_empty() {
    let (app, registry) = create_mock_app();
    let state: tauri::State<TaskRegistryState> = app.state();
    let handle = app.handle().clone();

    let result = git_commit(
        "/tmp/repo".to_string(),
        "Empty commit".to_string(),
        Some(true), // allow_empty
        None,
        None,
        state,
        handle,
    )
    .await;

    assert!(result.is_ok());
    let uuid = uuid::Uuid::parse_str(&result.unwrap()).unwrap();
    assert!(registry.snapshot(&uuid).is_some());
}

// ============================================================
// Tasks Command Tests
// ============================================================

use fireworks_collaboration_lib::app::commands::tasks::{
    task_cancel, task_list, task_snapshot, task_start_sleep,
};

#[tokio::test]
async fn test_task_list_command_empty() {
    let (app, _registry) = create_mock_app();
    let state: tauri::State<TaskRegistryState> = app.state();

    let result = task_list(state).await;

    assert!(result.is_ok());
    assert!(result.unwrap().is_empty());
}

#[tokio::test]
async fn test_task_list_command_with_tasks() {
    let (app, registry) = create_mock_app();
    let state: tauri::State<TaskRegistryState> = app.state();

    use fireworks_collaboration_lib::core::tasks::TaskKind;
    registry.create(TaskKind::Sleep { ms: 100 });
    registry.create(TaskKind::Sleep { ms: 200 });

    let result = task_list(state).await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap().len(), 2);
}

#[tokio::test]
async fn test_task_snapshot_command_existing() {
    let (app, registry) = create_mock_app();
    let state: tauri::State<TaskRegistryState> = app.state();

    use fireworks_collaboration_lib::core::tasks::TaskKind;
    let (id, _) = registry.create(TaskKind::Sleep { ms: 100 });

    let result = task_snapshot(id.to_string(), state).await;

    assert!(result.is_ok());
    let snapshot = result.unwrap().unwrap();
    assert_eq!(snapshot.kind, "Sleep");
}

#[tokio::test]
async fn test_task_snapshot_command_not_found() {
    let (app, _registry) = create_mock_app();
    let state: tauri::State<TaskRegistryState> = app.state();

    let result = task_snapshot(uuid::Uuid::new_v4().to_string(), state).await;

    assert!(result.is_ok());
    assert!(result.unwrap().is_none());
}

#[tokio::test]
async fn test_task_snapshot_command_invalid_uuid() {
    let (app, _registry) = create_mock_app();
    let state: tauri::State<TaskRegistryState> = app.state();

    let result = task_snapshot("not-a-uuid".to_string(), state).await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_task_cancel_command() {
    let (app, registry) = create_mock_app();
    let state: tauri::State<TaskRegistryState> = app.state();

    use fireworks_collaboration_lib::core::tasks::TaskKind;
    let (id, _) = registry.create(TaskKind::Sleep { ms: 10000 });

    let result = task_cancel(id.to_string(), state).await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_task_start_sleep_command() {
    let (app, registry) = create_mock_app();
    let state: tauri::State<TaskRegistryState> = app.state();
    let handle = app.handle().clone();

    let result = task_start_sleep(100, state, handle).await;

    assert!(result.is_ok());
    let uuid = uuid::Uuid::parse_str(&result.unwrap()).unwrap();
    assert_eq!(registry.snapshot(&uuid).unwrap().kind, "Sleep");
}
