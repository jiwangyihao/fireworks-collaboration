#![cfg(not(feature = "tauri-app"))]
//! 验证 clone filter 回退事件包含结构化 code 字段 partial_filter_fallback
use std::path::PathBuf; use std::process::Command; use std::sync::Arc;
use fireworks_collaboration_lib::core::tasks::registry::TaskRegistry;
use fireworks_collaboration_lib::core::tasks::model::{TaskKind, TaskState};
use fireworks_collaboration_lib::events::emitter::{AppHandle, peek_captured_events};

fn unique_dir(label:&str)->PathBuf { std::env::temp_dir().join(format!("fwc-partial-clone-code-{}-{}", label, uuid::Uuid::new_v4())) }

fn build_origin()->PathBuf { let dir = unique_dir("origin"); std::fs::create_dir_all(&dir).unwrap(); let run=|a:&[&str]|{assert!(Command::new("git").current_dir(&dir).args(a).status().unwrap().success());}; run(&["init","--quiet"]); run(&["config","user.email","a@example.com"]); run(&["config","user.name","A"]); std::fs::write(dir.join("f.txt"),"1").unwrap(); run(&["add","."]); run(&["commit","-m","c1"]); dir }

#[tokio::test]
async fn clone_filter_fallback_has_code() {
  let origin = build_origin();
  let dest = unique_dir("clone");
  let registry = Arc::new(TaskRegistry::new());
  let (id, token) = registry.create(TaskKind::GitClone { repo: origin.to_string_lossy().to_string(), dest: dest.to_string_lossy().to_string(), depth: None, filter: Some("blob:none".into()), strategy_override: None });
  let app = AppHandle;
  let handle = registry.spawn_git_clone_task_with_opts(Some(app), id, token, origin.to_string_lossy().to_string(), dest.to_string_lossy().to_string(), None, Some("blob:none".into()), None);
  let mut waited=0; while waited<6000 { if let Some(s)=registry.snapshot(&id) { if matches!(s.state, TaskState::Completed|TaskState::Failed|TaskState::Canceled){ break; } } tokio::time::sleep(std::time::Duration::from_millis(50)).await; waited+=50; }
  let snap = registry.snapshot(&id).unwrap(); assert_eq!(snap.state, TaskState::Completed);
  handle.await.unwrap();
  // 查找具有 code 字段的 fallback error 事件
  let mut found=false; let mut attempts=0; while attempts<20 && !found { let evs = peek_captured_events(); for (topic,json) in &evs { if topic=="task://error" && json.contains("partial_filter_fallback") && json.contains("fallback=full") { found=true; break; } } if !found { tokio::time::sleep(std::time::Duration::from_millis(40)).await; } attempts+=1; }
  assert!(found, "expected fallback error event with code partial_filter_fallback");
}
