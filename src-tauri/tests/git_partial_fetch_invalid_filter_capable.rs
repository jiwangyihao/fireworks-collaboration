#![cfg(not(feature = "tauri-app"))]
//! capability=1 时非法 filter 仍应解析失败并 Failed，且无 fallback code。
use std::path::PathBuf; use std::process::Command; use std::sync::Arc;
use fireworks_collaboration_lib::core::tasks::registry::TaskRegistry;
use fireworks_collaboration_lib::core::tasks::model::{TaskKind, TaskState};
use fireworks_collaboration_lib::events::emitter::AppHandle;
use fireworks_collaboration_lib::events::structured::{set_global_event_bus, MemoryEventBus};
use fireworks_collaboration_lib::tests_support::event_assert::{assert_partial_unsupported, assert_no_partial_capability, assert_no_partial_fallback};

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
  std::env::set_var("FWC_PARTIAL_FILTER_SUPPORTED","1");
  let (work,_origin) = build_origin_and_work();
  let registry = Arc::new(TaskRegistry::new());
  let (id, token) = registry.create(TaskKind::GitFetch { repo: "origin".into(), dest: work.to_string_lossy().to_string(), depth: None, filter: Some("bad:filter".into()), strategy_override: None });
  let app = AppHandle; let _ = set_global_event_bus(std::sync::Arc::new(MemoryEventBus::new())); let handle = registry.spawn_git_fetch_task_with_opts(Some(app), id, token, "origin".into(), work.to_string_lossy().to_string(), None, None, Some("bad:filter".into()), None);
  let _ = fireworks_collaboration_lib::tests_support::wait::wait_task_terminal(&registry,&id,50,60).await; // ≈3s 上限
  let snap = registry.snapshot(&id).unwrap(); assert_eq!(snap.state, TaskState::Failed, "invalid filter should fail even when capability=1");
  handle.await.unwrap();
  // 断言：有 unsupported 事件；无 capability（解析失败不应产生 capability）且无 fallback
  assert_partial_unsupported(&id.to_string(), Some("unsupported filter"));
  assert_no_partial_capability(&id.to_string());
  assert_no_partial_fallback(&id.to_string());
  std::env::remove_var("FWC_PARTIAL_FILTER_CAPABLE");
  std::env::remove_var("FWC_PARTIAL_FILTER_SUPPORTED");
}
