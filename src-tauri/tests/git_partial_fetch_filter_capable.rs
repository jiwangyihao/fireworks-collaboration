#![cfg(not(feature = "tauri-app"))]
//! 当环境变量 FWC_PARTIAL_FILTER_CAPABLE=1 时，fetch 请求 filter 不应发送 fallback 事件
use std::path::PathBuf; use std::process::Command; use std::sync::Arc;
use fireworks_collaboration_lib::core::tasks::registry::TaskRegistry;
use fireworks_collaboration_lib::core::tasks::model::{TaskKind, TaskState};
use fireworks_collaboration_lib::events::emitter::{AppHandle, peek_captured_events, drain_captured_events};

fn unique_dir(label:&str)->PathBuf { std::env::temp_dir().join(format!("fwc-partial-fetch-capable-{}-{}", label, uuid::Uuid::new_v4())) }
fn build_origin_and_work()->(PathBuf, PathBuf) {
  let origin = unique_dir("origin");
  std::fs::create_dir_all(&origin).unwrap();
  let run=|a:&[&str]|{assert!(Command::new("git").current_dir(&origin).args(a).status().unwrap().success());};
  run(&["init","--quiet"]); run(&["config","user.email","a@example.com"]); run(&["config","user.name","A"]);
  for i in 1..=3 { std::fs::write(origin.join(format!("f{}.txt",i)), i.to_string()).unwrap(); run(&["add","."]); run(&["commit","-m", &format!("c{}", i)]); }
  let work = unique_dir("work");
  assert!(Command::new("git").args(["clone", origin.to_string_lossy().as_ref(), work.to_string_lossy().as_ref()]).status().unwrap().success());
  std::fs::write(origin.join("new.txt"),"x").unwrap(); run(&["add","new.txt"]); run(&["commit","-m","later"]);
  (work, origin)
}

async fn run_case(depth: Option<u32>, filter: &str) {
  std::env::set_var("FWC_PARTIAL_FILTER_CAPABLE","1");
  let (work, _origin) = build_origin_and_work();
  let registry = Arc::new(TaskRegistry::new());
  let (id, token) = registry.create(TaskKind::GitFetch { repo: "origin".into(), dest: work.to_string_lossy().to_string(), depth, filter: Some(filter.into()), strategy_override: None });
  let app = AppHandle; let depth_json = depth.map(|d| serde_json::json!(d));
  let handle = registry.spawn_git_fetch_task_with_opts(Some(app), id, token, "origin".into(), work.to_string_lossy().to_string(), None, depth_json, Some(filter.into()), None);
  let mut waited=0; while waited<6000 { if let Some(s)=registry.snapshot(&id) { if matches!(s.state, TaskState::Completed|TaskState::Failed|TaskState::Canceled){ break; } } tokio::time::sleep(std::time::Duration::from_millis(60)).await; waited+=60; }
  let snap = registry.snapshot(&id).unwrap(); assert_eq!(snap.state, TaskState::Completed, "fetch should succeed when capability on");
  handle.await.unwrap();
  let mut attempts=0; let mut seen_fallback=false; while attempts<10 { let evs=peek_captured_events(); for (topic,json) in &evs { if topic=="task://error" && json.contains("partial_filter_fallback") { seen_fallback=true; break; } } if seen_fallback { break; } tokio::time::sleep(std::time::Duration::from_millis(40)).await; attempts+=1; }
  let _ = drain_captured_events();
  assert!(!seen_fallback, "should NOT emit fallback when capability enabled (fetch depth={:?} filter={})", depth, filter);
  std::env::remove_var("FWC_PARTIAL_FILTER_CAPABLE");
}

#[tokio::test]
async fn fetch_filter_only_capable_no_fallback() { run_case(None, "blob:none").await; }

#[tokio::test]
async fn fetch_depth_filter_capable_no_fallback() { run_case(Some(2), "tree:0").await; }
