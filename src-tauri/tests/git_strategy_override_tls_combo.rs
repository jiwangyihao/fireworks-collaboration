//! Combined HTTP+TLS+Retry strategy override integration tests (extended P2.3d)
//! Focus: event emission correctness & per-task isolation under parallel tasks.

use std::sync::Arc;
use fireworks_collaboration_lib::tasks::{TaskRegistry, TaskKind};
use fireworks_collaboration_lib::events::emitter::{AppHandle, drain_captured_events};
use fireworks_collaboration_lib::tasks::model::TaskState;

async fn wait_task(reg:&TaskRegistry, id:uuid::Uuid) { for _ in 0..120 { if let Some(s)=reg.snapshot(&id) { if matches!(s.state, TaskState::Completed | TaskState::Failed | TaskState::Canceled) { break; } } tokio::time::sleep(std::time::Duration::from_millis(35)).await; } }

fn make_local_repo() -> tempfile::TempDir {
    let tmp = tempfile::tempdir().unwrap();
    let repo = git2::Repository::init(tmp.path()).unwrap();
    let f = tmp.path().join("seed.txt");
    std::fs::write(&f, "hello").unwrap();
    let mut index = repo.index().unwrap();
    index.add_path(std::path::Path::new("seed.txt")).unwrap();
    index.write().unwrap();
    let tree_id = index.write_tree().unwrap();
    let tree = repo.find_tree(tree_id).unwrap();
    let sig = repo.signature().unwrap();
    repo.commit(Some("HEAD"), &sig, &sig, "seed", &tree, &[]).unwrap();
    tmp
}

#[test]
fn strategy_override_http_tls_retry_combo_parallel() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let src = make_local_repo();
        let src_path = src.path().to_string_lossy().to_string();
        let reg = Arc::new(TaskRegistry::new());
        let app = AppHandle;

        // Prepare three destinations
        let d1 = tempfile::tempdir().unwrap();
        let d2 = tempfile::tempdir().unwrap();
        let d3 = tempfile::tempdir().unwrap();

        // Task 1: http+tls+retry all changed
        let ov1 = serde_json::json!({
            "http": {"followRedirects": false},
            "tls": {"insecureSkipVerify": true, "skipSanWhitelist": true},
            "retry": {"max": 3, "baseMs": 400, "factor": 2.0, "jitter": false}
        });
        let (id1, tk1) = reg.create(TaskKind::GitClone { repo: src_path.clone(), dest: d1.path().to_string_lossy().to_string(), depth: None, filter: None, strategy_override: Some(ov1.clone()) });
        let h1 = reg.clone().spawn_git_clone_task_with_opts(Some(app.clone()), id1, tk1, src_path.clone(), d1.path().to_string_lossy().to_string(), None, None, Some(ov1));

        // Task 2: tls-only change (skipSanWhitelist)
        let ov2 = serde_json::json!({"tls": {"skipSanWhitelist": true}});
        let (id2, tk2) = reg.create(TaskKind::GitClone { repo: src_path.clone(), dest: d2.path().to_string_lossy().to_string(), depth: None, filter: None, strategy_override: Some(ov2.clone()) });
        let h2 = reg.clone().spawn_git_clone_task_with_opts(Some(app.clone()), id2, tk2, src_path.clone(), d2.path().to_string_lossy().to_string(), None, None, Some(ov2));

        // Task 3: unchanged override (no events) http same, tls same, retry same as defaults
        let ov3 = serde_json::json!({
            "http": {"followRedirects": true},
            "tls": {"insecureSkipVerify": false, "skipSanWhitelist": false},
            "retry": {"max": 6, "baseMs":300, "factor":1.5, "jitter": true}
        });
        let (id3, tk3) = reg.create(TaskKind::GitClone { repo: src_path.clone(), dest: d3.path().to_string_lossy().to_string(), depth: None, filter: None, strategy_override: Some(ov3.clone()) });
        let h3 = reg.clone().spawn_git_clone_task_with_opts(Some(app.clone()), id3, tk3, src_path.clone(), d3.path().to_string_lossy().to_string(), None, None, Some(ov3));

        // Await all
        wait_task(&reg, id1).await; wait_task(&reg, id2).await; wait_task(&reg, id3).await;
        h1.await.unwrap(); h2.await.unwrap(); h3.await.unwrap();

        let ev = drain_captured_events();
        let mut http1=0; let mut tls1=0; let mut retry1=0; let mut tls2=0;
    for (topic,p) in &ev { if topic=="task://error" { if p.contains(&id1.to_string()) { if p.contains("\"code\":\"http_strategy_override_applied\"") { http1+=1; } if p.contains("\"code\":\"tls_strategy_override_applied\"") { tls1+=1; } if p.contains("\"code\":\"retry_strategy_override_applied\"") { retry1+=1; } } if p.contains(&id2.to_string()) { if p.contains("\"code\":\"tls_strategy_override_applied\"") { tls2+=1; } if p.contains("\"code\":\"http_strategy_override_applied\"") || p.contains("\"code\":\"retry_strategy_override_applied\"") { panic!("unexpected http/retry event for task2"); } } if p.contains(&id3.to_string()) && (p.contains("\"code\":\"http_strategy_override_applied\"") || p.contains("\"code\":\"tls_strategy_override_applied\"") || p.contains("\"code\":\"retry_strategy_override_applied\"")) { panic!("no override events expected for task3"); } } }
        assert_eq!(http1,1); assert_eq!(tls1,1); assert_eq!(retry1,1); assert_eq!(tls2,1);
    });
}
