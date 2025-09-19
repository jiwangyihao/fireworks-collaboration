#![cfg(not(feature = "tauri-app"))]
//! 验证 fetch 仅 filter (无 depth) 情况下的回退事件：应出现 fallback=full 提示
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use fireworks_collaboration_lib::core::tasks::registry::TaskRegistry;
use fireworks_collaboration_lib::core::tasks::model::{TaskKind, TaskState};
use fireworks_collaboration_lib::events::emitter::{AppHandle, peek_captured_events};

fn unique_dir(label:&str)->PathBuf { std::env::temp_dir().join(format!("fwc-partial-fetch-only-{}-{}", label, uuid::Uuid::new_v4())) }

fn build_repo_with_origin()->(PathBuf, PathBuf) {
    // origin
    let origin = unique_dir("origin"); std::fs::create_dir_all(&origin).unwrap();
    let run_origin = |args:&[&str]|{ let st=Command::new("git").current_dir(&origin).args(args).status().unwrap(); assert!(st.success()); };
    run_origin(&["init","--quiet"]); run_origin(&["config","user.email","a@example.com"]); run_origin(&["config","user.name","A"]);
    for i in 1..=2 { std::fs::write(origin.join(format!("f{}.txt",i)), i.to_string()).unwrap(); run_origin(&["add","."]); run_origin(&["commit","-m", &format!("c{}", i)]); }
    // clone full once to local working repo
    let work = unique_dir("work");
    let st = Command::new("git").arg("clone").arg(&origin).arg(&work).status().unwrap(); assert!(st.success());
    (work, origin)
}

#[tokio::test]
async fn fetch_filter_only_fallback_full_message() {
    let (work, origin) = build_repo_with_origin();
    // Add one more commit to origin so fetch has something to negotiate
    std::fs::write(origin.join("new.txt"), "x").unwrap();
    let st = Command::new("git").current_dir(&origin).args(["add","new.txt"]).status().unwrap(); assert!(st.success());
    let st = Command::new("git").current_dir(&origin).args(["commit","-m","extra"]).status().unwrap(); assert!(st.success());

    let registry = Arc::new(TaskRegistry::new());
    let (id, token) = registry.create(TaskKind::GitFetch { repo: "origin".into(), dest: work.to_string_lossy().to_string(), depth: None, filter: Some("blob:none".into()), strategy_override: None });
    let app = AppHandle;
    let handle = registry.spawn_git_fetch_task_with_opts(Some(app), id, token, "origin".into(), work.to_string_lossy().to_string(), None, None, Some("blob:none".into()), None);
    // Wait for completion
    let mut waited=0; while waited<8000 { if let Some(s)=registry.snapshot(&id) { if matches!(s.state, TaskState::Completed|TaskState::Failed|TaskState::Canceled){ break; } } tokio::time::sleep(std::time::Duration::from_millis(50)).await; waited+=50; }
    let snap = registry.snapshot(&id).unwrap(); assert_eq!(snap.state, TaskState::Completed, "fetch should complete");
    handle.await.unwrap();
    // Poll captured events
    let mut found=false; let mut attempts=0; while attempts<20 && !found { let evs=peek_captured_events(); for (topic,json) in &evs { if topic=="task://error" && json.contains("fallback=full") && json.contains("GitFetch") { found=true; break; } } if !found { tokio::time::sleep(std::time::Duration::from_millis(50)).await; } attempts+=1; }
    assert!(found, "expected fallback=full protocol error event for filter-only fetch");
}
