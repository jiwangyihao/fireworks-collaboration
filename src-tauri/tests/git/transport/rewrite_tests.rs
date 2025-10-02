use fireworks_collaboration_lib::core::git::transport::rewrite::{
    decide_https_to_custom, RewriteDecision,
};
use fireworks_collaboration_lib::core::config::model::AppConfig;
use std::sync::{Mutex, OnceLock};

// 全局锁保护环境变量访问
fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

// 内部测试函数，模拟 proxy_present 参数
fn decision_with_proxy(cfg: &AppConfig, url: &str, proxy_present: bool) -> RewriteDecision {
    let _guard = env_lock().lock().unwrap();
    // 由于 decide_https_to_custom_inner 是私有的，我们需要通过环境变量来模拟 proxy
    if proxy_present {
        // 测试时设置临时环境变量
        std::env::set_var("HTTP_PROXY", "http://proxy:8080");
        let result = decide_https_to_custom(cfg, url);
        std::env::remove_var("HTTP_PROXY");
        result
    } else {
        std::env::remove_var("HTTP_PROXY");
        std::env::remove_var("HTTPS_PROXY");
        std::env::remove_var("http_proxy");
        std::env::remove_var("https_proxy");
        decide_https_to_custom(cfg, url)
    }
}

#[test]
fn test_rewrite_only_when_enabled_and_whitelisted() {
    let mut cfg = AppConfig::default();
    cfg.http.fake_sni_enabled = true;
    cfg.http.fake_sni_rollout_percent = 100; // 全量
    cfg.tls.san_whitelist = vec!["github.com".into()];
    let url = "https://github.com/rust-lang/git2-rs";
    let out = decision_with_proxy(&cfg, url, false);
    assert_eq!(
        out.rewritten.as_deref(),
        Some("https+custom://github.com/rust-lang/git2-rs.git")
    );
    assert!(out.sampled);
    assert!(out.eligible);

    // 非 https 不改写
    assert!(decision_with_proxy(&cfg, "http://github.com/", false)
        .rewritten
        .is_none());

    // 关闭开关不改写
    cfg.http.fake_sni_enabled = false;
    let disabled = decision_with_proxy(&cfg, url, false);
    assert!(disabled.rewritten.is_none());
    assert!(!disabled.eligible);

    // 非白名单域不改写
    let mut cfg2 = AppConfig::default();
    cfg2.http.fake_sni_enabled = true;
    cfg2.tls.san_whitelist = vec!["example.com".into()];
    assert!(decision_with_proxy(&cfg2, url, false).rewritten.is_none());
}

#[test]
fn test_rewrite_disabled_when_proxy_env_present() {
    let mut cfg = AppConfig::default();
    cfg.http.fake_sni_enabled = true;
    cfg.tls.san_whitelist = vec!["github.com".into()];
    let url = "https://github.com/owner/repo";
    // 指定存在代理 -> 不改写
    let out = decision_with_proxy(&cfg, url, true);
    assert!(out.rewritten.is_none());
    assert!(!out.sampled);
    assert!(!out.eligible);
}

#[test]
fn test_rollout_sampling_zero() {
    let mut cfg = AppConfig::default();
    cfg.http.fake_sni_enabled = true;
    cfg.http.fake_sni_rollout_percent = 0; // 禁用
    cfg.tls.san_whitelist = vec!["github.com".into()];
    let url = "https://github.com/rust-lang/git2-rs";
    let out = decision_with_proxy(&cfg, url, false);
    assert!(out.rewritten.is_none());
    assert!(out.eligible);
    assert!(!out.sampled);
}

#[test]
fn test_rollout_sampling_partial_deterministic() {
    let mut cfg = AppConfig::default();
    cfg.http.fake_sni_enabled = true;
    cfg.http.fake_sni_rollout_percent = 10; // 10%
    cfg.tls.san_whitelist = vec!["github.com".into()];
    let url = "https://github.com/rust-lang/git2-rs";
    // 结果稳定（要么始终改写，要么始终不改写）
    let first = decision_with_proxy(&cfg, url, false).sampled;
    for _ in 0..10 {
        assert_eq!(first, decision_with_proxy(&cfg, url, false).sampled);
    }
}

#[test]
fn test_extra_allow_list() {
    let mut cfg = AppConfig::default();
    cfg.http.fake_sni_enabled = true;
    cfg.http.fake_sni_rollout_percent = 100;
    cfg.tls.san_whitelist = vec!["example.com".into()];
    cfg.http.host_allow_list_extra = vec!["github.com".into()];
    let url = "https://github.com/owner/repo";
    let out = decision_with_proxy(&cfg, url, false);
    assert!(out.sampled);
    assert!(out.rewritten.is_some());
}

#[test]
fn test_rewrite_preserves_query_and_fragment() {
    let mut cfg = AppConfig::default();
    cfg.http.fake_sni_enabled = true;
    cfg.http.fake_sni_rollout_percent = 100;
    cfg.tls.san_whitelist = vec!["github.com".into()];
    let url = "https://github.com/rust-lang/git2-rs?foo=bar#frag";
    let out = decision_with_proxy(&cfg, url, false);
    assert_eq!(
        out.rewritten.as_deref(),
        Some("https+custom://github.com/rust-lang/git2-rs.git?foo=bar#frag")
    );
    assert!(out.sampled);
}

#[test]
fn test_rewrite_existing_git_suffix_not_duplicated() {
    let mut cfg = AppConfig::default();
    cfg.http.fake_sni_enabled = true;
    cfg.http.fake_sni_rollout_percent = 100;
    cfg.tls.san_whitelist = vec!["github.com".into()];
    let url = "https://github.com/rust-lang/git2-rs.git";
    let out = decision_with_proxy(&cfg, url, false);
    let rewritten = out.rewritten.expect("expected rewrite when rollout 100%");
    assert!(rewritten.ends_with("git2-rs.git"));
    assert!(!rewritten.ends_with(".git.git"));
}

#[test]
fn test_rollout_sampling_clamps_percent() {
    let mut cfg = AppConfig::default();
    cfg.http.fake_sni_enabled = true;
    cfg.http.fake_sni_rollout_percent = 200; // clamp to 100
    cfg.tls.san_whitelist = vec!["github.com".into()];
    let url = "https://github.com/rust-lang/git2-rs";
    let out = decision_with_proxy(&cfg, url, false);
    assert!(out.sampled, "percent above 100 should behave like 100");
    assert!(out.rewritten.is_some());
}
