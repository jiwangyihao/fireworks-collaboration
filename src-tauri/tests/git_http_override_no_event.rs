use fireworks_collaboration_lib::events::emitter::AppHandle;
use fireworks_collaboration_lib::tasks::{TaskRegistry, TaskKind};
use std::sync::Arc;
// TaskState 通过等待 helper 间接使用
use fireworks_collaboration_lib::tests_support::wait::wait_task_terminal;
use fireworks_collaboration_lib::events::structured::{set_global_event_bus, MemoryEventBus};
use fireworks_collaboration_lib::tests_support::event_assert::assert_no_applied_code;
use fireworks_collaboration_lib::tests_support::repo::build_repo;

#[test]
fn git_clone_http_override_no_event_when_same_values() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
    let _ = set_global_event_bus(std::sync::Arc::new(MemoryEventBus::new()));
    // source repo via builder
    let (_tmp, src_path) = build_repo(&[("a.txt","hi")]);
    let dest = tempfile::tempdir().unwrap();

    let reg = Arc::new(TaskRegistry::new());
    // global defaults follow=true max=5 ; override with same values -> no event
    let override_json = serde_json::json!({"http":{"followRedirects": true, "maxRedirects":5}});
    let (id, token) = reg.create(TaskKind::GitClone { repo: src_path.to_string_lossy().to_string(), dest: dest.path().to_string_lossy().to_string(), depth: None, filter: None, strategy_override: Some(override_json.clone()) });
    let app = AppHandle;
    let handle = reg.clone().spawn_git_clone_task_with_opts(Some(app), id, token, src_path.to_string_lossy().to_string(), dest.path().to_string_lossy().to_string(), None, None, Some(override_json));

    let _ = wait_task_terminal(&reg, &id, 40, 100).await;
    handle.await.unwrap();

    assert_no_applied_code(&id.to_string(), "http_strategy_override_applied");
    });
}
