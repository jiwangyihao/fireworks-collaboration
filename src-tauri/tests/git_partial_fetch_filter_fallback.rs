#![cfg(not(feature = "tauri-app"))]
//! 确认 fetch 带 filter 回退后任务仍成功完成（非阻断）
use std::path::PathBuf; use std::process::Command; use std::sync::Arc;
use fireworks_collaboration_lib::core::tasks::registry::TaskRegistry;
use fireworks_collaboration_lib::core::tasks::model::{TaskKind, TaskState};

fn unique_dir(label:&str)->PathBuf { std::env::temp_dir().join(format!("fwc-partial-fetch-fallback-{}-{}", label, uuid::Uuid::new_v4())) }

fn build_origin(commits:u32)->PathBuf {
    let dir = unique_dir("origin"); std::fs::create_dir_all(&dir).unwrap();
    let run = |args:&[&str]|{ let st=Command::new("git").current_dir(&dir).args(args).status().unwrap(); assert!(st.success()); };
    run(&["init","--quiet"]); run(&["config","user.email","a@example.com"]); run(&["config","user.name","A"]);
    for i in 1..=commits { std::fs::write(dir.join(format!("f{}.txt",i)), i.to_string()).unwrap(); run(&["add","."]); run(&["commit","-m", &format!("c{}", i)]); }
    dir
}

#[tokio::test]
async fn fetch_with_filter_fallbacks_successfully() {
    let origin = build_origin(3);
    // initial clone
    let work = unique_dir("work"); let st = Command::new("git").args(["clone", origin.to_string_lossy().as_ref(), work.to_string_lossy().as_ref()]).status().unwrap(); assert!(st.success());
    // extra commit to fetch
    std::fs::write(origin.join("new.txt"), "x").unwrap(); let st = Command::new("git").current_dir(&origin).args(["add","new.txt"]).status().unwrap(); assert!(st.success()); let st = Command::new("git").current_dir(&origin).args(["commit","-m","extra"]).status().unwrap(); assert!(st.success());

    let registry = Arc::new(TaskRegistry::new());
    let (id, token) = registry.create(TaskKind::GitFetch { repo: "origin".into(), dest: work.to_string_lossy().to_string(), depth: None, filter: Some("blob:none".into()), strategy_override: None });
    let handle = registry.spawn_git_fetch_task_with_opts(None, id, token, "origin".into(), work.to_string_lossy().to_string(), None, None, Some("blob:none".into()), None);
    let _ = fireworks_collaboration_lib::tests_support::wait::wait_task_terminal(&registry,&id,50,160).await;
    let snap = registry.snapshot(&id).unwrap(); assert_eq!(snap.state, TaskState::Completed, "fetch with filter fallback should complete successfully");
    handle.await.unwrap();
    assert!(work.join(".git").exists(), "work should remain a git repo");
}
