use std::fs;
use std::sync::Arc;
use fireworks_collaboration_lib::tasks::{TaskRegistry, TaskKind};
use fireworks_collaboration_lib::events::emitter::AppHandle;
// TaskState 仅在等待 helper 内部使用，测试文件无需直接引用
use fireworks_collaboration_lib::events::structured::{set_global_event_bus, MemoryEventBus, Event, get_global_memory_bus};
use fireworks_collaboration_lib::tests_support::event_assert::{assert_ignored_fields};
use std::sync::Mutex;

static TEST_SERIAL_MUTEX: Mutex<()> = Mutex::new(());

// 验证含未知字段的 strategyOverride 产生一次结构化 IgnoredFields 事件（legacy TaskErrorEvent 已移除）
#[test]
fn clone_override_with_ignored_fields_emits_event() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let _guard = TEST_SERIAL_MUTEX.lock().unwrap();
    let _ = set_global_event_bus(std::sync::Arc::new(MemoryEventBus::new()));
    if let Some(bus) = get_global_memory_bus() { let _ = bus.take_all(); }
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
    let _ = fireworks_collaboration_lib::tests_support::wait::wait_task_terminal(&reg, &id, 50, 120).await;
        handle.await.unwrap();
    // 结构化：断言新的 StrategyEvent::IgnoredFields
    assert_ignored_fields(&id.to_string(), "GitClone", &["extraTop"], &["http.AAA","tls.BBB","retry.CCC"]);
    });
}

// 验证无未知字段不产生 ignored 事件
#[test]
fn clone_override_without_ignored_fields_no_event() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let _guard = TEST_SERIAL_MUTEX.lock().unwrap();
    let _ = set_global_event_bus(std::sync::Arc::new(MemoryEventBus::new()));
    if let Some(bus) = get_global_memory_bus() { let _ = bus.take_all(); }
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
    let _ = fireworks_collaboration_lib::tests_support::wait::wait_task_terminal(&reg, &id, 40, 120).await;
        handle.await.unwrap();
    // 不应出现 IgnoredFields 结构化事件（clean override）
    let bus = fireworks_collaboration_lib::events::structured::get_global_memory_bus().expect("bus");
    let snapshot = bus.snapshot();
    // 使用 helper 验证没有 IgnoredFields 结构化事件
    let has_ignored = snapshot.iter().any(|e| matches!(e, Event::Strategy(fireworks_collaboration_lib::events::structured::StrategyEvent::IgnoredFields{id,kind,..}) if id==&id.to_string() && kind=="GitClone"));
    assert!(!has_ignored, "unexpected structured IgnoredFields event for clean override");
    });
}
