//! Tests retry override actually impacts retry plan (attempt count / factor / baseMs) for failing clone.
use std::sync::Arc;
use fireworks_collaboration_lib::tasks::{TaskRegistry, TaskKind};
use fireworks_collaboration_lib::events::emitter::{AppHandle, peek_captured_events};
use fireworks_collaboration_lib::tasks::model::TaskState;

#[test]
fn clone_retry_override_applies_override_event_without_retry_on_internal_error() {
    // Use an obviously invalid URL to force immediate network/protocol failure.
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
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
    let events = peek_captured_events();
        let mut http_event_found=false; let mut progress_retry_attempts=0; let mut override_event=false;
    // Debug dump for diagnosis
    for (topic,p) in &events { if topic=="task://error" { if p.contains("retry_strategy_override_applied") && p.contains(&id.to_string()) { override_event=true; } if p.contains("http_strategy_override_applied") { http_event_found=true; } } if topic=="task://progress" && p.contains("Retrying (attempt 1 of 2)") { progress_retry_attempts+=1; } }
        assert!(override_event, "expected retry override applied event");
        assert!(!http_event_found, "no http override expected (only retry supplied)");
    // 更新：由于改进了网络错误分类（中文/连接拒绝更可能归为 Network），此处允许出现一次重试进度。
    assert!(progress_retry_attempts <= 1, "expected at most one retry schedule, got {}", progress_retry_attempts);
    });
}

#[test]
fn clone_retry_override_factor_edges() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
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
        // Might interleave; gather once
        let events = peek_captured_events();
        let mut f05=false; let mut f10=false;
        for (topic,p) in &events { if topic=="task://error" && p.contains("retry_strategy_override_applied") { if p.contains(&id1.to_string()) && p.contains("factor=0.5") { f05=true; } if p.contains(&id2.to_string()) && p.contains("factor=10") { f10=true; } } }
        assert!(f05, "factor=0.5 override event should appear");
        assert!(f10, "factor=10.0 override event should appear");
    });
}
