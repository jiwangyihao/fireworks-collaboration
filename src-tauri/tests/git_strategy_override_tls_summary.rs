#![cfg(not(feature = "tauri-app"))]
//! 覆盖 TLS override 在 summary 中的呈现与 gating=0 禁止独立事件。

use fireworks_collaboration_lib::events::emitter::{AppHandle, drain_captured_events};
use fireworks_collaboration_lib::core::tasks::registry::TaskRegistry;
use fireworks_collaboration_lib::core::tasks::model::TaskKind;

fn make_repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let repo = git2::Repository::init(dir.path()).unwrap();
    std::fs::write(dir.path().join("f.txt"), "one").unwrap();
    let mut idx = repo.index().unwrap(); idx.add_path(std::path::Path::new("f.txt")).unwrap(); idx.write().unwrap();
    let tree_id = idx.write_tree().unwrap(); let tree = repo.find_tree(tree_id).unwrap(); let sig = repo.signature().unwrap();
    repo.commit(Some("HEAD"), &sig, &sig, "c1", &tree, &[]).unwrap();
    dir
}

#[tokio::test]
async fn tls_override_summary_and_gating() {
    // 1) gating=1 => summary + 独立 tls 应用事件
    std::env::set_var("FWC_STRATEGY_APPLIED_EVENTS", "1");
    let _ = drain_captured_events();
    let origin = make_repo();
    let dest_dir = tempfile::tempdir().unwrap();
    let reg = std::sync::Arc::new(TaskRegistry::new());
    let (id, token) = reg.create(TaskKind::GitClone { repo: origin.path().to_string_lossy().to_string(), dest: dest_dir.path().to_string_lossy().to_string(), depth: None, filter: None, strategy_override: None });
    let override_json = serde_json::json!({"tls": {"insecureSkipVerify": true, "skipSanWhitelist": true}}); // 触发规范化 conflict
    let handle = reg.spawn_git_clone_task_with_opts(Some(AppHandle {}), id, token.clone(), origin.path().to_string_lossy().to_string(), dest_dir.path().to_string_lossy().to_string(), None, None, Some(override_json));
    let _ = handle.await;
    let events = drain_captured_events();
    let mut has_summary=false; let mut has_tls=false; let mut has_conflict=false;
    for (topic,p) in &events { if topic=="task://error" { if p.contains("tls_strategy_override_applied") { has_tls=true; } if p.contains("strategy_override_summary") && p.contains(&id.to_string()) && p.contains("tls_strategy_override_applied") { has_summary=true; } if p.contains("strategy_override_conflict") { has_conflict=true; } } }
    assert!(has_summary, "summary missing tls applied code; events={:?}", events);
    assert!(has_tls, "independent tls applied event missing when gating=1; events={:?}", events);
    assert!(has_conflict, "conflict normalization event missing; events={:?}", events);

    // 2) gating=0 => 仍有 summary + appliedCodes 含 tls，但无独立 tls_strategy_override_applied
    std::env::set_var("FWC_STRATEGY_APPLIED_EVENTS", "0");
    let _ = drain_captured_events();
    let origin2 = make_repo();
    let dest_dir2 = tempfile::tempdir().unwrap();
    let (id2, token2) = reg.create(TaskKind::GitClone { repo: origin2.path().to_string_lossy().to_string(), dest: dest_dir2.path().to_string_lossy().to_string(), depth: None, filter: None, strategy_override: None });
    let override_json2 = serde_json::json!({"tls": {"insecureSkipVerify": true}}); // 仅一个字段变化
    let handle2 = reg.spawn_git_clone_task_with_opts(Some(AppHandle {}), id2, token2.clone(), origin2.path().to_string_lossy().to_string(), dest_dir2.path().to_string_lossy().to_string(), None, None, Some(override_json2));
    let _ = handle2.await;
    let events2 = drain_captured_events();
    let mut summary_ok=false; let mut has_independent=false;
    for (topic,p) in &events2 { if topic=="task://error" { if p.contains("strategy_override_summary") && p.contains(&id2.to_string()) && p.contains("tls_strategy_override_applied") { summary_ok=true; } if p.contains("tls_strategy_override_applied") && !p.contains("strategy_override_summary") { has_independent=true; } } }
    assert!(summary_ok, "gating=0 summary missing tls appliedCodes; events={:?}", events2);
    assert!(!has_independent, "gating=0 should suppress independent tls applied event; events={:?}", events2);
}
