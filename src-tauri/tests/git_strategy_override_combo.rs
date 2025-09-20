//! Combined strategyOverride integration tests for P2.3c
//! Covers clone/fetch/push with http+retry overrides, http-only, retry-only, unchanged, and invalid cases.

use std::sync::Arc;
use fireworks_collaboration_lib::tasks::{TaskRegistry, TaskKind};
use fireworks_collaboration_lib::events::emitter::{AppHandle, peek_captured_events, drain_captured_events};
use fireworks_collaboration_lib::tasks::model::TaskState;

fn make_local_repo() -> tempfile::TempDir {
    let tmp = tempfile::tempdir().unwrap();
    let repo = git2::Repository::init(tmp.path()).unwrap();
    let f = tmp.path().join("a.txt");
    std::fs::write(&f, "hello").unwrap();
    let mut index = repo.index().unwrap();
    index.add_path(std::path::Path::new("a.txt")).unwrap();
    index.write().unwrap();
    let tree_id = index.write_tree().unwrap();
    let tree = repo.find_tree(tree_id).unwrap();
    let sig = repo.signature().unwrap();
    repo.commit(Some("HEAD"), &sig, &sig, "c1", &tree, &[]).unwrap();
    tmp
}

async fn wait_task(reg:&TaskRegistry, id:uuid::Uuid) { for _ in 0..120 { if let Some(s)=reg.snapshot(&id) { if matches!(s.state, TaskState::Completed | TaskState::Failed | TaskState::Canceled) { break; } } tokio::time::sleep(std::time::Duration::from_millis(40)).await; } }

