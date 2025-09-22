#![cfg(not(feature = "tauri-app"))]
//! 验证 clone filter 回退事件包含结构化 code 字段 partial_filter_fallback
use std::path::PathBuf; use std::process::Command; use std::sync::Arc;
use fireworks_collaboration_lib::core::tasks::registry::TaskRegistry;
use fireworks_collaboration_lib::core::tasks::model::{TaskKind, TaskState};
use fireworks_collaboration_lib::events::emitter::AppHandle;
use fireworks_collaboration_lib::events::structured::{set_global_event_bus, MemoryEventBus};
use fireworks_collaboration_lib::tests_support::event_assert::{assert_partial_capability, assert_partial_fallback};

fn unique_dir(label:&str)->PathBuf { std::env::temp_dir().join(format!("fwc-partial-clone-code-{}-{}", label, uuid::Uuid::new_v4())) }

fn build_origin()->PathBuf { let dir = unique_dir("origin"); std::fs::create_dir_all(&dir).unwrap(); let run=|a:&[&str]|{assert!(Command::new("git").current_dir(&dir).args(a).status().unwrap().success());}; run(&["init","--quiet"]); run(&["config","user.email","a@example.com"]); run(&["config","user.name","A"]); std::fs::write(dir.join("f.txt"),"1").unwrap(); run(&["add","."]); run(&["commit","-m","c1"]); dir }

#[tokio::test]
async fn clone_filter_fallback_has_code() {
  let origin = build_origin();
  let dest = unique_dir("clone");
  let registry = Arc::new(TaskRegistry::new());
  let (id, token) = registry.create(TaskKind::GitClone { repo: origin.to_string_lossy().to_string(), dest: dest.to_string_lossy().to_string(), depth: None, filter: Some("blob:none".into()), strategy_override: None });
  let app = AppHandle;
  let _ = set_global_event_bus(std::sync::Arc::new(MemoryEventBus::new()));
  let handle = registry.spawn_git_clone_task_with_opts(Some(app), id, token, origin.to_string_lossy().to_string(), dest.to_string_lossy().to_string(), None, Some("blob:none".into()), None);
  let _ = fireworks_collaboration_lib::tests_support::wait::wait_task_terminal(&registry,&id,50,120).await;
  let snap = registry.snapshot(&id).unwrap(); assert_eq!(snap.state, TaskState::Completed);
  handle.await.unwrap();
  // 断言 fallback (shallow=false) 且 capability(supported=false) 也已发布
  assert_partial_fallback(&id.to_string(), Some(false));
  assert_partial_capability(&id.to_string(), false);
}
