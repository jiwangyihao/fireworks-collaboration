#![cfg(not(feature = "tauri-app"))]
//! 当环境变量 FWC_PARTIAL_FILTER_CAPABLE=1 时，clone 请求 filter 不应发送 fallback 事件
use std::path::PathBuf; use std::process::Command; use std::sync::Arc;
use fireworks_collaboration_lib::core::tasks::registry::TaskRegistry;
use fireworks_collaboration_lib::core::tasks::model::{TaskKind, TaskState};
use fireworks_collaboration_lib::events::emitter::AppHandle;
use fireworks_collaboration_lib::events::structured::{set_global_event_bus, MemoryEventBus};
use fireworks_collaboration_lib::tests_support::event_assert::{assert_partial_capability, assert_no_partial_fallback, assert_no_partial_unsupported};

fn unique_dir(label:&str)->PathBuf { std::env::temp_dir().join(format!("fwc-partial-clone-capable-{}-{}", label, uuid::Uuid::new_v4())) }
fn build_origin()->PathBuf { let dir = unique_dir("origin"); std::fs::create_dir_all(&dir).unwrap(); let run=|a:&[&str]|{assert!(Command::new("git").current_dir(&dir).args(a).status().unwrap().success());}; run(&["init","--quiet"]); run(&["config","user.email","a@example.com"]); run(&["config","user.name","A"]); for i in 1..=3 { std::fs::write(dir.join(format!("f{}.txt",i)), i.to_string()).unwrap(); run(&["add","."]); run(&["commit","-m", &format!("c{}", i)]); } dir }

async fn run_case(depth: Option<u32>, filter: &str) {
  // allow either env name; set both to be explicit
  std::env::set_var("FWC_PARTIAL_FILTER_SUPPORTED","1");
  std::env::set_var("FWC_PARTIAL_FILTER_CAPABLE","1");
  let origin = build_origin();
  let dest = unique_dir("clone");
  let registry = Arc::new(TaskRegistry::new());
  let (id, token) = registry.create(TaskKind::GitClone { repo: origin.to_string_lossy().to_string(), dest: dest.to_string_lossy().to_string(), depth, filter: Some(filter.into()), strategy_override: None });
  let app = AppHandle;
  let _ = set_global_event_bus(std::sync::Arc::new(MemoryEventBus::new()));
  let depth_json = depth.map(|d| serde_json::json!(d));
  let handle = registry.spawn_git_clone_task_with_opts(Some(app), id, token, origin.to_string_lossy().to_string(), dest.to_string_lossy().to_string(), depth_json, Some(filter.into()), None);
  // 使用统一等待 helper 替换手写轮询 (≈6s 上限)
  let _ = fireworks_collaboration_lib::tests_support::wait::wait_task_terminal(&registry,&id,60,100).await;
  let snap = registry.snapshot(&id).unwrap(); assert_eq!(snap.state, TaskState::Completed, "clone should succeed when capability on");
  handle.await.unwrap();
  // 断言：有 capability(supported=true)，无 fallback/unsupported
  assert_partial_capability(&id.to_string(), true);
  assert_no_partial_fallback(&id.to_string());
  assert_no_partial_unsupported(&id.to_string());
  std::env::remove_var("FWC_PARTIAL_FILTER_SUPPORTED");
  std::env::remove_var("FWC_PARTIAL_FILTER_CAPABLE");
}

#[tokio::test]
async fn clone_filter_only_capable_no_fallback() { run_case(None, "blob:none").await; }

#[tokio::test]
async fn clone_depth_filter_capable_no_fallback() { run_case(Some(1), "tree:0").await; }
