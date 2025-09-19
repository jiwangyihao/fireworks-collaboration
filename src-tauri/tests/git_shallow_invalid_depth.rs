#![cfg(not(feature = "tauri-app"))]

use std::sync::Arc;
use fireworks_collaboration_lib::core::tasks::registry::TaskRegistry;
use fireworks_collaboration_lib::core::tasks::model::{TaskKind, TaskState};
use serde_json::json;

fn wait_state(reg:&TaskRegistry, id:uuid::Uuid, target:TaskState, max_ms:u64)->bool {
    let mut elapsed=0; while elapsed<max_ms { if let Some(s)=reg.snapshot(&id){ if s.state==target { return true;} } std::thread::sleep(std::time::Duration::from_millis(25)); elapsed+=25; } false }

fn init_origin() -> String {
    // 创建一个本地源仓库（用于合法参数测试），非法参数测试只需进入解析即可失败，不依赖网络
    let dir = std::env::temp_dir().join(format!("fwc-invalid-depth-origin-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let repo = git2::Repository::init(&dir).unwrap();
    std::fs::write(dir.join("a.txt"), "a").unwrap();
    let mut idx = repo.index().unwrap(); idx.add_path(std::path::Path::new("a.txt")).unwrap(); idx.write().unwrap();
    let tree_id = idx.write_tree().unwrap(); let tree = repo.find_tree(tree_id).unwrap();
    let sig = git2::Signature::now("Tester","tester@example.com").unwrap();
    repo.commit(Some("HEAD"), &sig, &sig, "init", &tree, &[]).unwrap();
    dir.to_string_lossy().to_string()
}

#[tokio::test]
async fn clone_depth_zero_fails_protocol() {
    let reg = Arc::new(TaskRegistry::new());
    let origin = init_origin();
    let dest = std::env::temp_dir().join(format!("fwc-clone-depth0-{}", uuid::Uuid::new_v4())).to_string_lossy().to_string();
    let (id, token) = reg.create(TaskKind::GitClone { repo: origin.clone(), dest: dest.clone(), depth: None, filter: None, strategy_override: None });
    let handle = reg.clone().spawn_git_clone_task_with_opts(None, id, token, origin, dest, Some(json!(0)), None, None);
    let failed = wait_state(&reg, id, TaskState::Failed, 2000); assert!(failed, "depth=0 should fail quickly");
    handle.await.unwrap();
}

#[tokio::test]
async fn clone_depth_negative_fails_protocol() {
    let reg = Arc::new(TaskRegistry::new());
    let origin = init_origin();
    let dest = std::env::temp_dir().join(format!("fwc-clone-depth-neg-{}", uuid::Uuid::new_v4())).to_string_lossy().to_string();
    let (id, token) = reg.create(TaskKind::GitClone { repo: origin.clone(), dest: dest.clone(), depth: None, filter: None, strategy_override: None });
    // 传入一个负值 JSON
    let handle = reg.clone().spawn_git_clone_task_with_opts(None, id, token, origin, dest, Some(json!(-3)), None, None);
    let failed = wait_state(&reg, id, TaskState::Failed, 2000); assert!(failed, "negative depth should fail");
    handle.await.unwrap();
}

#[tokio::test]
async fn clone_depth_too_large_fails_protocol() {
    let reg = Arc::new(TaskRegistry::new());
    let origin = init_origin();
    let dest = std::env::temp_dir().join(format!("fwc-clone-depth-big-{}", uuid::Uuid::new_v4())).to_string_lossy().to_string();
    let (id, token) = reg.create(TaskKind::GitClone { repo: origin.clone(), dest: dest.clone(), depth: None, filter: None, strategy_override: None });
    let too_big = (i32::MAX as i64) + 1; // 超过上限触发 "depth too large"
    let handle = reg.clone().spawn_git_clone_task_with_opts(None, id, token, origin, dest, Some(json!(too_big)), None, None);
    let failed = wait_state(&reg, id, TaskState::Failed, 2000); assert!(failed, "too large depth should fail");
    handle.await.unwrap();
}
