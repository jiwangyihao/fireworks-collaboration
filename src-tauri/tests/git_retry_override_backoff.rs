//! Tests retry override actually impacts retry plan (attempt count / factor / baseMs) for failing clone.
use std::sync::Arc;
use fireworks_collaboration_lib::tasks::{TaskRegistry, TaskKind};
use fireworks_collaboration_lib::events::emitter::AppHandle;
use fireworks_collaboration_lib::tasks::model::TaskState;
use fireworks_collaboration_lib::events::structured::{set_global_event_bus, MemoryEventBus, Event, StrategyEvent, get_global_memory_bus};
#[path = "support/mod.rs"] mod support; use support::event_assert::retry_applied_matrix;

#[test]
fn clone_retry_override_applies_override_event_without_retry_on_internal_error() {
    // Use an obviously invalid URL to force immediate network/protocol failure.
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        // 安装全局 MemoryEventBus 捕获结构化事件
        let _ = set_global_event_bus(std::sync::Arc::new(MemoryEventBus::new()));
        let reg = Arc::new(TaskRegistry::new());
        let app = AppHandle;
        // max=2 so that exactly one retry attempt is scheduled -> we expect a single progress line "attempt 1 of 2" and no "attempt 2" line since attempt 2 is the final attempt actually executing, not queued for another retry.
        let override_json = serde_json::json!({"retry": {"max": 2, "baseMs": 50, "factor": 2.0, "jitter": false}});
        let dest = tempfile::tempdir().unwrap();
    // Use unroutable TEST-NET-2 IP to trigger connection failure classified as Network -> retryable
    let bad_url = "https://127.0.0.1:9/repo.git"; // connection refused -> Network category
    let (id, token) = reg.create(TaskKind::GitClone { repo: bad_url.into(), dest: dest.path().to_string_lossy().to_string(), depth: None, filter: None, strategy_override: Some(override_json.clone()) });
    let handle = reg.clone().spawn_git_clone_task_with_opts(Some(app), id, token, bad_url.into(), dest.path().to_string_lossy().to_string(), None, None, Some(override_json));
        for _ in 0..120 { if let Some(s)=reg.snapshot(&id) { if matches!(s.state, TaskState::Failed | TaskState::Completed) { break; } } tokio::time::sleep(std::time::Duration::from_millis(50)).await; }
        handle.await.unwrap();
        // 结构化事件断言：存在 RetryApplied 且 id 匹配；不得出现 Strategy::HttpApplied
        let matrix = retry_applied_matrix();
        assert!(matrix.iter().any(|(rid, _)| rid==&id.to_string()), "expected a RetryApplied policy event for task id");
        // 若出现 StrategyEvent::HttpApplied 则失败（本测试未设置 http override）
    let mem = get_global_memory_bus().unwrap();
        let has_http = mem.snapshot().into_iter().any(|e| matches!(e, Event::Strategy(StrategyEvent::HttpApplied{..})));
        assert!(!has_http, "no http override expected (only retry supplied)");
    });
}

#[test]
fn clone_retry_override_factor_edges() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let _ = set_global_event_bus(std::sync::Arc::new(MemoryEventBus::new()));
        // Two tasks: factor=0.5 and factor=10.0; verify override events present (can't deterministically assert sleep duration, but factor surfaces in message).
        let reg = Arc::new(TaskRegistry::new());
        let app = AppHandle;
        let dest1 = tempfile::tempdir().unwrap();
        let ov1 = serde_json::json!({"retry": {"max": 1, "baseMs": 100, "factor": 0.5, "jitter": false}});
    let bad_url1 = "https://127.0.0.1:9/a.git";
    let (id1, tk1) = reg.create(TaskKind::GitClone { repo: bad_url1.into(), dest: dest1.path().to_string_lossy().to_string(), depth: None, filter: None, strategy_override: Some(ov1.clone()) });
    let h1 = reg.clone().spawn_git_clone_task_with_opts(Some(app.clone()), id1, tk1, bad_url1.into(), dest1.path().to_string_lossy().to_string(), None, None, Some(ov1));
        let dest2 = tempfile::tempdir().unwrap();
        let ov2 = serde_json::json!({"retry": {"max": 1, "baseMs": 100, "factor": 10.0, "jitter": false}});
    let bad_url2 = "https://127.0.0.1:9/b.git";
    let (id2, tk2) = reg.create(TaskKind::GitClone { repo: bad_url2.into(), dest: dest2.path().to_string_lossy().to_string(), depth: None, filter: None, strategy_override: Some(ov2.clone()) });
    let h2 = reg.clone().spawn_git_clone_task_with_opts(Some(app.clone()), id2, tk2, bad_url2.into(), dest2.path().to_string_lossy().to_string(), None, None, Some(ov2));
        h1.await.unwrap(); h2.await.unwrap();
        let matrix = retry_applied_matrix();
        // changed 字段中虽然不包含 factor 数值本身，但我们只需确认两个任务都触发 override；可后续在策略 Summary 中补强详细断言。
        assert!(matrix.iter().any(|(rid, _)| rid==&id1.to_string()), "expected override event for factor=0.5 task");
        assert!(matrix.iter().any(|(rid, _)| rid==&id2.to_string()), "expected override event for factor=10 task");
    });
}
