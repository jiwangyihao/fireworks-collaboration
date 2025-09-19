#![cfg(not(feature = "tauri-app"))]
//! 验证非法 filter 导致解析失败，不应出现 partial_filter_fallback code 事件
use std::path::PathBuf; use std::process::Command; use std::sync::Arc;
use fireworks_collaboration_lib::core::tasks::registry::TaskRegistry;
use fireworks_collaboration_lib::core::tasks::model::{TaskKind, TaskState};
use fireworks_collaboration_lib::events::emitter::{AppHandle, peek_captured_events, drain_captured_events};

fn unique_dir(label:&str)->PathBuf { std::env::temp_dir().join(format!("fwc-partial-fetch-invalid-{}-{}", label, uuid::Uuid::new_v4())) }
fn build_origin()->(PathBuf, PathBuf) { let origin = unique_dir("origin"); std::fs::create_dir_all(&origin).unwrap(); let run=|a:&[&str]|{assert!(Command::new("git").current_dir(&origin).args(a).status().unwrap().success());}; run(&["init","--quiet"]); run(&["config","user.email","a@example.com"]); run(&["config","user.name","A"]); std::fs::write(origin.join("f.txt"),"1").unwrap(); run(&["add","."]); run(&["commit","-m","c1"]); let work = unique_dir("work"); assert!(Command::new("git").args(["clone", origin.to_string_lossy().as_ref(), work.to_string_lossy().as_ref()]).status().unwrap().success()); (work, origin) }

#[tokio::test]
async fn fetch_invalid_filter_fails_without_fallback_code() {
  let (work,_origin) = build_origin();
  let registry = Arc::new(TaskRegistry::new());
  // 直接传非法 filter
  let (id, token) = registry.create(TaskKind::GitFetch { repo: "origin".into(), dest: work.to_string_lossy().to_string(), depth: None, filter: Some("weird:rule".into()), strategy_override: None });
  let app = AppHandle; let handle = registry.spawn_git_fetch_task_with_opts(Some(app), id, token, "origin".into(), work.to_string_lossy().to_string(), None, None, Some("weird:rule".into()), None);
  let mut waited=0; while waited<3000 { if let Some(s)=registry.snapshot(&id) { if matches!(s.state, TaskState::Completed|TaskState::Failed|TaskState::Canceled){ break; } } tokio::time::sleep(std::time::Duration::from_millis(40)).await; waited+=40; }
  let snap = registry.snapshot(&id).unwrap(); assert_eq!(snap.state, TaskState::Failed, "invalid filter should cause Failed state");
  handle.await.unwrap();
  let evs = peek_captured_events();
  // 需要存在一个错误事件（非法参数），但不应包含 partial_filter_fallback code
  let mut has_protocol=false; let mut has_fallback_code=false; for (topic,json) in &evs { if topic=="task://error" { if json.contains("unsupported filter") { has_protocol=true; } if json.contains("partial_filter_fallback") { has_fallback_code=true; } } }
  let _ = drain_captured_events();
  assert!(has_protocol, "expected protocol error for invalid filter");
  assert!(!has_fallback_code, "should not emit fallback code for invalid filter");
}
