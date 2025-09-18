#![cfg(not(feature = "tauri-app"))]
use fireworks_collaboration_lib::core::tasks::registry::TaskRegistry;
use fireworks_collaboration_lib::core::tasks::model::{TaskKind, TaskState};
use std::sync::Arc;
use std::path::Path;

fn init_local_repo() -> String {
    let base = std::env::temp_dir().join(format!("fwc-origin-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&base).unwrap();
    let repo = git2::Repository::init(&base).unwrap();
    // 写入一个文件并提交，避免空仓库潜在行为差异
    std::fs::write(base.join("README.md"), "test").unwrap();
    let mut index = repo.index().unwrap();
    index.add_path(Path::new("README.md")).unwrap();
    index.write().unwrap();
    let tree_id = index.write_tree().unwrap();
    let tree = repo.find_tree(tree_id).unwrap();
    let sig = git2::Signature::now("Tester","tester@example.com").unwrap();
    repo.commit(Some("HEAD"), &sig, &sig, "init", &tree, &[]).unwrap();
    base.to_string_lossy().to_string()
}

#[tokio::test]
async fn clone_with_valid_depth_and_filter_placeholder_succeeds() {
    // 使用本地临时仓库，避免网络不确定性
    let reg = Arc::new(TaskRegistry::new());
    let local_repo = init_local_repo();
    let dest = std::env::temp_dir().join(format!("fwc-clone-valid-{}", uuid::Uuid::new_v4())).to_string_lossy().to_string();
    let (id, token) = reg.create(TaskKind::GitClone { repo: local_repo.clone(), dest: dest.clone(), depth: Some(1), filter: Some("blob:none".into()), strategy_override: None });
    // 通过 with_opts 触发解析 + 验证
    let handle = reg.clone().spawn_git_clone_task_with_opts(None, id, token, local_repo, dest, Some(serde_json::json!(1)), Some("blob:none".into()), None);
    handle.await.unwrap();
    if let Some(snap)=reg.snapshot(&id){ assert!(!matches!(snap.state, TaskState::Failed), "unexpected immediate failure (params should be accepted)"); }
}

#[tokio::test]
async fn strategy_override_unknown_fields_not_error() {
    let reg = Arc::new(TaskRegistry::new());
    let local_repo = init_local_repo();
    let dest = std::env::temp_dir().join(format!("fwc-clone-strategy-{}", uuid::Uuid::new_v4())).to_string_lossy().to_string();
    let override_val = serde_json::json!({"http":{"followRedirects":true},"retry":{"max":2},"__unknown":123});
    let (id, token) = reg.create(TaskKind::GitClone { repo: local_repo.clone(), dest: dest.clone(), depth: None, filter: None, strategy_override: Some(override_val.clone()) });
    let handle = reg.clone().spawn_git_clone_task_with_opts(None, id, token, local_repo, dest, None, None, Some(override_val));
    handle.await.unwrap();
    if let Some(snap)=reg.snapshot(&id){ assert!(!matches!(snap.state, TaskState::Failed), "unexpected direct failure (strategy override parsing)"); }
}