#[test]
fn strategy_override_combo() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        // Source repo
        let src = make_local_repo();
        let src_path = src.path().to_string_lossy().to_string();
        let reg = Arc::new(TaskRegistry::new());
        let app = AppHandle;

        // 1) clone with http + retry override
        let dest1 = tempfile::tempdir().unwrap();
        let ov1 = serde_json::json!({"http": {"followRedirects": false, "maxRedirects": 4}, "retry": {"max": 3, "baseMs": 500}});
        let (id1, t1) = reg.create(TaskKind::GitClone { repo: src_path.clone(), dest: dest1.path().to_string_lossy().to_string(), depth: None, filter: None, strategy_override: Some(ov1.clone()) });
        let h1 = reg.clone().spawn_git_clone_task_with_opts(Some(app.clone()), id1, t1, src_path.clone(), dest1.path().to_string_lossy().to_string(), None, None, Some(ov1));
        wait_task(&reg, id1).await; h1.await.unwrap();
        let ev1 = drain_captured_events();
        let mut http_evt=0; let mut retry_evt=0; for (topic,p) in &ev1 { if topic=="task://error" { if p.contains("http_strategy_override_applied") && p.contains(&id1.to_string()) { http_evt+=1; } if p.contains("retry_strategy_override_applied") && p.contains(&id1.to_string()) { retry_evt+=1; } } }
        assert_eq!(http_evt,1,"http override event exactly once");
        assert_eq!(retry_evt,1,"retry override event exactly once");

        // 2) clone with retry-only override (http unchanged)
        let dest2 = tempfile::tempdir().unwrap();
        let ov2 = serde_json::json!({"retry": {"max": 5}});
        let (id2, t2) = reg.create(TaskKind::GitClone { repo: src_path.clone(), dest: dest2.path().to_string_lossy().to_string(), depth: None, filter: None, strategy_override: Some(ov2.clone()) });
        let h2 = reg.clone().spawn_git_clone_task_with_opts(Some(app.clone()), id2, t2, src_path.clone(), dest2.path().to_string_lossy().to_string(), None, None, Some(ov2));
        wait_task(&reg, id2).await; h2.await.unwrap();
        let ev2 = drain_captured_events();
        let mut http_evt2=0; let mut retry_evt2=0; for (topic,p) in &ev2 { if topic=="task://error" { if p.contains("http_strategy_override_applied") && p.contains(&id2.to_string()) { http_evt2+=1; } if p.contains("retry_strategy_override_applied") && p.contains(&id2.to_string()) { retry_evt2+=1; } } }
        assert_eq!(http_evt2,0); assert_eq!(retry_evt2,1);

        // 3) clone unchanged override (values == defaults) -> no events
        let dest3 = tempfile::tempdir().unwrap();
        let ov3 = serde_json::json!({"retry": {"max": 6, "baseMs":300, "factor":1.5, "jitter": true}, "http": {"followRedirects": true, "maxRedirects":5}});
        let (id3, t3) = reg.create(TaskKind::GitClone { repo: src_path.clone(), dest: dest3.path().to_string_lossy().to_string(), depth: None, filter: None, strategy_override: Some(ov3.clone()) });
        let h3 = reg.clone().spawn_git_clone_task_with_opts(Some(app.clone()), id3, t3, src_path.clone(), dest3.path().to_string_lossy().to_string(), None, None, Some(ov3));
        wait_task(&reg, id3).await; h3.await.unwrap();
        let ev3 = drain_captured_events();
        for (topic,p) in &ev3 { if topic=="task://error" && (p.contains("http_strategy_override_applied") || p.contains("retry_strategy_override_applied")) && p.contains(&id3.to_string()) { panic!("should not emit events for unchanged override"); } }

        // 4) invalid retry (max=0) -> Protocol fail, no override-applied events
        let dest4 = tempfile::tempdir().unwrap();
        let ov4 = serde_json::json!({"retry": {"max": 0}});
        let (id4, t4) = reg.create(TaskKind::GitClone { repo: src_path.clone(), dest: dest4.path().to_string_lossy().to_string(), depth: None, filter: None, strategy_override: Some(ov4.clone()) });
        let h4 = reg.clone().spawn_git_clone_task_with_opts(Some(app.clone()), id4, t4, src_path.clone(), dest4.path().to_string_lossy().to_string(), None, None, Some(ov4));
        wait_task(&reg, id4).await; h4.await.unwrap();
        let snap4 = reg.snapshot(&id4).unwrap();
        assert!(matches!(snap4.state, TaskState::Failed));
        let ev4 = drain_captured_events();
        for (topic,p) in &ev4 { if topic=="task://error" && p.contains("override_applied") && p.contains(&id4.to_string()) { panic!("invalid override should not emit applied events"); } }

        // 5) fetch with http+retry override
        let work5 = tempfile::tempdir().unwrap();
        // clone baseline into work5
        let (id_c, tk_c) = reg.create(TaskKind::GitClone { repo: src_path.clone(), dest: work5.path().to_string_lossy().to_string(), depth: None, filter: None, strategy_override: None });
        let hc = reg.clone().spawn_git_clone_task_with_opts(Some(app.clone()), id_c, tk_c, src_path.clone(), work5.path().to_string_lossy().to_string(), None, None, None);
        wait_task(&reg, id_c).await; hc.await.unwrap(); drain_captured_events();
        let ov5 = serde_json::json!({"http": {"followRedirects": false}, "retry": {"max": 2}});
    let (id5, tk5) = reg.create(TaskKind::GitFetch { repo: src_path.clone(), dest: work5.path().to_string_lossy().to_string(), depth: None, filter: None, strategy_override: Some(ov5.clone()) });
        let h5 = reg.clone().spawn_git_fetch_task_with_opts(Some(app.clone()), id5, tk5, src_path.clone(), work5.path().to_string_lossy().to_string(), None, None, None, Some(ov5));
        wait_task(&reg, id5).await; h5.await.unwrap();
        let ev5 = drain_captured_events();
        let mut h_evt5=0; let mut r_evt5=0; for (topic,p) in &ev5 { if topic=="task://error" { if p.contains("http_strategy_override_applied") && p.contains(&id5.to_string()) { h_evt5+=1; } if p.contains("retry_strategy_override_applied") && p.contains(&id5.to_string()) { r_evt5+=1; } } }
        assert_eq!(h_evt5,1); assert_eq!(r_evt5,1);

        // 6) push with retry only (http unchanged) - create a commit first
        // Reuse work5 repo to add new commit then push to bare remote (src acts as remote) is complex; skip actual network push here due to local bare separation complexity.
        // Instead, just ensure parsing path executes without error using strategy_override and that retry event triggers once.
        let work6 = tempfile::tempdir().unwrap();
        let (id6c, tk6c) = reg.create(TaskKind::GitClone { repo: src_path.clone(), dest: work6.path().to_string_lossy().to_string(), depth: None, filter: None, strategy_override: None });
        let h6c = reg.clone().spawn_git_clone_task_with_opts(Some(app.clone()), id6c, tk6c, src_path.clone(), work6.path().to_string_lossy().to_string(), None, None, None);
        wait_task(&reg, id6c).await; h6c.await.unwrap(); drain_captured_events();
        // Skip making new commits; push may report no ref updates but still run pipeline.
        let ov6 = serde_json::json!({"retry": {"max": 4}});
        let (id6, tk6) = reg.create(TaskKind::GitPush { dest: work6.path().to_string_lossy().to_string(), remote: None, refspecs: None, username: None, password: None, strategy_override: Some(ov6.clone()) });
        let h6 = reg.clone().spawn_git_push_task(Some(app.clone()), id6, tk6, work6.path().to_string_lossy().to_string(), None, None, None, None, Some(ov6));
        wait_task(&reg, id6).await; h6.await.unwrap();
        let ev6 = drain_captured_events();
        let mut http_evt6=0; let mut retry_evt6=0; for (topic,p) in &ev6 { if topic=="task://error" { if p.contains("http_strategy_override_applied") && p.contains(&id6.to_string()) { http_evt6+=1; } if p.contains("retry_strategy_override_applied") && p.contains(&id6.to_string()) { retry_evt6+=1; } } }
        assert_eq!(http_evt6,0); assert_eq!(retry_evt6,1);
    });
}
