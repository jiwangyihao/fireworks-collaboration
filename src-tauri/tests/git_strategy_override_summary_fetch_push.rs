#![cfg(not(feature = "tauri-app"))]
//! 验证 GitFetch / GitPush 也产生 strategy_override_summary 事件，并包含 appliedCodes。

use fireworks_collaboration_lib::events::emitter::{AppHandle, drain_captured_events};
use fireworks_collaboration_lib::core::tasks::registry::TaskRegistry;
use fireworks_collaboration_lib::core::tasks::model::TaskKind;

fn init_repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let repo = git2::Repository::init(dir.path()).unwrap();
    std::fs::write(dir.path().join("a.txt"), "one").unwrap();
    let mut idx = repo.index().unwrap();
    idx.add_path(std::path::Path::new("a.txt")).unwrap();
    idx.write().unwrap();
    let tree_id = idx.write_tree().unwrap();
    let tree = repo.find_tree(tree_id).unwrap();
    let sig = repo.signature().unwrap();
    repo.commit(Some("HEAD"), &sig, &sig, "c1", &tree, &[]).unwrap();
    dir
}

#[tokio::test]
async fn fetch_summary_event_and_applied_codes() {
    std::env::set_var("FWC_STRATEGY_APPLIED_EVENTS", "1");
    let _ = drain_captured_events();
    let origin_dir = init_repo();
    // 创建工作仓库并添加远程
    let work_dir = tempfile::tempdir().unwrap();
    let work_repo = git2::Repository::init(work_dir.path()).unwrap();
    work_repo.remote("origin", origin_dir.path().to_string_lossy().as_ref()).unwrap();

    let reg = std::sync::Arc::new(TaskRegistry::new());
    let repo_url = origin_dir.path().to_string_lossy().to_string();
    let dest_path = work_dir.path().to_string_lossy().to_string();
    // fetch TaskKind 需要 repo+dest
    let (id, token) = reg.create(TaskKind::GitFetch { repo: repo_url.clone(), dest: dest_path.clone(), depth: None, filter: None, strategy_override: None });
    let override_json = serde_json::json!({"retry": {"max": 4}});
    let handle = reg.spawn_git_fetch_task_with_opts(Some(AppHandle {}), id, token.clone(), repo_url, dest_path, None, None, None, Some(override_json));
    let _ = handle.await;

    let events = drain_captured_events();
    let mut found_summary = false;
    for (topic,p) in &events {
        if topic=="task://error" && p.contains("\"code\":\"strategy_override_summary\"") && p.contains(&id.to_string()) {
            // 断言 appliedCodes 中包含 retry
            if p.contains("retry_strategy_override_applied") { found_summary = true; break; }
        }
    }
    assert!(found_summary, "fetch summary with retry appliedCodes not found; events={:?}", events);
}

#[tokio::test]
async fn push_summary_event_and_gating_off() {
    std::env::set_var("FWC_STRATEGY_APPLIED_EVENTS", "0");
    let _ = drain_captured_events();
    // 初始化一个包含提交的仓库并克隆到另一个目录作为推送来源
    let origin_dir = init_repo();
    let src_clone = tempfile::tempdir().unwrap();
    // 简单用 system git 克隆太重；这里直接再次 init 并添加 remote 指向 origin，然后 push 默认分支
    // 创建源仓库（带不同内容便于 push）
    let src_repo = git2::Repository::init(src_clone.path()).unwrap();
    std::fs::write(src_clone.path().join("b.txt"), "two").unwrap();
    let mut idx = src_repo.index().unwrap(); idx.add_path(std::path::Path::new("b.txt")).unwrap(); idx.write().unwrap();
    let tree_id = idx.write_tree().unwrap(); let tree = src_repo.find_tree(tree_id).unwrap(); let sig = src_repo.signature().unwrap(); src_repo.commit(Some("HEAD"), &sig, &sig, "c1", &tree, &[]).unwrap();
    src_repo.remote("origin", origin_dir.path().to_string_lossy().as_ref()).unwrap();

    let reg = std::sync::Arc::new(TaskRegistry::new());
    let (pid, ptoken) = reg.create(TaskKind::GitPush { dest: src_clone.path().to_string_lossy().to_string(), remote: Some("origin".into()), refspecs: None, username: None, password: None, strategy_override: None });
    let override_json = serde_json::json!({"http": {"followRedirects": false}, "tls": {"insecureSkipVerify": true}});
    let handle = reg.spawn_git_push_task(Some(AppHandle {}), pid, ptoken.clone(), src_clone.path().to_string_lossy().to_string(), Some("origin".into()), None, None, None, Some(override_json));
    let _ = handle.await;
    let events = drain_captured_events();
    let mut summary_ok = false;
    let mut has_independent_applied = false;
    for (topic,p) in &events {
        if topic=="task://error" {
            if p.contains("\"code\":\"strategy_override_summary\"") && p.contains(&pid.to_string()) {
                if p.contains("http_strategy_override_applied") && p.contains("tls_strategy_override_applied") { summary_ok = true; }
            }
            // 独立 applied 事件格式含 code 字段且不应是 summary 本身
            if p.contains("_strategy_override_applied") && !p.contains("strategy_override_summary") { has_independent_applied = true; }
        }
    }
    assert!(summary_ok, "push summary missing or codes absent; events={:?}", events);
    assert!(!has_independent_applied, "gating off: should not emit independent applied events (push); events={:?}", events);
}
