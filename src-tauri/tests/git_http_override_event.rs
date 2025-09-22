use fireworks_collaboration_lib::events::emitter::AppHandle;
use fireworks_collaboration_lib::tasks::{TaskRegistry, TaskKind};
use std::sync::Arc;
use fireworks_collaboration_lib::tests_support::wait::wait_task_terminal;
use fireworks_collaboration_lib::events::structured::{set_global_event_bus, MemoryEventBus};
use fireworks_collaboration_lib::tests_support::event_assert::assert_applied_code;
use fireworks_collaboration_lib::tests_support::repo::build_repo;

#[test]
fn git_clone_http_override_event() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
    let _ = set_global_event_bus(std::sync::Arc::new(MemoryEventBus::new()));
    // 使用通用 repo builder 创建包含一个提交的源仓库
    let (_src_tmp, src_path) = build_repo(&[("a.txt","hello")]);
    let dest = tempfile::tempdir().unwrap();

    // Build registry
    let reg = Arc::new(TaskRegistry::new());
    let override_json = serde_json::json!({"http": {"followRedirects": false, "maxRedirects": 2}});
    let (id, token) = reg.create(TaskKind::GitClone { repo: src_path.to_string_lossy().to_string(), dest: dest.path().to_string_lossy().to_string(), depth: None, filter: None, strategy_override: Some(override_json.clone()) });
    let app = AppHandle; // non-tauri 占位句柄
    let handle = reg.clone().spawn_git_clone_task_with_opts(Some(app), id, token, src_path.to_string_lossy().to_string(), dest.path().to_string_lossy().to_string(), None, None, Some(override_json));

    // Wait for completion (poll snapshot)
    let _ = wait_task_terminal(&reg, &id, 50, 100).await;
    handle.await.unwrap();

    assert_applied_code(&id.to_string(), "http_strategy_override_applied");
    });
}
