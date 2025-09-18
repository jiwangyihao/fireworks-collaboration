use std::fs;
use fireworks_collaboration_lib::core::tasks::{TaskRegistry};
use fireworks_collaboration_lib::core::tasks::model::{TaskKind, TaskState};

#[tokio::test]
async fn clone_invalid_depth_zero_fails() {
    let reg = std::sync::Arc::new(TaskRegistry::new());
    let (id, token) = reg.create(TaskKind::GitClone { repo: "https://example.com/repo.git".into(), dest: "./temp_repo_zero".into(), depth: Some(0), filter: None, strategy_override: None });
    let handle = reg.clone().spawn_git_clone_task_with_opts(None, id, token, "https://example.com/repo.git".into(), "./temp_repo_zero".into(), Some(serde_json::json!(0)), None, None);
    // block on handle
    handle.await.unwrap();
    let snap = reg.snapshot(&id).unwrap();
    assert!(matches!(snap.state, TaskState::Failed));
}

#[tokio::test]
async fn fetch_invalid_filter_fails() {
    let tmp = tempfile::tempdir().unwrap();
    let repo_path = tmp.path().join("repo");
    fs::create_dir_all(&repo_path).unwrap();
    let _r = git2::Repository::init(&repo_path).unwrap();
    let reg = std::sync::Arc::new(TaskRegistry::new());
    let (id, token) = reg.create(TaskKind::GitFetch { repo: "".into(), dest: repo_path.to_string_lossy().into(), depth: None, filter: Some("invalid:rule".into()), strategy_override: None });
    let handle = reg.clone().spawn_git_fetch_task_with_opts(None, id, token, "".into(), repo_path.to_string_lossy().into(), None, None, Some("invalid:rule".into()), None);
    handle.await.unwrap();
    let snap = reg.snapshot(&id).unwrap();
    assert!(matches!(snap.state, TaskState::Failed));
}
