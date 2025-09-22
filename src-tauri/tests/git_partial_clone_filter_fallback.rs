#![cfg(not(feature = "tauri-app"))]
//! P2.2d: Partial clone filter 回退测试（当前阶段不真正启用 partial，需发送非阻断提示并成功完成克隆）
use std::path::PathBuf;
use std::process::Command;
use fireworks_collaboration_lib::core::tasks::registry::TaskRegistry;
use fireworks_collaboration_lib::core::tasks::registry::SharedTaskRegistry;
use fireworks_collaboration_lib::core::tasks::model::{TaskKind, TaskState};
use std::sync::Arc;

fn unique_dir(label:&str)->PathBuf { std::env::temp_dir().join(format!("fwc-partial-fallback-{}-{}", label, uuid::Uuid::new_v4())) }

fn build_origin(commits:u32)->PathBuf {
    let dir = unique_dir("origin");
    std::fs::create_dir_all(&dir).unwrap();
    let run = |args:&[&str]| { let st = Command::new("git").current_dir(&dir).args(args).status().unwrap(); assert!(st.success(), "git {:?} failed", args); };
    run(&["init","--quiet"]);
    run(&["config","user.email","a@example.com"]);
    run(&["config","user.name","A"]);
    for i in 1..=commits { std::fs::write(dir.join(format!("f{}.txt", i)), format!("{}", i)).unwrap(); run(&["add","."]); run(&["commit","-m", &format!("c{}", i)]); }
    dir
}

#[tokio::test]
async fn clone_with_filter_fallbacks_successfully() {
    let origin = build_origin(3);
    let dest = unique_dir("clone");
    let registry: SharedTaskRegistry = Arc::new(TaskRegistry::new());
    let (id, token) = registry.create(TaskKind::GitClone { repo: origin.to_string_lossy().to_string(), dest: dest.to_string_lossy().to_string(), depth: None, filter: Some("blob:none".into()), strategy_override: None });
    // 直接调用带 opts 的 spawn，传入 filter 占位；应完成而不是 Failed
    let handle = registry.spawn_git_clone_task_with_opts(None, id, token.clone(), origin.to_string_lossy().to_string(), dest.to_string_lossy().to_string(), None, Some("blob:none".into()), None);
    // 简单轮询等待完成
    let mut waited = 0u64; let max = 8000u64; // 8s 保险
    while waited < max { if let Some(snap) = registry.snapshot(&id) { match snap.state { TaskState::Completed | TaskState::Failed | TaskState::Canceled => break, _=>{} } } tokio::time::sleep(std::time::Duration::from_millis(50)).await; waited += 50; }
    let snap = registry.snapshot(&id).expect("snapshot");
    assert_eq!(snap.state, TaskState::Completed, "clone with filter fallback should complete successfully");
    handle.await.unwrap();
    // 验证仓库已创建
    assert!(dest.join(".git").exists(), "dest should be a git repo");
}
