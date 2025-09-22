use fireworks_collaboration_lib::events::emitter::AppHandle;
use fireworks_collaboration_lib::core::tasks::registry::TaskRegistry;
use fireworks_collaboration_lib::core::tasks::model::{TaskKind, TaskState};
use fireworks_collaboration_lib::events::structured::{set_global_event_bus, MemoryEventBus};
use fireworks_collaboration_lib::tests_support::event_assert::{assert_applied_code, assert_no_applied_code};
use std::sync::Arc;

// Combined test to avoid parallel interference:
// 1) Changed retry override should emit event once.
// 2) Unchanged retry override should not emit event.
#[test]
fn git_clone_retry_override_event_and_no_event() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        // Prepare a local bare repo to clone
        let tmp_src = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(tmp_src.path()).unwrap();
        // create one commit
        let file = tmp_src.path().join("a.txt");
        std::fs::write(&file, "hello").unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(std::path::Path::new("a.txt")).unwrap();
        index.write().unwrap();
        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        let sig = repo.signature().unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "c1", &tree, &[]).unwrap();

        let dest = tempfile::tempdir().unwrap();
    let reg = Arc::new(TaskRegistry::new());
    // 在启动任务前设置结构化事件总线以捕获策略事件
    let _ = set_global_event_bus(std::sync::Arc::new(MemoryEventBus::new()));
        // Override retry values different from defaults (default max=6 baseMs=300 factor=1.5 jitter=true)
        let override_json = serde_json::json!({"retry": {"max": 3, "baseMs": 500, "factor": 2.0, "jitter": false}});
        let (id, token) = reg.create(TaskKind::GitClone { repo: tmp_src.path().to_string_lossy().to_string(), dest: dest.path().to_string_lossy().to_string(), depth: None, filter: None, strategy_override: Some(override_json.clone()) });
        let app = AppHandle;
    let handle = reg.clone().spawn_git_clone_task_with_opts(Some(app), id, token, tmp_src.path().to_string_lossy().to_string(), dest.path().to_string_lossy().to_string(), None, None, Some(override_json));
        for _ in 0..100 { if let Some(s)=reg.snapshot(&id) { if matches!(s.state, TaskState::Completed | TaskState::Failed) { break; } } tokio::time::sleep(std::time::Duration::from_millis(50)).await; }
        handle.await.unwrap();
    assert_applied_code(&id.to_string(), "retry_strategy_override_applied");

        // Drain before second scenario
    // summary 事件无条件发送，不需清空 legacy；重新设置新 bus 以隔离
    let _ = set_global_event_bus(std::sync::Arc::new(MemoryEventBus::new()));

        // Scenario 2: unchanged override -> no event
        let dest2 = tempfile::tempdir().unwrap();
        let (id2, token2) = reg.create(TaskKind::GitClone { repo: tmp_src.path().to_string_lossy().to_string(), dest: dest2.path().to_string_lossy().to_string(), depth: None, filter: None, strategy_override: Some(serde_json::json!({"retry": {"max": 6, "baseMs": 300, "factor": 1.5, "jitter": true}})) });
        let handle2 = reg.clone().spawn_git_clone_task_with_opts(Some(AppHandle), id2, token2, tmp_src.path().to_string_lossy().to_string(), dest2.path().to_string_lossy().to_string(), None, None, Some(serde_json::json!({"retry": {"max": 6, "baseMs": 300, "factor": 1.5, "jitter": true}})));
        for _ in 0..100 { if let Some(s)=reg.snapshot(&id2) { if matches!(s.state, TaskState::Completed | TaskState::Failed) { break; } } tokio::time::sleep(std::time::Duration::from_millis(50)).await; }
        handle2.await.unwrap();
        assert_no_applied_code(&id2.to_string(), "retry_strategy_override_applied");
    });
}
