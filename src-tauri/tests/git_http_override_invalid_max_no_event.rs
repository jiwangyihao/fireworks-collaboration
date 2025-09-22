use std::sync::Arc;
use fireworks_collaboration_lib::events::emitter::AppHandle;
use fireworks_collaboration_lib::tasks::{TaskRegistry, TaskKind};
use fireworks_collaboration_lib::tasks::model::TaskState;
use fireworks_collaboration_lib::tests_support::wait::wait_task_terminal;
use fireworks_collaboration_lib::events::structured::{set_global_event_bus, MemoryEventBus};
use fireworks_collaboration_lib::tests_support::event_assert::assert_no_applied_code;
use fireworks_collaboration_lib::tests_support::repo::build_repo;

// When maxRedirects > 20 parser returns Protocol error; ensure no override-applied event appears.
#[test]
fn git_clone_http_override_invalid_max_no_event() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let _ = set_global_event_bus(std::sync::Arc::new(MemoryEventBus::new()));
    let (_tmp, src_path) = build_repo(&[("a.txt","hello")]);
    let dest = tempfile::tempdir().unwrap();

        let reg = Arc::new(TaskRegistry::new());
        let bad_override = serde_json::json!({"http": {"maxRedirects": 99}}); // invalid >20
    let (id, token) = reg.create(TaskKind::GitClone { repo: src_path.to_string_lossy().to_string(), dest: dest.path().to_string_lossy().to_string(), depth: None, filter: None, strategy_override: Some(bad_override.clone()) });
    let app = AppHandle; reg.clone().spawn_git_clone_task_with_opts(Some(app), id, token, src_path.to_string_lossy().to_string(), dest.path().to_string_lossy().to_string(), None, None, Some(bad_override));
    let _ = wait_task_terminal(&reg, &id, 25, 140).await;

        let snap = reg.snapshot(&id).expect("snapshot");
        assert!(matches!(snap.state, TaskState::Failed), "expected protocol failure state");

        assert_no_applied_code(&id.to_string(), "http_strategy_override_applied");
    });
}
