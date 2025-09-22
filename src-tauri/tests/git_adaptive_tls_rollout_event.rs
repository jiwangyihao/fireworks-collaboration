//! Tests for adaptive_tls_rollout informational event under different rollout percents.
//! We simulate git clone against a local bare repo (no network TLS), but the URL rewrite logic
//! still executes for https://github.com/* style URLs. We therefore target a public-like URL
//! yet rely on rewrite logic only (no real network handshakes are performed in these tests).

use fireworks_collaboration_lib::events::emitter::{peek_captured_events, drain_captured_events};
use fireworks_collaboration_lib::core::tasks::registry::test_emit_clone_strategy_and_rollout;
use std::sync::{OnceLock, Mutex};

// 全局一次性配置基目录 + 串行锁，避免测试并发导致配置被其他测试覆盖。
fn test_serial_guard() -> std::sync::MutexGuard<'static, ()> {
    static M: OnceLock<Mutex<()>> = OnceLock::new();
    M.get_or_init(|| Mutex::new(())).lock().unwrap()
}

fn ensure_base_dir_once() {
    static INIT: OnceLock<()> = OnceLock::new();
    INIT.get_or_init(|| {
        let dir = tempfile::tempdir().expect("temp cfg dir");
        // into_path 让目录在进程生命周期内保留，避免被删除
        let path = dir.into_path();
        fireworks_collaboration_lib::core::config::loader::set_global_base_dir(&path);
    });
}

// Test helper: ensure proxy-related env vars do not interfere with rewrite logic.
// Some developer / CI environments may inject HTTP(S)_PROXY which disables adaptive TLS rewrite.
// We clear them temporarily inside each test to get deterministic behavior.
struct ProxyEnvGuard { saved: Vec<(String, String)> }
impl ProxyEnvGuard { fn new() -> Self { let keys=["HTTPS_PROXY","https_proxy","HTTP_PROXY","http_proxy","ALL_PROXY","all_proxy"]; let mut saved=Vec::new(); for k in keys { if let Ok(v)=std::env::var(k){ saved.push((k.to_string(),v)); std::env::remove_var(k);} } Self{saved} } }
impl Drop for ProxyEnvGuard { fn drop(&mut self){ for (k,v) in self.saved.drain(..){ std::env::set_var(k,v);} } }

fn init_local_repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let repo = git2::Repository::init(dir.path()).unwrap();
    std::fs::write(dir.path().join("a.txt"), "v1").unwrap();
    let mut index = repo.index().unwrap(); index.add_path(std::path::Path::new("a.txt")).unwrap(); index.write().unwrap();
    let tree = repo.find_tree(index.write_tree().unwrap()).unwrap();
    let sig = repo.signature().unwrap(); repo.commit(Some("HEAD"), &sig,&sig,"c1", &tree, &[]).unwrap();
    dir
}

/// Helper to wait until task finished (Completed/Failed) or timeout.
// No longer need wait_task since we directly invoke helper (no async network)

#[test]
fn test_a_rollout_100_emits_event() {
    let _lock = test_serial_guard();
    ensure_base_dir_once();
    // Ensure clean captured events
    let _ = drain_captured_events();
    let _proxy_guard = ProxyEnvGuard::new();

    // 显式设置 100%（不依赖默认值，确保与其它测试互不影响）
    let mut cfg = fireworks_collaboration_lib::core::config::loader::load_or_init().expect("load cfg");
    cfg.http.fake_sni_rollout_percent = 100;
    fireworks_collaboration_lib::core::config::loader::save(&cfg).expect("save cfg");

    // Use a github-like URL to trigger whitelist match & rewrite（不需要真实网络）
    let _origin = init_local_repo();
    let repo_url = "https://github.com/owner/repo";
    let id = uuid::Uuid::new_v4();
    test_emit_clone_strategy_and_rollout(repo_url, id);

    let events = peek_captured_events();
    let mut found = false;
    for (topic,payload) in &events { if topic=="task://error" && payload.contains("adaptive_tls_rollout") && payload.contains(&id.to_string()) { found=true; break; } }
    assert!(found, "expected adaptive_tls_rollout event for rollout=100; events={events:?}");
}

#[test]
fn test_b_rollout_0_no_event() {
    let _lock = test_serial_guard();
    ensure_base_dir_once();
    let _ = drain_captured_events();
    let _proxy_guard = ProxyEnvGuard::new();
    // 显式写入 0% 并保存
    let mut cfg = fireworks_collaboration_lib::core::config::loader::load_or_init().expect("load cfg");
    cfg.http.fake_sni_rollout_percent = 0; // disable
    fireworks_collaboration_lib::core::config::loader::save(&cfg).expect("save cfg");
    let repo_url = "https://github.com/owner/repo";
    let id = uuid::Uuid::new_v4();
    test_emit_clone_strategy_and_rollout(repo_url, id);

    let events = peek_captured_events();
    for (topic,payload) in &events { if topic=="task://error" && payload.contains("adaptive_tls_rollout") && payload.contains(&id.to_string()) { panic!("unexpected rollout event when percent=0: {payload}"); } }
}

#[test]
fn test_c_single_event_only_once() {
    let _lock = test_serial_guard();
    ensure_base_dir_once();
    let _ = drain_captured_events();
    let _proxy_guard = ProxyEnvGuard::new();
    let repo_url = "https://github.com/owner/repo";
    // ensure percent=100 again (覆盖可能被 test_b 写入的 0%)
    let mut cfg = fireworks_collaboration_lib::core::config::loader::load_or_init().expect("load cfg");
    cfg.http.fake_sni_rollout_percent = 100;
    fireworks_collaboration_lib::core::config::loader::save(&cfg).expect("save cfg");
    let id = uuid::Uuid::new_v4();
    test_emit_clone_strategy_and_rollout(repo_url, id);
    let events = peek_captured_events();
    let mut count=0; for (topic,payload) in &events { if topic=="task://error" && payload.contains("adaptive_tls_rollout") && payload.contains(&id.to_string()) { count+=1; } }
    assert!(count<=1, "rollout event should appear at most once, got {count} events={events:?}");
}
