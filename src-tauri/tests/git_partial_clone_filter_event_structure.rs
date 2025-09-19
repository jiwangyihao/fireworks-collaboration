#![cfg(not(feature = "tauri-app"))]
//! 验证 clone fallback 事件结构字段完备：code/category/message 并且不含 retriedTimes
use std::path::PathBuf; use std::process::Command; use std::sync::Arc;
use fireworks_collaboration_lib::core::tasks::registry::TaskRegistry;
use fireworks_collaboration_lib::core::tasks::model::{TaskKind, TaskState};
use fireworks_collaboration_lib::events::emitter::{AppHandle, peek_captured_events, drain_captured_events};

fn unique_dir(label:&str)->PathBuf { std::env::temp_dir().join(format!("fwc-partial-clone-struct-{}-{}", label, uuid::Uuid::new_v4())) }
fn build_origin()->PathBuf { let dir = unique_dir("origin"); std::fs::create_dir_all(&dir).unwrap(); let run=|a:&[&str]|{assert!(Command::new("git").current_dir(&dir).args(a).status().unwrap().success());}; run(&["init","--quiet"]); run(&["config","user.email","a@example.com"]); run(&["config","user.name","A"]); std::fs::write(dir.join("f.txt"),"1").unwrap(); run(&["add","."]); run(&["commit","-m","c1"]); dir }

#[tokio::test]
async fn clone_fallback_event_structure() {
  std::env::remove_var("FWC_PARTIAL_FILTER_CAPABLE"); // 确保触发 fallback
  let origin = build_origin(); let dest = unique_dir("clone");
  let registry = Arc::new(TaskRegistry::new());
  let (id, token) = registry.create(TaskKind::GitClone { repo: origin.to_string_lossy().to_string(), dest: dest.to_string_lossy().to_string(), depth: None, filter: Some("blob:none".into()), strategy_override: None });
  let app = AppHandle; let handle = registry.spawn_git_clone_task_with_opts(Some(app), id, token, origin.to_string_lossy().to_string(), dest.to_string_lossy().to_string(), None, Some("blob:none".into()), None);
  let mut waited=0; while waited<5000 { if let Some(s)=registry.snapshot(&id) { if matches!(s.state, TaskState::Completed|TaskState::Failed|TaskState::Canceled){ break; } } tokio::time::sleep(std::time::Duration::from_millis(50)).await; waited+=50; }
  let snap = registry.snapshot(&id).unwrap(); assert_eq!(snap.state, TaskState::Completed);
  handle.await.unwrap();
  let mut found_json=None; for (topic,raw) in peek_captured_events() { if topic=="task://error" && raw.contains("partial_filter_fallback") { found_json=Some(raw); break; } }
  let payload = found_json.expect("fallback event not found");
  let v: serde_json::Value = serde_json::from_str(&payload).expect("valid json");
  assert_eq!(v["code"], "partial_filter_fallback");
  assert_eq!(v["category"], "Protocol");
  assert!(v["message"].as_str().unwrap().contains("fallback=full"));
  assert!(v.get("retriedTimes").is_none(), "fallback should not include retriedTimes");
  let _ = drain_captured_events();
}
