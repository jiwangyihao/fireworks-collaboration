use std::sync::Arc;
use tokio::time::Duration;

use uuid::Uuid;

use fireworks_collaboration_lib::core::tasks::{
    model::{TaskKind, TaskState, WorkspaceBatchOperation},
    registry::TaskRegistry,
    workspace_batch::{CloneOptions, WorkspaceBatchChildOperation, WorkspaceBatchChildSpec},
};

#[tokio::test]
async fn test_workspace_batch_partial_failures() {
    // 1. Setup Registry
    let registry = Arc::new(TaskRegistry::new());

    // 2. Prepare Specs
    // - Child 1: Sleep (Success)
    // - Child 2: Clone Invalid (Fail)
    // - Child 3: Sleep (Success)
    // Note: Sleep is only available if we can construct WorkspaceBatchChildOperation::Sleep
    // But WorkspaceBatchChildOperation enum definition has `#[cfg(test)] Sleep(u64)`.
    // Since we are compiled as test, it should be available?
    // Wait, the lib is compiled as `lib`. Even with `tauri-core`, is `#[cfg(test)]` enabled for the LIB?
    // Generally `#[cfg(test)]` in lib is enabled when compiling `lib` for testing (unit tests).
    // But integration tests link against the lib.
    // If usage of `Sleep` is exposed only in `cfg(test)`, integration tests might NOT see it if lib is compiled normally.
    // Cargo compiles lib with `cfg(test)` ONLY for unit tests.
    // For integration tests, it compiles lib WITHOUT `cfg(test)`.
    // So `Sleep` variant might NOT be available here!

    // Let's verify this assumption.
    // If I cannot use Sleep, I must use something else safe.
    // `Fetch` with valid URL? Needs network.
    // `Clone` with valid URL? Needs network.
    // I risk network issues.

    // Alternative: Use `Fetch` with garbage URL (Fail).
    // Use `Fetch` with valid LOCAL URL (Success)?
    // I can init a local bare git repo using `git init --bare`?
    // Or just checking if `registry` handles failures is enough?
    // If I assume `Clone` with invalid URL FAILS.
    // How to make one SUCCEED without network?
    // I can create a local git repo in temp dir.

    // Let's try to verify `Sleep` existence first?
    // If I write code using `Sleep` and it fails to compile, I know.
    // But checking `src/core/tasks/workspace_batch.rs`:
    // #[cfg(test)]
    // Sleep(u64),

    // Integration tests cannot see this. I am 90% sure.
    // So I need a real operation.

    // Plan:
    // Create a local dummy repo to clone FROM.
    // Repo A (Source).
    // Child 1: Clone Repo A -> Dest A (Success).
    // Child 2: Clone Invalid -> Dest B (Fail).
    // Verify partial failure handling.

    use std::process::Command;
    use tempfile::tempdir;

    let temp = tempdir().unwrap();
    let source_path = temp.path().join("source_repo");
    let dest_success_path = temp.path().join("dest_success");
    let dest_fail_path = temp.path().join("dest_fail"); // Won't be created

    // Set up source repo
    std::fs::create_dir(&source_path).unwrap();
    // Initialize git repo
    let status = Command::new("git")
        .args(&["init", source_path.to_str().unwrap()])
        .status()
        .expect("git init failed");
    assert!(status.success());

    // Commit something so it can be cloned
    // git config user.email/name needed?
    Command::new("git")
        .current_dir(&source_path)
        .args(&["config", "user.email", "test@example.com"])
        .status()
        .unwrap();
    Command::new("git")
        .current_dir(&source_path)
        .args(&["config", "user.name", "Test User"])
        .status()
        .unwrap();
    // Create file
    std::fs::write(source_path.join("README.md"), "# Test").unwrap();
    Command::new("git")
        .current_dir(&source_path)
        .args(&["add", "."])
        .status()
        .unwrap();
    Command::new("git")
        .current_dir(&source_path)
        .args(&["commit", "-m", "Initial commit"])
        .status()
        .unwrap();

    let source_url = source_path.to_string_lossy().replace("\\", "/"); // file://...

    let specs = vec![
        WorkspaceBatchChildSpec {
            repo_id: "success_repo".into(),
            repo_name: "Success Repo".into(),
            operation: WorkspaceBatchChildOperation::Clone(CloneOptions {
                repo_url: source_url,
                dest: dest_success_path.to_string_lossy().to_string(),
                depth_u32: None,
                depth_value: None,
                filter: None,
                strategy_override: None,
                recurse_submodules: false,
            }),
        },
        WorkspaceBatchChildSpec {
            repo_id: "fail_repo".into(),
            repo_name: "Fail Repo".into(),
            operation: WorkspaceBatchChildOperation::Clone(CloneOptions {
                repo_url: "http://invalid-host-that-does-not-exist.local/repo.git".into(),
                dest: dest_fail_path.to_string_lossy().to_string(),
                depth_u32: None,
                depth_value: None,
                filter: None,
                strategy_override: None,
                recurse_submodules: false,
            }),
        },
    ];

    let (parent_id, parent_token) = registry.create(TaskKind::WorkspaceBatch {
        operation: WorkspaceBatchOperation::Clone,
        total: specs.len() as u32,
    });
    // Update total manually? No, create just registers it.
    // But `spawn_workspace_batch_task` updates/marks running.

    let _handle = registry.spawn_workspace_batch_task(
        None,
        parent_id,
        parent_token,
        WorkspaceBatchOperation::Clone,
        specs,
        2, // concurrency
    );

    // Wait for terminal state
    let final_state = wait_for_terminal_state(registry.clone(), parent_id).await;

    assert_eq!(
        final_state,
        TaskState::Failed,
        "Batch should be Failed due to partial failure"
    );

    let reason = registry.fail_reason(&parent_id).unwrap();
    assert!(
        reason.contains("batch clone: 1 repository failures"),
        "Reason should summarize failures: {}",
        reason
    );
    assert!(
        reason.contains("Fail Repo"),
        "Reason should mention failed repo name"
    );

    // Verify children states
    let children = registry.children_of(&parent_id);
    assert_eq!(children.len(), 2);

    let mut success_count = 0;
    let mut fail_count = 0;

    for child_id in children {
        let snap = registry.snapshot(&child_id).unwrap();
        match snap.state {
            TaskState::Completed => success_count += 1,
            TaskState::Failed => fail_count += 1,
            _ => panic!("Child task not finished: {:?}", snap.state),
        }
    }

    assert_eq!(success_count, 1, "Should have 1 successful child");
    assert_eq!(fail_count, 1, "Should have 1 failed child");
}

async fn wait_for_terminal_state(registry: Arc<TaskRegistry>, id: Uuid) -> TaskState {
    loop {
        if let Some(snapshot) = registry.snapshot(&id) {
            match snapshot.state {
                TaskState::Completed | TaskState::Failed | TaskState::Canceled => {
                    return snapshot.state
                }
                _ => {}
            }
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}
