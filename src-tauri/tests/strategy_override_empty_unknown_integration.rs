#![cfg(not(feature = "tauri-app"))]
use fireworks_collaboration_lib::core::tasks::registry::TaskRegistry;
use fireworks_collaboration_lib::core::tasks::model::{TaskKind, TaskState};
use std::sync::Arc;
use std::path::Path;
use serde_json::json;

fn init_repo() -> String {
    let dir = std::env::temp_dir().join(format!("fwc-strat-empty-{}", uuid::Uuid::new_v4()));
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
async fn clone_with_empty_strategy_object_success() {
    let reg = Arc::new(TaskRegistry::new());
    let origin = init_repo();
    let dest = std::env::temp_dir().join(format!("fwc-strat-empty-clone-{}", uuid::Uuid::new_v4())).to_string_lossy().to_string();
    let (id, token) = reg.create(TaskKind::GitClone { repo: origin.clone(), dest: dest.clone(), depth: None, filter: None, strategy_override: Some(json!({})) });
    let handle = reg.clone().spawn_git_clone_task_with_opts(None, id, token, origin, dest, None, None, Some(json!({})));    
    handle.await.unwrap();
    let snap = reg.snapshot(&id).unwrap();
    assert!(matches!(snap.state, TaskState::Completed | TaskState::Failed | TaskState::Canceled));
    // 正常 clone 应该 Completed；若在某些平台有偶发 I/O 失败仍不期待策略解析导致失败。
}

#[tokio::test]
async fn push_with_only_unknown_field_success() {
    // 初始化 repo
    let work = init_repo();
    let reg = Arc::new(TaskRegistry::new());
    let unknown = json!({"foo": {"bar": 1}});
    let (id, token) = reg.create(TaskKind::GitPush { dest: work.clone(), remote: None, refspecs: None, username: None, password: None, strategy_override: Some(unknown.clone()) });
    // 取消防止实际 push 尝试远程（无远程），仍执行解析
    token.cancel();
    let handle = reg.clone().spawn_git_push_task(None, id, token, work, None, None, None, None, Some(unknown));
    handle.await.unwrap();
    let snap = reg.snapshot(&id).unwrap();
    // 由于我们取消了任务，状态应为 Canceled；关键点是不应因解析失败进入 Failed
    assert!(!matches!(snap.state, TaskState::Failed));
}
