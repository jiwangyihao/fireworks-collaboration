#![cfg(not(feature = "tauri-app"))]
use fireworks_collaboration_lib::core::tasks::registry::TaskRegistry; // ensure linkage
use fireworks_collaboration_lib::events::emitter::AppHandle;
use fireworks_collaboration_lib::events::structured::{set_global_event_bus, MemoryEventBus};
use fireworks_collaboration_lib::tests_support::event_assert::{assert_applied_code};
use fireworks_collaboration_lib::core::tasks::model::TaskKind;


#[test]
fn strategy_override_summary_and_gating() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
    std::env::set_var("FWC_STRATEGY_APPLIED_EVENTS", "1");
    let _ = set_global_event_bus(std::sync::Arc::new(MemoryEventBus::new()));
    // 构建一个本地临时仓库，确保 parse 路径和策略流程执行
    let tmp_src = tempfile::tempdir().unwrap();
    let repo = git2::Repository::init(tmp_src.path()).unwrap();
    std::fs::write(tmp_src.path().join("readme.txt"), "hi").unwrap();
    let mut idx = repo.index().unwrap(); idx.add_path(std::path::Path::new("readme.txt")).unwrap(); idx.write().unwrap();
    let tree_id = idx.write_tree().unwrap(); let tree = repo.find_tree(tree_id).unwrap(); let sig = repo.signature().unwrap(); repo.commit(Some("HEAD"), &sig, &sig, "c1", &tree, &[]).unwrap();
    let reg = std::sync::Arc::new(TaskRegistry::new());
    let dest_dir = tempfile::tempdir().unwrap();
    let (id, token) = reg.create(TaskKind::GitClone { repo: tmp_src.path().to_string_lossy().to_string(), dest: dest_dir.path().to_string_lossy().to_string(), depth: None, filter: None, strategy_override: None });
    // 组合 http+retry 两个改变字段，确保 appliedCodes 收集到 http 和 retry
    let override_json = serde_json::json!({"http": {"followRedirects": false}, "retry": {"max": 5}});
        let handle = reg.spawn_git_clone_task_with_opts(Some(AppHandle {}), id, token.clone(), tmp_src.path().to_string_lossy().to_string(), dest_dir.path().to_string_lossy().to_string(), None, None, Some(override_json));
        // 等待任务完成（本地 clone 很快）；若过长，可超时取消
        let _ = handle.await; // 任务结束
        // 直接使用结构化 summary applied code 断言（summary 事件保证发送）
        assert_applied_code(&id.to_string(), "http_strategy_override_applied");
        assert_applied_code(&id.to_string(), "retry_strategy_override_applied");
        // 第二阶段：验证 gating 关闭
        std::env::set_var("FWC_STRATEGY_APPLIED_EVENTS", "0");
    // gating=0 仅抑制 legacy applied 事件，不影响 summary；我们仍然期望 summary 包含 codes。
        let tmp_src2 = tempfile::tempdir().unwrap();
        let repo2 = git2::Repository::init(tmp_src2.path()).unwrap();
        std::fs::write(tmp_src2.path().join("g.txt"), "hi").unwrap();
        let mut idx2 = repo2.index().unwrap(); idx2.add_path(std::path::Path::new("g.txt")).unwrap(); idx2.write().unwrap();
        let tree_id2 = idx2.write_tree().unwrap(); let tree2 = repo2.find_tree(tree_id2).unwrap(); let sig2 = repo2.signature().unwrap(); repo2.commit(Some("HEAD"), &sig2, &sig2, "c1", &tree2, &[]).unwrap();
        let dest2 = tempfile::tempdir().unwrap();
        let (gid, gtoken) = reg.create(TaskKind::GitClone { repo: tmp_src2.path().to_string_lossy().to_string(), dest: dest2.path().to_string_lossy().to_string(), depth: None, filter: None, strategy_override: None });
        let govr = serde_json::json!({"http": {"followRedirects": false}, "retry": {"max": 3}});
        let ghandle = reg.spawn_git_clone_task_with_opts(Some(AppHandle {}), gid, gtoken.clone(), tmp_src2.path().to_string_lossy().to_string(), dest2.path().to_string_lossy().to_string(), None, None, Some(govr));
        let _ = ghandle.await;
        assert_applied_code(&gid.to_string(), "http_strategy_override_applied");
        assert_applied_code(&gid.to_string(), "retry_strategy_override_applied");
    });
}
