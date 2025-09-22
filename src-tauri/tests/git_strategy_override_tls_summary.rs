#![cfg(not(feature = "tauri-app"))]
//! 覆盖 TLS override 在 summary 中的呈现与 gating=0 禁止独立事件。

use fireworks_collaboration_lib::events::emitter::AppHandle;
use fireworks_collaboration_lib::events::structured::{set_global_event_bus, MemoryEventBus};
use fireworks_collaboration_lib::tests_support::event_assert::{assert_applied_code, assert_conflict_kind, assert_tls_applied};
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
    // gating 只影响 legacy 事件；结构化 TlsApplied 始终按变更存在
    std::env::set_var("FWC_STRATEGY_APPLIED_EVENTS", "1");
    let _ = set_global_event_bus(std::sync::Arc::new(MemoryEventBus::new()));
    let origin = make_repo();
    let dest_dir = tempfile::tempdir().unwrap();
    let reg = std::sync::Arc::new(TaskRegistry::new());
    let (id, token) = reg.create(TaskKind::GitClone { repo: origin.path().to_string_lossy().to_string(), dest: dest_dir.path().to_string_lossy().to_string(), depth: None, filter: None, strategy_override: None });
    let override_json = serde_json::json!({"tls": {"insecureSkipVerify": true, "skipSanWhitelist": true}}); // 触发规范化 conflict
    let handle = reg.spawn_git_clone_task_with_opts(Some(AppHandle {}), id, token.clone(), origin.path().to_string_lossy().to_string(), dest_dir.path().to_string_lossy().to_string(), None, None, Some(override_json));
    let _ = handle.await;
    assert_applied_code(&id.to_string(), "tls_strategy_override_applied");
    assert_tls_applied(&id.to_string(), true);
    assert_conflict_kind(&id.to_string(), "tls", Some("normalizes"));

    // 2) gating=0 => 仍有 summary + appliedCodes 含 tls，但无独立 tls_strategy_override_applied
    std::env::set_var("FWC_STRATEGY_APPLIED_EVENTS", "0");
    // 不能重置 global bus; 使用不同 id 过滤
    let origin2 = make_repo();
    let dest_dir2 = tempfile::tempdir().unwrap();
    let (id2, token2) = reg.create(TaskKind::GitClone { repo: origin2.path().to_string_lossy().to_string(), dest: dest_dir2.path().to_string_lossy().to_string(), depth: None, filter: None, strategy_override: None });
    let override_json2 = serde_json::json!({"tls": {"insecureSkipVerify": true}}); // 仅一个字段变化
    let handle2 = reg.spawn_git_clone_task_with_opts(Some(AppHandle {}), id2, token2.clone(), origin2.path().to_string_lossy().to_string(), dest_dir2.path().to_string_lossy().to_string(), None, None, Some(override_json2));
    let _ = handle2.await;
    assert_applied_code(&id2.to_string(), "tls_strategy_override_applied");
    assert_tls_applied(&id2.to_string(), true);
}
