//! Workspace command integration tests (Logic Verification)
//!
//! Tests the batch operations logic which is the core complexity of workspace commands.

use std::sync::Arc;
use tokio::time::Duration;

use fireworks_collaboration_lib::app::types::AppHandle;
use fireworks_collaboration_lib::core::tasks::model::{TaskKind, WorkspaceBatchOperation};
use fireworks_collaboration_lib::core::tasks::workspace_batch::{
    CloneOptions, WorkspaceBatchChildOperation, WorkspaceBatchChildSpec,
};
use fireworks_collaboration_lib::core::tasks::TaskRegistry;

/// Test Workspace Batch Clone Task Spawn Logic
#[tokio::test]
async fn test_workspace_batch_clone_logic() {
    // 1. Setup
    let registry = Arc::new(TaskRegistry::new());

    // 2. Prepare Specs
    let dest = "dummy/path".to_string();
    let repo_url = "https://github.com/example/repo.git".to_string();

    let clone_opts = CloneOptions {
        repo_url: repo_url.clone(),
        dest: dest.clone(),
        depth_u32: None,
        depth_value: None,
        filter: None,
        strategy_override: None,
        recurse_submodules: false,
    };

    let specs = vec![WorkspaceBatchChildSpec {
        repo_id: "repo-1".to_string(),
        repo_name: "Repo 1".to_string(),
        operation: WorkspaceBatchChildOperation::Clone(clone_opts),
    }];

    let operation = WorkspaceBatchOperation::Clone;
    let concurrency = 1;

    // 3. Create Task
    let (id, token) = registry.create(TaskKind::WorkspaceBatch {
        operation: operation.clone(),
        total: specs.len() as u32,
    });

    // 4. Spawn Task
    let app_handle = AppHandle::from_tauri(());

    registry.spawn_workspace_batch_task(Some(app_handle), id, token, operation, specs, concurrency);

    // 5. Verify Task Exists
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(registry.snapshot(&id).is_some());
}
