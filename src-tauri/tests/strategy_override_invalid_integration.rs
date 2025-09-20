#![cfg(not(feature = "tauri-app"))]
use std::path::Path;
use fireworks_collaboration_lib::core::tasks::registry::TaskRegistry;
use fireworks_collaboration_lib::core::tasks::model::{TaskKind, TaskState};
use std::sync::Arc;
use serde_json::json;

fn init_origin() -> String {
    let dir = std::env::temp_dir().join(format!("fwc-strat-origin-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let repo = git2::Repository::init(&dir).unwrap();
    std::fs::write(dir.join("f.txt"), "1").unwrap();
    let mut idx = repo.index().unwrap();
    idx.add_path(Path::new("f.txt")).unwrap();
    idx.write().unwrap();
    let tree_id = idx.write_tree().unwrap();
    let tree = repo.find_tree(tree_id).unwrap();
    let sig = git2::Signature::now("Tester","tester@example.com").unwrap();
    repo.commit(Some("HEAD"), &sig, &sig, "init", &tree, &[]).unwrap();
    dir.to_string_lossy().to_string()
}

#[tokio::test]
async fn clone_invalid_http_max_redirects_fails() {
    let reg = Arc::new(TaskRegistry::new());
    let origin = init_origin();
    let dest = std::env::temp_dir().join(format!("fwc-strat-clone-fail-{}", uuid::Uuid::new_v4())).to_string_lossy().to_string();
    let bad = json!({"http": {"maxRedirects": 999}});
    let (id, token) = reg.create(TaskKind::GitClone { repo: origin.clone(), dest: dest.clone(), depth: None, filter: None, strategy_override: Some(bad.clone()) });
    let handle = reg.clone().spawn_git_clone_task_with_opts(None, id, token, origin, dest, None, None, Some(bad));
    handle.await.unwrap();
    let snap = reg.snapshot(&id).unwrap();
    assert!(matches!(snap.state, TaskState::Failed));
}

#[tokio::test]
async fn push_invalid_retry_factor_fails() {
    // init repo
    let work = std::env::temp_dir().join(format!("fwc-strat-push-fail-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&work).unwrap();
    let repo = git2::Repository::init(&work).unwrap();
    std::fs::write(work.join("a.txt"), "1").unwrap();
    let mut idx = repo.index().unwrap();
    idx.add_path(Path::new("a.txt")).unwrap();
    idx.write().unwrap();
    let tree_id = idx.write_tree().unwrap();
    let tree = repo.find_tree(tree_id).unwrap();
    let sig = git2::Signature::now("Tester","tester@example.com").unwrap();
    repo.commit(Some("HEAD"), &sig, &sig, "c1", &tree, &[]).unwrap();

    let reg = Arc::new(TaskRegistry::new());
    let bad = json!({"retry": {"factor": 50.0}}); // out of range
    let (id, token) = reg.create(TaskKind::GitPush { dest: work.to_string_lossy().to_string(), remote: None, refspecs: None, username: None, password: None, strategy_override: Some(bad.clone()) });
    let handle = reg.clone().spawn_git_push_task(None, id, token, work.to_string_lossy().to_string(), None, None, None, None, Some(bad));
    handle.await.unwrap();
    let snap = reg.snapshot(&id).unwrap();
    assert!(matches!(snap.state, TaskState::Failed));
}
