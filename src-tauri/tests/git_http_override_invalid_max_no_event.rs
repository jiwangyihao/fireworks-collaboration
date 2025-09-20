use std::sync::Arc;
use fireworks_collaboration_lib::events::emitter::{peek_captured_events, AppHandle};
use fireworks_collaboration_lib::tasks::{TaskRegistry, TaskKind};
use fireworks_collaboration_lib::tasks::model::TaskState;

// When maxRedirects > 20 parser returns Protocol error; ensure no override-applied event appears.
#[test]
fn git_clone_http_override_invalid_max_no_event() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let src = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(src.path()).unwrap();
        std::fs::write(src.path().join("a.txt"), "hello").unwrap();
        let mut index = repo.index().unwrap(); index.add_path(std::path::Path::new("a.txt")).unwrap(); index.write().unwrap();
        let tree = repo.find_tree(index.write_tree().unwrap()).unwrap(); let sig = repo.signature().unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "c1", &tree, &[]).unwrap();
        let dest = tempfile::tempdir().unwrap();

        let reg = Arc::new(TaskRegistry::new());
        let bad_override = serde_json::json!({"http": {"maxRedirects": 99}}); // invalid >20
        let (id, token) = reg.create(TaskKind::GitClone { repo: src.path().to_string_lossy().to_string(), dest: dest.path().to_string_lossy().to_string(), depth: None, filter: None, strategy_override: Some(bad_override.clone()) });
        let app = AppHandle; reg.clone().spawn_git_clone_task_with_opts(Some(app), id, token, src.path().to_string_lossy().to_string(), dest.path().to_string_lossy().to_string(), None, None, Some(bad_override));
        for _ in 0..140 { if let Some(s)=reg.snapshot(&id) { if matches!(s.state, TaskState::Failed|TaskState::Completed) { break; } } tokio::time::sleep(std::time::Duration::from_millis(25)).await; }

        let snap = reg.snapshot(&id).expect("snapshot");
        assert!(matches!(snap.state, TaskState::Failed), "expected protocol failure state");

        let events = peek_captured_events();
        for (topic,payload) in events { if topic=="task://error" && payload.contains("http_strategy_override_applied") && payload.contains(&id.to_string()) { panic!("should not produce override-applied event on invalid maxRedirects"); } }
    });
}
