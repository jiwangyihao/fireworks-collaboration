//! TLS strategyOverride integration tests (P2.3d)
//! Covers clone/fetch/push with tls overrides changed vs unchanged.

use std::sync::Arc;
use fireworks_collaboration_lib::tasks::{TaskRegistry, TaskKind};
use fireworks_collaboration_lib::events::emitter::{AppHandle, drain_captured_events};
use fireworks_collaboration_lib::tasks::model::TaskState;

async fn wait_task(reg:&TaskRegistry, id:uuid::Uuid) { for _ in 0..120 { if let Some(s)=reg.snapshot(&id) { if matches!(s.state, TaskState::Completed | TaskState::Failed | TaskState::Canceled) { break; } } tokio::time::sleep(std::time::Duration::from_millis(40)).await; } }

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

#[test]
fn tls_override_changed_and_unchanged() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let src = make_local_repo();
        let src_path = src.path().to_string_lossy().to_string();
        let reg = Arc::new(TaskRegistry::new());
        let app = AppHandle;

        // 1) clone with tls override (insecureSkipVerify=true) -> event once
        let dest1 = tempfile::tempdir().unwrap();
        let ov1 = serde_json::json!({"tls": {"insecureSkipVerify": true}});
        let (id1, tk1) = reg.create(TaskKind::GitClone { repo: src_path.clone(), dest: dest1.path().to_string_lossy().to_string(), depth: None, filter: None, strategy_override: Some(ov1.clone()) });
        let h1 = reg.clone().spawn_git_clone_task_with_opts(Some(app.clone()), id1, tk1, src_path.clone(), dest1.path().to_string_lossy().to_string(), None, None, Some(ov1));
        wait_task(&reg, id1).await; h1.await.unwrap();
        let ev1 = drain_captured_events();
        let mut tls_evt=0; for (topic,p) in &ev1 { if topic=="task://error" && p.contains("tls_strategy_override_applied") && p.contains(&id1.to_string()) { tls_evt+=1; } }
        assert_eq!(tls_evt,1,"tls override event exactly once for changed values");

        // 2) clone with unchanged tls override (defaults: insecure=false skipSan=false)
        let dest2 = tempfile::tempdir().unwrap();
        let ov2 = serde_json::json!({"tls": {"insecureSkipVerify": false, "skipSanWhitelist": false}});
        let (id2, tk2) = reg.create(TaskKind::GitClone { repo: src_path.clone(), dest: dest2.path().to_string_lossy().to_string(), depth: None, filter: None, strategy_override: Some(ov2.clone()) });
        let h2 = reg.clone().spawn_git_clone_task_with_opts(Some(app.clone()), id2, tk2, src_path.clone(), dest2.path().to_string_lossy().to_string(), None, None, Some(ov2));
        wait_task(&reg, id2).await; h2.await.unwrap();
        let ev2 = drain_captured_events();
        for (topic,p) in &ev2 { if topic=="task://error" && p.contains("tls_strategy_override_applied") && p.contains(&id2.to_string()) { panic!("should not emit tls override event when values unchanged"); } }

        // 3) fetch with tls override skipSanWhitelist=true
        let work3 = tempfile::tempdir().unwrap();
        // baseline clone
        let (idc, tkc) = reg.create(TaskKind::GitClone { repo: src_path.clone(), dest: work3.path().to_string_lossy().to_string(), depth: None, filter: None, strategy_override: None });
        let hc = reg.clone().spawn_git_clone_task_with_opts(Some(app.clone()), idc, tkc, src_path.clone(), work3.path().to_string_lossy().to_string(), None, None, None);
        wait_task(&reg, idc).await; hc.await.unwrap(); drain_captured_events();
        let ovf = serde_json::json!({"tls": {"skipSanWhitelist": true}});
        let (idf, tkf) = reg.create(TaskKind::GitFetch { repo: src_path.clone(), dest: work3.path().to_string_lossy().to_string(), depth: None, filter: None, strategy_override: Some(ovf.clone()) });
        let hf = reg.clone().spawn_git_fetch_task_with_opts(Some(app.clone()), idf, tkf, src_path.clone(), work3.path().to_string_lossy().to_string(), None, None, None, Some(ovf));
        wait_task(&reg, idf).await; hf.await.unwrap();
        let evf = drain_captured_events();
        let mut tls_evt_f=0; for (topic,p) in &evf { if topic=="task://error" && p.contains("tls_strategy_override_applied") && p.contains(&idf.to_string()) { tls_evt_f+=1; } }
        assert_eq!(tls_evt_f,1);

        // 4) push path with tls override (insecure=true, skipSan=true) -> event once
        let work4 = tempfile::tempdir().unwrap();
        let (idc4, tkc4) = reg.create(TaskKind::GitClone { repo: src_path.clone(), dest: work4.path().to_string_lossy().to_string(), depth: None, filter: None, strategy_override: None });
        let hc4 = reg.clone().spawn_git_clone_task_with_opts(Some(app.clone()), idc4, tkc4, src_path.clone(), work4.path().to_string_lossy().to_string(), None, None, None);
        wait_task(&reg, idc4).await; hc4.await.unwrap(); drain_captured_events();
        let ovp = serde_json::json!({"tls": {"insecureSkipVerify": true, "skipSanWhitelist": true}});
        let (idp, tkp) = reg.create(TaskKind::GitPush { dest: work4.path().to_string_lossy().to_string(), remote: None, refspecs: None, username: None, password: None, strategy_override: Some(ovp.clone()) });
        let hp = reg.clone().spawn_git_push_task(Some(app.clone()), idp, tkp, work4.path().to_string_lossy().to_string(), None, None, None, None, Some(ovp));
        wait_task(&reg, idp).await; hp.await.unwrap();
        let evp = drain_captured_events();
        let mut tls_evt_p=0; for (topic,p) in &evp { if topic=="task://error" && p.contains("tls_strategy_override_applied") && p.contains(&idp.to_string()) { tls_evt_p+=1; } }
        assert_eq!(tls_evt_p,1);
    });
}
