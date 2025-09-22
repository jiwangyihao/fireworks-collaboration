use std::sync::Arc;
use fireworks_collaboration_lib::events::emitter::AppHandle;
use fireworks_collaboration_lib::core::tasks::registry::TaskRegistry;
use fireworks_collaboration_lib::core::tasks::model::{TaskKind, TaskState};
use fireworks_collaboration_lib::events::structured::{set_global_event_bus, MemoryEventBus};
use fireworks_collaboration_lib::tests_support::event_assert::{assert_applied_code, assert_no_applied_code};

#[test]
fn git_clone_http_override_only_follow_triggers_event() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let src = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(src.path()).unwrap();
        std::fs::write(src.path().join("a.txt"), "hello").unwrap();
        let mut index = repo.index().unwrap(); index.add_path(std::path::Path::new("a.txt")).unwrap(); index.write().unwrap();
        let tree = repo.find_tree(index.write_tree().unwrap()).unwrap();
        let sig = repo.signature().unwrap(); repo.commit(Some("HEAD"), &sig,&sig,"c1", &tree, &[]).unwrap();
        let dest = tempfile::tempdir().unwrap();

    let reg = Arc::new(TaskRegistry::new());
    // 在任务启动前设置结构化内存事件总线
    let _ = set_global_event_bus(std::sync::Arc::new(MemoryEventBus::new()));
        let override_json = serde_json::json!({"http": {"followRedirects": false}}); // only follow changes (default true)
    // 创建占位任务 ID，实际策略在 spawn 参数中提供，避免重复 applied 事件
    let (id, token) = reg.create(TaskKind::Sleep { ms: 5 });
    let app = AppHandle; reg.clone().spawn_git_clone_task_with_opts(Some(app), id, token, src.path().to_string_lossy().to_string(), dest.path().to_string_lossy().to_string(), None, None, Some(override_json));

        for _ in 0..140 { if let Some(s)=reg.snapshot(&id) { if matches!(s.state, TaskState::Completed|TaskState::Failed) { break; } } tokio::time::sleep(std::time::Duration::from_millis(25)).await; }

    // 断言 summary 中 http applied code 出现一次（且无 retry/tls）
        assert_applied_code(&id.to_string(), "http_strategy_override_applied");
        assert_no_applied_code(&id.to_string(), "retry_strategy_override_applied");
        assert_no_applied_code(&id.to_string(), "tls_strategy_override_applied");
    });
}
