//! Mixed clone+fetch+push TLS override scenarios including empty and unknown fields.

use std::sync::Arc;
use fireworks_collaboration_lib::tasks::{TaskRegistry, TaskKind};
use fireworks_collaboration_lib::events::emitter::{AppHandle, drain_captured_events};
use fireworks_collaboration_lib::tasks::model::TaskState;

async fn wait_done(reg:&TaskRegistry, id:uuid::Uuid){ for _ in 0..120 { if let Some(s)=reg.snapshot(&id) { if matches!(s.state, TaskState::Completed | TaskState::Failed | TaskState::Canceled) { break; } } tokio::time::sleep(std::time::Duration::from_millis(35)).await; } }

fn make_repo() -> tempfile::TempDir {
    let tmp = tempfile::tempdir().unwrap();
    let repo = git2::Repository::init(tmp.path()).unwrap();
    let f = tmp.path().join("f.txt");
    std::fs::write(&f, "x").unwrap();
    let mut idx = repo.index().unwrap(); idx.add_path(std::path::Path::new("f.txt")).unwrap(); idx.write().unwrap();
    let tree_id = idx.write_tree().unwrap(); let tree = repo.find_tree(tree_id).unwrap();
    let sig = repo.signature().unwrap(); repo.commit(Some("HEAD"), &sig,&sig, "init", &tree, &[]).unwrap();
    tmp
}

#[test]
fn tls_mixed_scenarios() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let src = make_repo(); let src_path = src.path().to_string_lossy().to_string();
        let reg = Arc::new(TaskRegistry::new()); let app = AppHandle;

        // clone A: insecure only
        let dest_a = tempfile::tempdir().unwrap();
        let ova = serde_json::json!({"tls": {"insecureSkipVerify": true}});
        let (id_a, tk_a) = reg.create(TaskKind::GitClone { repo: src_path.clone(), dest: dest_a.path().to_string_lossy().to_string(), depth: None, filter: None, strategy_override: Some(ova.clone()) });
        let ha = reg.clone().spawn_git_clone_task_with_opts(Some(app.clone()), id_a, tk_a, src_path.clone(), dest_a.path().to_string_lossy().to_string(), None, None, Some(ova));

        // clone baseline for fetch/push
        let base = tempfile::tempdir().unwrap();
        let (id_base, tk_base) = reg.create(TaskKind::GitClone { repo: src_path.clone(), dest: base.path().to_string_lossy().to_string(), depth: None, filter: None, strategy_override: None });
        let h_base = reg.clone().spawn_git_clone_task_with_opts(Some(app.clone()), id_base, tk_base, src_path.clone(), base.path().to_string_lossy().to_string(), None, None, None);

    wait_done(&reg, id_a).await; ha.await.unwrap();
    wait_done(&reg, id_base).await; h_base.await.unwrap();
    // 不清空事件，保留 clone insecure 事件用于最终统计；基线 clone 不会产生 tls 覆盖事件。

        // fetch B: tls empty object (should NOT emit)
        let ovb = serde_json::json!({"tls": {}});
        let (id_b, tk_b) = reg.create(TaskKind::GitFetch { repo: src_path.clone(), dest: base.path().to_string_lossy().to_string(), depth: None, filter: None, strategy_override: Some(ovb.clone()) });
        let hb = reg.clone().spawn_git_fetch_task_with_opts(Some(app.clone()), id_b, tk_b, src_path.clone(), base.path().to_string_lossy().to_string(), None, None, None, Some(ovb));

        // push C: skipSan only
        let (id_c, tk_c) = reg.create(TaskKind::GitPush { dest: base.path().to_string_lossy().to_string(), remote: None, refspecs: None, username: None, password: None, strategy_override: Some(serde_json::json!({"tls": {"skipSanWhitelist": true}})) });
        let hc = reg.clone().spawn_git_push_task(Some(app.clone()), id_c, tk_c, base.path().to_string_lossy().to_string(), None, None, None, None, Some(serde_json::json!({"tls": {"skipSanWhitelist": true}})) );

        // fetch D: unknown field inside tls (should parse with warning, no event)
        let ovd = serde_json::json!({"tls": {"foo": true}});
        let (id_d, tk_d) = reg.create(TaskKind::GitFetch { repo: src_path.clone(), dest: base.path().to_string_lossy().to_string(), depth: None, filter: None, strategy_override: Some(ovd.clone()) });
        let hd = reg.clone().spawn_git_fetch_task_with_opts(Some(app.clone()), id_d, tk_d, src_path.clone(), base.path().to_string_lossy().to_string(), None, None, None, Some(ovd));

        wait_done(&reg, id_b).await; hb.await.unwrap();
        wait_done(&reg, id_c).await; hc.await.unwrap();
        wait_done(&reg, id_d).await; hd.await.unwrap();

    let ev = drain_captured_events();
    let mut tls_a=0; let mut tls_c=0; let mut unexpected=Vec::new();
    for (topic,p) in &ev { if topic=="task://error" { if p.contains("tls_strategy_override_applied") { if p.contains(&id_a.to_string()) { tls_a+=1; } else if p.contains(&id_c.to_string()) { tls_c+=1; } else { unexpected.push(p.clone()); } } } }
        assert_eq!(tls_a,1,"clone insecure should emit once");
        assert_eq!(tls_c,1,"push skipSan should emit once");
        assert!(unexpected.is_empty(), "no tls events expected for empty or unknown field overrides: {:?}", unexpected);
    });
}
