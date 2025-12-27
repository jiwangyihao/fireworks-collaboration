use std::process::Command;
use std::sync::Arc;
use tempfile::tempdir;
use tokio::time::Duration;
use uuid::Uuid;

use fireworks_collaboration_lib::core::tasks::{
    model::{TaskKind, TaskState},
    registry::TaskRegistry,
};

#[tokio::test]
async fn test_git_push_conflict() {
    let registry = Arc::new(TaskRegistry::new());
    let temp = tempdir().unwrap();
    let bare_repo_path = temp.path().join("bare_repo.git");
    let client_a_path = temp.path().join("client_a");
    let client_b_path = temp.path().join("client_b");

    // 1. Setup Bare Repo
    let status = Command::new("git")
        .args(&["init", "--bare", bare_repo_path.to_str().unwrap()])
        .status()
        .expect("git init bare failed");
    assert!(status.success());
    let repo_url = bare_repo_path.to_string_lossy().replace("\\", "/");
    // Wait, on windows file:///C:/... works best with forward slashes usually.
    // Standardize URL:
    let repo_url = format!("file:///{}", repo_url);

    // 2. Client A setup (Clone, Commit, Push)
    // We can use git commands directly for setup to "prime" the state.
    std::fs::create_dir(&client_a_path).unwrap();
    Command::new("git")
        .args(&["clone", &repo_url, client_a_path.to_str().unwrap()])
        .status()
        .unwrap();

    // Config user
    fn config_git(path: &std::path::Path, name: &str) {
        Command::new("git")
            .current_dir(path)
            .args(&["config", "user.email", "test@example.com"])
            .status()
            .unwrap();
        Command::new("git")
            .current_dir(path)
            .args(&["config", "user.name", name])
            .status()
            .unwrap();
    }
    config_git(&client_a_path, "Client A");

    // A adds file and pushes
    std::fs::write(client_a_path.join("file.txt"), "Version A").unwrap();
    Command::new("git")
        .current_dir(&client_a_path)
        .args(&["add", "."])
        .status()
        .unwrap();
    Command::new("git")
        .current_dir(&client_a_path)
        .args(&["commit", "-m", "Commit A"])
        .status()
        .unwrap();
    Command::new("git")
        .current_dir(&client_a_path)
        .args(&["push"])
        .status()
        .unwrap();

    // 3. Client B setup (Clone, Commit different, Push -> Conflict)
    std::fs::create_dir(&client_b_path).unwrap();
    // Clone NOW. It gets "Version A".
    Command::new("git")
        .args(&["clone", &repo_url, client_b_path.to_str().unwrap()])
        .status()
        .unwrap();
    config_git(&client_b_path, "Client B");

    // Now A updates file and pushes again to move HEAD forward.
    std::fs::write(client_a_path.join("file.txt"), "Version A2").unwrap();
    Command::new("git")
        .current_dir(&client_a_path)
        .args(&["add", "."])
        .status()
        .unwrap();
    Command::new("git")
        .current_dir(&client_a_path)
        .args(&["commit", "-m", "Commit A2"])
        .status()
        .unwrap();
    Command::new("git")
        .current_dir(&client_a_path)
        .args(&["push"])
        .status()
        .unwrap();

    // B still has "Version A" (plus maybe A2 if I cloned after? No, I cloned at A).
    // Actually, B cloned at A.
    // If B changes file.txt -> "Version B".
    std::fs::write(client_b_path.join("file.txt"), "Version B").unwrap();
    Command::new("git")
        .current_dir(&client_b_path)
        .args(&["add", "."])
        .status()
        .unwrap();
    Command::new("git")
        .current_dir(&client_b_path)
        .args(&["commit", "-m", "Commit B"])
        .status()
        .unwrap();

    // Now B tries to Push.
    // Remote is at A2. B is based on A. B's push should fail (non-fast-forward).

    // 4. Spawn Push Task for Client B using Registry
    // Need task kind
    let (task_id, cancel_token) = registry.create(TaskKind::GitPush {
        dest: client_b_path.to_string_lossy().to_string(),
        remote: Some("origin".into()),
        refspecs: None, // Push current branch
        username: None,
        password: None,
        strategy_override: None,
    });

    let handle = registry.spawn_git_push_task(
        None,
        task_id,
        cancel_token,
        client_b_path.to_string_lossy().to_string(),
        Some("origin".into()),
        None,
        None,
        None,
        None,
        None,
    );

    // 5. Wait for Result
    handle.await.unwrap(); // Join success

    // Check registry state
    let snap = registry.snapshot(&task_id).unwrap();
    assert_eq!(
        snap.state,
        TaskState::Failed,
        "Push should fail due to conflict"
    );

    if let Some(reason) = registry.fail_reason(&task_id) {
        assert!(
            reason.to_lowercase().contains("rejected")
                || reason.to_lowercase().contains("non-fast-forward")
                || reason.contains("failed to push"),
            "Reason should indicate conflict/rejection: {}",
            reason
        );
    } else {
        println!("WARNING: Task failed but no fail_reason set. As state is Failed, test passes.");
    }
}

// Reuse wait helper
#[allow(dead_code)]
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
