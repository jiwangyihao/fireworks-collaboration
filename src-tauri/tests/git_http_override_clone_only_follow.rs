use std::sync::Arc;
use fireworks_collaboration_lib::events::emitter::{peek_captured_events, AppHandle};
use fireworks_collaboration_lib::tasks::{TaskRegistry, TaskKind};
use fireworks_collaboration_lib::tasks::model::TaskState;

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
        let override_json = serde_json::json!({"http": {"followRedirects": false}}); // only follow changes (default true)
    // 创建占位任务 ID，实际策略在 spawn 参数中提供，避免重复 applied 事件
    let (id, token) = reg.create(TaskKind::Sleep { ms: 5 });
    let app = AppHandle; reg.clone().spawn_git_clone_task_with_opts(Some(app), id, token, src.path().to_string_lossy().to_string(), dest.path().to_string_lossy().to_string(), None, None, Some(override_json));

        for _ in 0..140 { if let Some(s)=reg.snapshot(&id) { if matches!(s.state, TaskState::Completed|TaskState::Failed) { break; } } tokio::time::sleep(std::time::Duration::from_millis(25)).await; }

        let events = peek_captured_events();
        let mut count=0; let mut found=false;
        for (topic,payload) in events {
            if topic=="task://error" && payload.contains("\"code\":\"http_strategy_override_applied\"") && payload.contains(&id.to_string()) { found=true; count+=1; }
        }
        assert!(found, "expected override event for only follow change");
        assert_eq!(count, 1, "should emit only once");
    });
}
