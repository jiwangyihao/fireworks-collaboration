use std::sync::Arc;
use fireworks_collaboration_lib::events::emitter::AppHandle;
use fireworks_collaboration_lib::core::tasks::registry::TaskRegistry;
use fireworks_collaboration_lib::core::tasks::model::TaskKind;
use fireworks_collaboration_lib::events::structured::{set_global_event_bus, MemoryEventBus};
use fireworks_collaboration_lib::tests_support::event_assert::assert_applied_code;
use fireworks_collaboration_lib::tests_support::repo::build_repo;
use fireworks_collaboration_lib::tests_support::wait::wait_task_terminal;

#[test]
fn git_clone_http_override_event_once_per_task() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        // build source repo with one commit
    let (_tmp, src_path_buf) = build_repo(&[("a.txt","hi")]);

        let dest = tempfile::tempdir().unwrap();
    let reg = Arc::new(TaskRegistry::new());
    let _ = set_global_event_bus(std::sync::Arc::new(MemoryEventBus::new()));
        let override_json = serde_json::json!({"http": {"followRedirects": false, "maxRedirects": 2}});
    let (id, token) = reg.create(TaskKind::GitClone { repo: src_path_buf.to_string_lossy().to_string(), dest: dest.path().to_string_lossy().to_string(), depth: None, filter: None, strategy_override: Some(override_json.clone()) });
        let app = AppHandle;
    let handle = reg.clone().spawn_git_clone_task_with_opts(Some(app), id, token, src_path_buf.to_string_lossy().to_string(), dest.path().to_string_lossy().to_string(), None, None, Some(override_json));
    let _ = wait_task_terminal(&reg, &id, 25, 120).await;
        handle.await.unwrap();

    assert_applied_code(&id.to_string(), "http_strategy_override_applied");
    });
}
