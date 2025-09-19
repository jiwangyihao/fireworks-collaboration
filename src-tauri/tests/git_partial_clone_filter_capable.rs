#![cfg(not(feature = "tauri-app"))]
//! 当环境变量 FWC_PARTIAL_FILTER_CAPABLE=1 时，clone 请求 filter 不应发送 fallback 事件
use std::path::PathBuf; use std::process::Command; use std::sync::Arc;
use fireworks_collaboration_lib::core::tasks::registry::TaskRegistry;
use fireworks_collaboration_lib::core::tasks::model::{TaskKind, TaskState};
use fireworks_collaboration_lib::events::emitter::{AppHandle, peek_captured_events, drain_captured_events};

fn unique_dir(label:&str)->PathBuf { std::env::temp_dir().join(format!("fwc-partial-clone-capable-{}-{}", label, uuid::Uuid::new_v4())) }
fn build_origin()->PathBuf { let dir = unique_dir("origin"); std::fs::create_dir_all(&dir).unwrap(); let run=|a:&[&str]|{assert!(Command::new("git").current_dir(&dir).args(a).status().unwrap().success());}; run(&["init","--quiet"]); run(&["config","user.email","a@example.com"]); run(&["config","user.name","A"]); for i in 1..=3 { std::fs::write(dir.join(format!("f{}.txt",i)), i.to_string()).unwrap(); run(&["add","."]); run(&["commit","-m", &format!("c{}", i)]); } dir }

async fn run_case(depth: Option<u32>, filter: &str) {
  std::env::set_var("FWC_PARTIAL_FILTER_CAPABLE","1");
  let origin = build_origin();
  let dest = unique_dir("clone");
  let registry = Arc::new(TaskRegistry::new());
  let (id, token) = registry.create(TaskKind::GitClone { repo: origin.to_string_lossy().to_string(), dest: dest.to_string_lossy().to_string(), depth, filter: Some(filter.into()), strategy_override: None });
  let app = AppHandle;
  let depth_json = depth.map(|d| serde_json::json!(d));
  let handle = registry.spawn_git_clone_task_with_opts(Some(app), id, token, origin.to_string_lossy().to_string(), dest.to_string_lossy().to_string(), depth_json, Some(filter.into()), None);
  let mut waited=0; while waited<6000 { if let Some(s)=registry.snapshot(&id) { if matches!(s.state, TaskState::Completed|TaskState::Failed|TaskState::Canceled){ break; } } tokio::time::sleep(std::time::Duration::from_millis(60)).await; waited+=60; }
  let snap = registry.snapshot(&id).unwrap(); assert_eq!(snap.state, TaskState::Completed, "clone should succeed when capability on");
  handle.await.unwrap();
  let mut attempts=0; let mut seen_fallback=false; while attempts<10 { let evs=peek_captured_events(); for (topic,json) in &evs { if topic=="task://error" && json.contains("partial_filter_fallback") { seen_fallback=true; break; } } if seen_fallback { break; } tokio::time::sleep(std::time::Duration::from_millis(40)).await; attempts+=1; }
  let _ = drain_captured_events();
  assert!(!seen_fallback, "should NOT emit fallback when capability enabled (clone depth={:?} filter={})", depth, filter);
  std::env::remove_var("FWC_PARTIAL_FILTER_CAPABLE");
}

#[tokio::test]
async fn clone_filter_only_capable_no_fallback() { run_case(None, "blob:none").await; }

#[tokio::test]
async fn clone_depth_filter_capable_no_fallback() { run_case(Some(1), "tree:0").await; }
