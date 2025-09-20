use std::sync::Arc;
use fireworks_collaboration_lib::events::emitter::{peek_captured_events, AppHandle};
use fireworks_collaboration_lib::tasks::{TaskRegistry, TaskKind};
use fireworks_collaboration_lib::tasks::model::TaskState;

#[test]
fn git_clone_http_override_event_once_per_task() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        // build source repo with one commit
        let src = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(src.path()).unwrap();
        std::fs::write(src.path().join("a.txt"), "hi").unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(std::path::Path::new("a.txt")).unwrap();
        index.write().unwrap();
        let tree = repo.find_tree(index.write_tree().unwrap()).unwrap();
        let sig = repo.signature().unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "c1", &tree, &[]).unwrap();

        let dest = tempfile::tempdir().unwrap();
        let reg = Arc::new(TaskRegistry::new());
        let override_json = serde_json::json!({"http": {"followRedirects": false, "maxRedirects": 2}});
        let (id, token) = reg.create(TaskKind::GitClone { repo: src.path().to_string_lossy().to_string(), dest: dest.path().to_string_lossy().to_string(), depth: None, filter: None, strategy_override: Some(override_json.clone()) });
        let app = AppHandle;
        let handle = reg.clone().spawn_git_clone_task_with_opts(Some(app), id, token, src.path().to_string_lossy().to_string(), dest.path().to_string_lossy().to_string(), None, None, Some(override_json));

        for _ in 0..120 { if let Some(s)=reg.snapshot(&id) { if matches!(s.state, TaskState::Completed | TaskState::Failed) { break; } } tokio::time::sleep(std::time::Duration::from_millis(25)).await; }
        handle.await.unwrap();

        let events = peek_captured_events();
        let mut count = 0u32;
        for (topic, payload) in events { if topic=="task://error" && payload.contains("\"code\":\"http_strategy_override_applied\"") && payload.contains(&id.to_string()) { count += 1; } }
        assert_eq!(count, 1, "expected exactly one override event for the task; got {}", count);
    });
}
