use fireworks_collaboration_lib::events::emitter::{peek_captured_events, AppHandle};
use fireworks_collaboration_lib::tasks::{TaskRegistry, TaskKind};
use fireworks_collaboration_lib::tasks::model::TaskState;
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
        // Override retry values different from defaults (default max=6 baseMs=300 factor=1.5 jitter=true)
        let override_json = serde_json::json!({"retry": {"max": 3, "baseMs": 500, "factor": 2.0, "jitter": false}});
        let (id, token) = reg.create(TaskKind::GitClone { repo: tmp_src.path().to_string_lossy().to_string(), dest: dest.path().to_string_lossy().to_string(), depth: None, filter: None, strategy_override: Some(override_json.clone()) });
        let app = AppHandle;
        let handle = reg.clone().spawn_git_clone_task_with_opts(Some(app), id, token, tmp_src.path().to_string_lossy().to_string(), dest.path().to_string_lossy().to_string(), None, None, Some(override_json));
        for _ in 0..100 { if let Some(s)=reg.snapshot(&id) { if matches!(s.state, TaskState::Completed | TaskState::Failed) { break; } } tokio::time::sleep(std::time::Duration::from_millis(50)).await; }
        handle.await.unwrap();
        let events_first = peek_captured_events();
        let mut found=false; let mut count=0;
        for (topic,payload) in &events_first { if topic=="task://error" && payload.contains("retry_strategy_override_applied") && payload.contains(&id.to_string()) { found=true; count+=1; } }
        assert!(found, "expected retry_strategy_override_applied event");
        assert_eq!(count, 1, "should emit exactly once for changed override");

        // Drain before second scenario
        let _ = fireworks_collaboration_lib::events::emitter::drain_captured_events();

        // Scenario 2: unchanged override -> no event
        let dest2 = tempfile::tempdir().unwrap();
        let (id2, token2) = reg.create(TaskKind::GitClone { repo: tmp_src.path().to_string_lossy().to_string(), dest: dest2.path().to_string_lossy().to_string(), depth: None, filter: None, strategy_override: Some(serde_json::json!({"retry": {"max": 6, "baseMs": 300, "factor": 1.5, "jitter": true}})) });
        let handle2 = reg.clone().spawn_git_clone_task_with_opts(Some(AppHandle), id2, token2, tmp_src.path().to_string_lossy().to_string(), dest2.path().to_string_lossy().to_string(), None, None, Some(serde_json::json!({"retry": {"max": 6, "baseMs": 300, "factor": 1.5, "jitter": true}})));
        for _ in 0..100 { if let Some(s)=reg.snapshot(&id2) { if matches!(s.state, TaskState::Completed | TaskState::Failed) { break; } } tokio::time::sleep(std::time::Duration::from_millis(50)).await; }
        handle2.await.unwrap();
        let events_second = peek_captured_events();
        for (topic,payload) in events_second { if topic=="task://error" && payload.contains("retry_strategy_override_applied") && payload.contains(&id2.to_string()) { panic!("should NOT emit retry override applied event when values unchanged"); } }
    });
}
