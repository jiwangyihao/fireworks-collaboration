#![cfg(not(feature = "tauri-app"))]
//! 基线：fetch 无 filter 不应出现 fallback 提示
use std::path::PathBuf; use std::process::Command; use std::sync::Arc;
use fireworks_collaboration_lib::core::tasks::registry::TaskRegistry;
use fireworks_collaboration_lib::core::tasks::model::{TaskKind, TaskState};
use fireworks_collaboration_lib::events::emitter::{AppHandle, peek_captured_events, drain_captured_events};

fn unique_dir(label:&str)->PathBuf { std::env::temp_dir().join(format!("fwc-partial-fetch-base-{}-{}", label, uuid::Uuid::new_v4())) }

fn build_repo_with_origin()->(PathBuf, PathBuf) {
    let origin = unique_dir("origin"); std::fs::create_dir_all(&origin).unwrap();
    let run = |args:&[&str]|{ let st=Command::new("git").current_dir(&origin).args(args).status().unwrap(); assert!(st.success()); };
    run(&["init","--quiet"]); run(&["config","user.email","a@example.com"]); run(&["config","user.name","A"]);
    for i in 1..=2 { std::fs::write(origin.join(format!("f{}.txt",i)), i.to_string()).unwrap(); run(&["add","."]); run(&["commit","-m", &format!("c{}", i)]); }
    let work = unique_dir("work"); let st = Command::new("git").args(["clone", origin.to_string_lossy().as_ref(), work.to_string_lossy().as_ref()]).status().unwrap(); assert!(st.success());
    (work, origin)
}

#[tokio::test]
async fn fetch_no_filter_no_fallback_event() {
    let (work, origin) = build_repo_with_origin();
    // add commit so fetch negotiates
    std::fs::write(origin.join("new.txt"), "x").unwrap();
    let st = Command::new("git").current_dir(&origin).args(["add","new.txt"]).status().unwrap(); assert!(st.success());
    let st = Command::new("git").current_dir(&origin).args(["commit","-m","later"]).status().unwrap(); assert!(st.success());

    let registry = Arc::new(TaskRegistry::new());
    let (id, token) = registry.create(TaskKind::GitFetch { repo: "origin".into(), dest: work.to_string_lossy().to_string(), depth: None, filter: None, strategy_override: None });
    let app = AppHandle; let handle = registry.spawn_git_fetch_task_with_opts(Some(app), id, token, "origin".into(), work.to_string_lossy().to_string(), None, None, None, None);
    let mut waited=0; while waited<8000 { if let Some(s)=registry.snapshot(&id) { if matches!(s.state, TaskState::Completed|TaskState::Failed|TaskState::Canceled){ break; } } tokio::time::sleep(std::time::Duration::from_millis(50)).await; waited+=50; }
    let snap = registry.snapshot(&id).unwrap(); assert_eq!(snap.state, TaskState::Completed, "fetch should complete");
    handle.await.unwrap();
    let mut attempts=0; let mut seen_fallback=false; while attempts<6 { let evs=peek_captured_events(); for (topic,json) in &evs { if topic=="task://error" && (json.contains("fallback=full")||json.contains("fallback=shallow")) { seen_fallback=true; break; } } if seen_fallback { break; } if attempts<5 { tokio::time::sleep(std::time::Duration::from_millis(40)).await; } attempts+=1; }
    let _ = drain_captured_events();
    assert!(!seen_fallback, "should not see fallback messages when no filter provided for fetch");
}
