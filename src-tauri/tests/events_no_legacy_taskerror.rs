//! 确认所有 legacy 策略类 TaskErrorEvent 已被移除，不再通过 Failed 事件通道出现。
use fireworks_collaboration_lib::events::structured::{set_global_event_bus, MemoryEventBus, get_global_memory_bus, Event, TaskEvent};
use fireworks_collaboration_lib::tasks::{TaskRegistry, TaskKind};
use fireworks_collaboration_lib::events::emitter::AppHandle;
use std::sync::Arc;

#[test]
fn no_legacy_strategy_taskerror_events() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
    let _ = set_global_event_bus(Arc::new(MemoryEventBus::new()));
    // 构造包含 override/partial/filter 情况的多任务触发路径（确保会生成结构化 summary/ignored/partial/adaptive 等）
    let reg = Arc::new(TaskRegistry::new());
    let tmp_repo = tempfile::tempdir().unwrap();
    let repo = git2::Repository::init(tmp_repo.path()).unwrap();
    // 提交一个文件
    std::fs::write(tmp_repo.path().join("f.txt"), "hi").unwrap();
    let mut index = repo.index().unwrap(); index.add_path(std::path::Path::new("f.txt")).unwrap(); index.write().unwrap();
    let tree_id = index.write_tree().unwrap(); let tree = repo.find_tree(tree_id).unwrap();
    let sig = repo.signature().unwrap(); repo.commit(Some("HEAD"), &sig, &sig, "c1", &tree, &[]).unwrap();

    let dest = tempfile::tempdir().unwrap();
    // override 含未知字段 + http/tls/retry 修改
    let override_json = serde_json::json!({
        "http": {"followRedirects": false, "maxRedirects": 5, "EXTRA": 1},
        "tls": {"insecureSkipVerify": true, "skipSanWhitelist": true},
        "retry": {"max": 2, "factor": 1.1, "EXTRA2": true},
        "unknownTop": {"x":1}
    });
    let (id, token) = reg.create(TaskKind::GitClone { repo: tmp_repo.path().to_string_lossy().to_string(), dest: dest.path().to_string_lossy().to_string(), depth: Some(1), filter: Some("blob:none".to_string()), strategy_override: Some(override_json) });
    let app = AppHandle;
    let handle = reg.clone().spawn_git_clone_task_with_opts(Some(app), id, token, tmp_repo.path().to_string_lossy().to_string(), dest.path().to_string_lossy().to_string(), Some(serde_json::json!(1)), Some("blob:none".to_string()), None);
    // 等待任务完成
    for _ in 0..120 { if let Some(s) = reg.snapshot(&id) { if matches!(s.state, fireworks_collaboration_lib::tasks::model::TaskState::Completed | fireworks_collaboration_lib::tasks::model::TaskState::Failed) { break; } } std::thread::sleep(std::time::Duration::from_millis(40)); }
    handle.await.unwrap();

    let bus = get_global_memory_bus().expect("bus");
    let events = bus.snapshot();
    let legacy_codes = [
        "http_strategy_override_applied",
        "tls_strategy_override_applied",
        "retry_strategy_override_applied",
        "strategy_override_conflict",
        "strategy_override_summary",
        "strategy_override_ignored_fields",
        "partial_filter_fallback", // legacy Failed 版本
        "adaptive_tls_rollout"
    ];
    for e in &events { if let Event::Task(TaskEvent::Failed { code, .. }) = e { if let Some(c) = code { assert!(!legacy_codes.contains(&c.as_str()), "unexpected legacy TaskErrorEvent code {} still present", c); } } }
    });
}
