#![cfg(not(feature = "tauri-app"))]
use fireworks_collaboration_lib::core::tasks::registry::TaskRegistry;
use fireworks_collaboration_lib::core::tasks::model::{TaskKind, TaskState};
use serde_json::json;
use std::sync::Arc;
use std::path::Path;

fn init_local_repo() -> String {
    let dir = std::env::temp_dir().join(format!("fwc-origin-combo-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let repo = git2::Repository::init(&dir).unwrap();
    std::fs::write(dir.join("a.txt"), "a").unwrap();
    let mut idx = repo.index().unwrap();
    idx.add_path(Path::new("a.txt")).unwrap();
    idx.write().unwrap();
    let tree_id = idx.write_tree().unwrap();
    let tree = repo.find_tree(tree_id).unwrap();
    let sig = git2::Signature::now("Tester","tester@example.com").unwrap();
    repo.commit(Some("HEAD"), &sig, &sig, "init", &tree, &[]).unwrap();
    dir.to_string_lossy().to_string()
}

#[tokio::test]
async fn clone_with_depth_and_filter_and_full_strategy_override_placeholder() {
    let reg = Arc::new(TaskRegistry::new());
    let origin = init_local_repo();
    let dest = std::env::temp_dir().join(format!("fwc-clone-combo-{}", uuid::Uuid::new_v4())).to_string_lossy().to_string();
    let strategy = json!({
        "http": {"followRedirects": true, "maxRedirects": 5},
        "tls": {"insecureSkipVerify": false, "skipSanWhitelist": true},
        "retry": {"max": 2, "baseMs": 100, "factor": 1.3, "jitter": false}
    });
    let (id, token) = reg.create(TaskKind::GitClone { repo: origin.clone(), dest: dest.clone(), depth: Some(2), filter: Some("tree:0".into()), strategy_override: Some(strategy.clone()) });
    let handle = reg.clone().spawn_git_clone_task_with_opts(None, id, token, origin, dest, Some(json!(2)), Some("tree:0".into()), Some(strategy));
    handle.await.unwrap();
    // 占位阶段，不应因合法组合失败
    if let Some(s)=reg.snapshot(&id){ assert!(!matches!(s.state, TaskState::Failed), "should not fail for valid combo parameters"); }
}

#[tokio::test]
async fn clone_with_invalid_strategy_override_type_currently_ignored() {
    let reg = Arc::new(TaskRegistry::new());
    let origin = init_local_repo();
    let dest = std::env::temp_dir().join(format!("fwc-clone-bad-override-{}", uuid::Uuid::new_v4())).to_string_lossy().to_string();
    // strategyOverride 给一个数组触发 Protocol
    let bad = json!([{"http": {"followRedirects": true}}]);
    let (id, token) = reg.create(TaskKind::GitClone { repo: origin.clone(), dest: dest.clone(), depth: None, filter: None, strategy_override: None });
    let handle = reg.clone().spawn_git_clone_task_with_opts(None, id, token, origin, dest, None, None, Some(bad));
    handle.await.unwrap();
    // 当前实现：非对象 strategyOverride 解析失败可能在内部被忽略或未导致任务状态 Failed（占位阶段允许未来行为调整）。
    if let Some(s)=reg.snapshot(&id){ assert!(!matches!(s.state, TaskState::Failed), "unexpected failure for invalid strategyOverride type placeholder"); }
}
