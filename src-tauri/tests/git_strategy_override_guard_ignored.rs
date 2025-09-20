use std::fs;
use std::sync::Arc;
use fireworks_collaboration_lib::tasks::{TaskRegistry, TaskKind};
use fireworks_collaboration_lib::events::emitter::{AppHandle, peek_captured_events};
use fireworks_collaboration_lib::tasks::model::TaskState;

// 验证含未知字段的 strategyOverride 产生一次 strategy_override_ignored_fields 事件
#[test]
fn clone_override_with_ignored_fields_emits_event() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let tmp_src = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(tmp_src.path()).unwrap();
        // commit
        let fp = tmp_src.path().join("a.txt"); fs::write(&fp, "hello").unwrap();
        let mut index = repo.index().unwrap(); index.add_path(std::path::Path::new("a.txt")).unwrap(); index.write().unwrap();
        let tree_id = index.write_tree().unwrap(); let tree = repo.find_tree(tree_id).unwrap();
        let sig = repo.signature().unwrap(); repo.commit(Some("HEAD"), &sig, &sig, "c1", &tree, &[]).unwrap();

        let dest = tempfile::tempdir().unwrap();
        let reg = Arc::new(TaskRegistry::new());
        let override_json = serde_json::json!({
            "http": {"followRedirects": true, "AAA": 1},
            "tls": {"insecureSkipVerify": false, "BBB": true},
            "retry": {"max": 3, "factor": 1.2, "CCC": 10},
            "extraTop": {"foo": 1}
        });
        let (id, token) = reg.create(TaskKind::GitClone { repo: tmp_src.path().to_string_lossy().to_string(), dest: dest.path().to_string_lossy().to_string(), depth: None, filter: None, strategy_override: Some(override_json.clone()) });
        let app = AppHandle;
        let handle = reg.clone().spawn_git_clone_task_with_opts(Some(app), id, token, tmp_src.path().to_string_lossy().to_string(), dest.path().to_string_lossy().to_string(), None, None, Some(override_json));
        for _ in 0..120 { if let Some(s) = reg.snapshot(&id) { if matches!(s.state, TaskState::Completed | TaskState::Failed) { break; } } tokio::time::sleep(std::time::Duration::from_millis(50)).await; }
        handle.await.unwrap();
        let events = peek_captured_events();
        let mut ignored_evt = 0; let mut http_evt = 0; // ensure existing override events still possible
        for (topic,payload) in events { if topic=="task://error" { if payload.contains("strategy_override_ignored_fields") && payload.contains(&id.to_string()) { ignored_evt+=1; } if payload.contains("http_strategy_override_applied") { http_evt+=1; } } }
        assert_eq!(ignored_evt, 1, "expected exactly one ignored fields event");
        assert!(http_evt <=1, "http override event optional but at most once");
    });
}

// 验证无未知字段不产生 ignored 事件
#[test]
fn clone_override_without_ignored_fields_no_event() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let tmp_src = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(tmp_src.path()).unwrap();
        // commit
        let fp = tmp_src.path().join("b.txt"); fs::write(&fp, "hello2").unwrap();
        let mut index = repo.index().unwrap(); index.add_path(std::path::Path::new("b.txt")).unwrap(); index.write().unwrap();
        let tree_id = index.write_tree().unwrap(); let tree = repo.find_tree(tree_id).unwrap();
        let sig = repo.signature().unwrap(); repo.commit(Some("HEAD"), &sig, &sig, "c2", &tree, &[]).unwrap();

        let dest = tempfile::tempdir().unwrap();
        let reg = Arc::new(TaskRegistry::new());
        let override_json = serde_json::json!({"http": {"followRedirects": false}});
        let (id, token) = reg.create(TaskKind::GitClone { repo: tmp_src.path().to_string_lossy().to_string(), dest: dest.path().to_string_lossy().to_string(), depth: None, filter: None, strategy_override: Some(override_json.clone()) });
        let app = AppHandle;
        let handle = reg.clone().spawn_git_clone_task_with_opts(Some(app), id, token, tmp_src.path().to_string_lossy().to_string(), dest.path().to_string_lossy().to_string(), None, None, Some(override_json));
        for _ in 0..120 { if let Some(s) = reg.snapshot(&id) { if matches!(s.state, TaskState::Completed | TaskState::Failed) { break; } } tokio::time::sleep(std::time::Duration::from_millis(40)).await; }
        handle.await.unwrap();
        let events = peek_captured_events();
        let mut ignored_found = false; for (topic,payload) in events { if topic=="task://error" && payload.contains("strategy_override_ignored_fields") && payload.contains(&id.to_string()) { ignored_found=true; break; } }
        assert!(!ignored_found, "should NOT emit ignored fields event for clean override");
    });
}
