#![cfg(not(feature = "tauri-app"))]
//! capability=1 时非法 filter 仍应解析失败并 Failed，且无 fallback code。
use std::path::PathBuf; use std::process::Command; use std::sync::Arc;
use fireworks_collaboration_lib::core::tasks::registry::TaskRegistry;
use fireworks_collaboration_lib::core::tasks::model::{TaskKind, TaskState};
use fireworks_collaboration_lib::events::emitter::{AppHandle, peek_captured_events, drain_captured_events};

fn unique_dir(label:&str)->PathBuf { std::env::temp_dir().join(format!("fwc-partial-fetch-invalid-cap-{}-{}", label, uuid::Uuid::new_v4())) }
fn build_origin_and_work()->(PathBuf, PathBuf) {
  let origin = unique_dir("origin"); std::fs::create_dir_all(&origin).unwrap(); let run=|a:&[&str]|{assert!(Command::new("git").current_dir(&origin).args(a).status().unwrap().success());};
  run(&["init","--quiet"]); run(&["config","user.email","a@example.com"]); run(&["config","user.name","A"]);
  std::fs::write(origin.join("f.txt"),"1").unwrap(); run(&["add","."]); run(&["commit","-m","c1"]);
  let work = unique_dir("work"); assert!(Command::new("git").args(["clone", origin.to_string_lossy().as_ref(), work.to_string_lossy().as_ref()]).status().unwrap().success());
  (work, origin)
}

#[tokio::test]
async fn fetch_invalid_filter_capability_enabled_still_fails() {
  std::env::set_var("FWC_PARTIAL_FILTER_CAPABLE","1");
  let (work,_origin) = build_origin_and_work();
  let registry = Arc::new(TaskRegistry::new());
  let (id, token) = registry.create(TaskKind::GitFetch { repo: "origin".into(), dest: work.to_string_lossy().to_string(), depth: None, filter: Some("bad:filter".into()), strategy_override: None });
  let app = AppHandle; let handle = registry.spawn_git_fetch_task_with_opts(Some(app), id, token, "origin".into(), work.to_string_lossy().to_string(), None, None, Some("bad:filter".into()), None);
  let mut waited=0; while waited<3000 { if let Some(s)=registry.snapshot(&id) { if matches!(s.state, TaskState::Completed|TaskState::Failed|TaskState::Canceled){ break; } } tokio::time::sleep(std::time::Duration::from_millis(50)).await; waited+=50; }
  let snap = registry.snapshot(&id).unwrap(); assert_eq!(snap.state, TaskState::Failed, "invalid filter should fail even when capability=1");
  handle.await.unwrap();
  let mut has_protocol=false; let mut has_fallback=false; for (topic,json) in peek_captured_events() { if topic=="task://error" { if json.contains("unsupported filter") { has_protocol=true; } if json.contains("partial_filter_fallback") { has_fallback=true; } } }
  assert!(has_protocol, "expected protocol error");
  assert!(!has_fallback, "should not emit fallback code for invalid filter");
  let _ = drain_captured_events();
  std::env::remove_var("FWC_PARTIAL_FILTER_CAPABLE");
}
