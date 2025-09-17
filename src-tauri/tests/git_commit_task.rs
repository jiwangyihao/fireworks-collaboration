use std::{fs, path::PathBuf, time::Duration};
use uuid::Uuid;
use fireworks_collaboration_lib::core::tasks::{TaskRegistry, TaskKind};
use fireworks_collaboration_lib::core::tasks::model::TaskState;

fn temp_repo() -> PathBuf { let p = std::env::temp_dir().join(format!("fwc_commit_task_{}", Uuid::new_v4())); fs::create_dir_all(&p).unwrap(); p }

#[tokio::test(flavor = "current_thread")] 
async fn spawn_git_commit_task_flow() {
    std::env::set_var("FWC_E2E_DISABLE", "true");
    let reg = std::sync::Arc::new(TaskRegistry::new());
    // prepare repo with one staged file
    let repo_dir = temp_repo();
    let repo = git2::Repository::init(&repo_dir).unwrap();
    std::fs::write(repo_dir.join("z.txt"), b"z").unwrap();
    let mut index = repo.index().unwrap();
    index.add_path(std::path::Path::new("z.txt")).unwrap();
    index.write().unwrap();

    let (id, token) = reg.create(TaskKind::GitCommit { dest: repo_dir.to_string_lossy().to_string(), message: "feat: z".into(), allow_empty: false, author_name: None, author_email: None });
    // simulate app = None (no events emission required for logic)
    let handle = reg.spawn_git_commit_task(None, id, token, repo_dir.to_string_lossy().to_string(), "feat: z".into(), false, None, None);

    // wait for completion
    let mut waited = 0u64;
    let done = loop {
        if let Some(s) = reg.snapshot(&id) { if matches!(s.state, TaskState::Completed|TaskState::Failed) { break s.state; } }
        tokio::time::sleep(Duration::from_millis(50)).await;
        waited += 50; if waited > 3000 { panic!("timeout waiting commit task"); }
    };
    assert!(matches!(done, TaskState::Completed), "task should complete");
    handle.await.unwrap();

    // verify commit exists
    let repo2 = git2::Repository::open(&repo_dir).unwrap();
    let head = repo2.head().unwrap();
    assert!(head.target().is_some(), "HEAD should point to commit");
}

#[tokio::test(flavor = "current_thread")] 
async fn spawn_git_commit_task_canceled() {
    std::env::set_var("FWC_E2E_DISABLE", "true");
    let reg = std::sync::Arc::new(TaskRegistry::new());
    let repo_dir = temp_repo();
    let repo = git2::Repository::init(&repo_dir).unwrap();
    std::fs::write(repo_dir.join("c.txt"), b"c").unwrap();
    let mut index = repo.index().unwrap();
    index.add_path(std::path::Path::new("c.txt")).unwrap();
    index.write().unwrap();
    let (id, token) = reg.create(TaskKind::GitCommit { dest: repo_dir.to_string_lossy().to_string(), message: "feat: c".into(), allow_empty: false, author_name: None, author_email: None });
    // cancel before spawn
    token.cancel();
    let handle = reg.spawn_git_commit_task(None, id, token.clone(), repo_dir.to_string_lossy().to_string(), "feat: c".into(), false, None, None);
    // wait briefly
    let mut waited = 0u64;
    let state = loop {
        if let Some(s) = reg.snapshot(&id) { if matches!(s.state, TaskState::Canceled|TaskState::Failed|TaskState::Completed) { break s.state; } }
        tokio::time::sleep(Duration::from_millis(30)).await; waited += 30; if waited > 1500 { panic!("timeout waiting canceled state"); }
    };
    assert!(matches!(state, TaskState::Canceled), "should be canceled early");
    handle.await.unwrap();
}
