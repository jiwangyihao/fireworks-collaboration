// Direct Command Function Testing
//
// 这个测试验证直接调用 #[tauri::command] 函数的可行性
// 关键发现: 在 tauri-core 模式下，TauriRuntime = MockRuntime

use std::borrow::Cow;
use std::sync::Arc;
use tauri::{Assets, Manager};
use tauri_utils::assets::{AssetKey, CspHash};

use fireworks_collaboration_lib::app::commands::git::git_clone;
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

/// Test directly calling the git_clone command function
#[tokio::test]
async fn test_direct_git_clone_command_call() {
    // Create registry with correct type: Arc<TaskRegistry>
    let registry: TaskRegistryState = Arc::new(TaskRegistry::new());
    let context = tauri::test::mock_context(MockAssets);

    let app = tauri::test::mock_builder()
        .manage::<TaskRegistryState>(registry.clone())
        .build(context)
        .expect("Failed to build mock app");

    // Get handle and state
    let handle = app.handle().clone();
    let state: tauri::State<TaskRegistryState> = app.state();

    // Call the command function directly!
    let result = git_clone(
        "https://github.com/example/repo.git".to_string(),
        "/tmp/test_repo".to_string(),
        None, // depth
        None, // filter
        None, // strategy_override
        None, // recurse_submodules
        state,
        handle,
    )
    .await;

    // Verify result
    assert!(
        result.is_ok(),
        "git_clone should return Ok, got: {:?}",
        result
    );
    let task_id = result.unwrap();
    assert!(!task_id.is_empty(), "Task ID should not be empty");

    // Verify task was created in registry
    let uuid = uuid::Uuid::parse_str(&task_id).expect("Valid UUID");
    let snapshot = registry.snapshot(&uuid);
    assert!(snapshot.is_some(), "Task should exist in registry");

    println!("SUCCESS: Direct command call works! Task ID: {}", task_id);
}
