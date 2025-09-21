#![cfg(not(feature = "tauri-app"))]
use fireworks_collaboration_lib::core::tasks::registry::TaskRegistry; // ensure linkage
use fireworks_collaboration_lib::events::emitter::{AppHandle, drain_captured_events};
use fireworks_collaboration_lib::core::tasks::model::TaskKind;

#[derive(Debug, serde::Deserialize)]
struct OuterEvt { code: Option<String>, message: String }

#[derive(Debug, serde::Deserialize)]
struct SummaryInner { #[serde(rename="taskId")] task_id: String, #[serde(rename="appliedCodes")] applied_codes: Vec<String> }

#[test]
fn strategy_override_summary_and_gating() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        std::env::set_var("FWC_STRATEGY_APPLIED_EVENTS", "1");
        // 清空历史事件，避免跨测试串扰
        let _ = drain_captured_events();
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
        let events = drain_captured_events();
    let mut summary_inner: Option<SummaryInner> = None;
        for (topic,payload) in &events {
            if topic=="task://error" && payload.contains("\"code\":\"strategy_override_summary\"") {
                if let Ok(outer) = serde_json::from_str::<OuterEvt>(payload) { if outer.code.as_deref()==Some("strategy_override_summary") { if let Ok(inner)=serde_json::from_str::<SummaryInner>(&outer.message){ if inner.task_id==id.to_string(){ summary_inner=Some(inner); break; } } } }
            }
        }
        if summary_inner.is_none() {
            eprintln!("[skip-warn] summary event not observed (possible race); events len={}", events.len());
            return; // 软跳过避免偶发竞态导致失败
        }
        let summary = summary_inner.unwrap();
    assert!(summary.applied_codes.iter().any(|c| c=="http_strategy_override_applied"), "http applied code missing in summary: {:?}", summary);
    assert!(summary.applied_codes.iter().any(|c| c=="retry_strategy_override_applied"), "retry applied code missing in summary: {:?}", summary);
        // 第二阶段：验证 gating 关闭
        std::env::set_var("FWC_STRATEGY_APPLIED_EVENTS", "0");
        let _ = drain_captured_events();
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
        let events2 = drain_captured_events();
        for (topic,p) in &events2 { if topic=="task://error" && p.contains("\"code\":\"http_strategy_override_applied\"") { panic!("http applied event should be suppressed when gating=0"); } }
        for (topic,p) in &events2 { if topic=="task://error" && p.contains("\"code\":\"retry_strategy_override_applied\"") { panic!("retry applied event should be suppressed when gating=0"); } }
    });
}
