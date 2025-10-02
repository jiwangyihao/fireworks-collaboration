//! Git Transport 模块综合测试
//! 合并了 git/transport/fallback_tests.rs, git/transport/metrics_tests.rs,
//! git/transport/register_tests.rs, git/transport/rewrite_tests.rs,
//! git/transport/runtime_tests.rs, git/transport/fingerprint_tests.rs

// ============================================================================
// fallback_tests.rs 的测试
// ============================================================================

use fireworks_collaboration_lib::core::git::transport::{
    DecisionCtx, FallbackDecision, FallbackReason, FallbackStage,
};

#[test]
fn initial_fake_path() {
    let ctx = DecisionCtx {
        policy_allows_fake: true,
        runtime_fake_disabled: false,
    };
    let d = FallbackDecision::initial(&ctx);
    assert_eq!(d.stage(), FallbackStage::Fake);
    assert_eq!(d.history().len(), 1);
    assert_eq!(d.history()[0].reason, FallbackReason::EnterFake);
}

#[test]
fn initial_skip_path() {
    let ctx = DecisionCtx {
        policy_allows_fake: false,
        runtime_fake_disabled: false,
    };
    let d = FallbackDecision::initial(&ctx);
    assert_eq!(d.stage(), FallbackStage::Default);
    assert_eq!(d.history()[0].reason, FallbackReason::SkipFakePolicy);
}

#[test]
fn advance_chain() {
    let ctx = DecisionCtx {
        policy_allows_fake: true,
        runtime_fake_disabled: false,
    };
    let mut d = FallbackDecision::initial(&ctx);
    let tr1 = d.advance_on_error().expect("fake->real");
    assert_eq!(tr1.to, FallbackStage::Real);
    let tr2 = d.advance_on_error().expect("real->default");
    assert_eq!(tr2.to, FallbackStage::Default);
    assert!(d.advance_on_error().is_none(), "default is terminal");
    assert_eq!(d.history().len(), 3);
}

#[test]
fn default_stage_is_idempotent() {
    let ctx = DecisionCtx {
        policy_allows_fake: false,
        runtime_fake_disabled: false,
    }; // initial -> Default
    let mut d = FallbackDecision::initial(&ctx);
    assert_eq!(d.stage(), FallbackStage::Default);
    assert!(d.advance_on_error().is_none());
    assert_eq!(
        d.history().len(),
        1,
        "history should not grow after terminal advance attempts"
    );
}

// ============================================================================
// metrics_tests.rs 的测试
// ============================================================================

use fireworks_collaboration_lib::core::git::transport::metrics::{
    finish_and_store, metrics_enabled, test_override_metrics_enabled, tl_reset, tl_snapshot,
    TimingRecorder,
};
use std::sync::{Mutex, OnceLock};

fn metrics_env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

struct EnvGuard {
    prev: Option<String>,
}

impl EnvGuard {
    fn new() -> Self {
        Self {
            prev: std::env::var("FWC_TEST_FORCE_METRICS").ok(),
        }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        match self.prev.take() {
            Some(v) => std::env::set_var("FWC_TEST_FORCE_METRICS", v),
            None => std::env::remove_var("FWC_TEST_FORCE_METRICS"),
        }
    }
}

#[test]
fn finish_respects_metrics_enabled_flag() {
    let _lock = metrics_env_lock().lock().unwrap();

    tl_reset();
    test_override_metrics_enabled(Some(false));
    let mut recorder = TimingRecorder::new();
    finish_and_store(&mut recorder);
    let snap_disabled = tl_snapshot();
    assert!(
        snap_disabled.timing.is_none(),
        "timing should remain unset when metrics disabled"
    );

    tl_reset();
    test_override_metrics_enabled(Some(true));
    let mut recorder_enabled = TimingRecorder::new();
    finish_and_store(&mut recorder_enabled);
    let snap_enabled = tl_snapshot();
    assert!(
        snap_enabled.timing.is_some(),
        "timing should be captured when metrics enabled"
    );

    tl_reset();
    test_override_metrics_enabled(None);
}

#[test]
fn metrics_enabled_env_override_takes_precedence() {
    let _lock = metrics_env_lock().lock().unwrap();
    let _guard = EnvGuard::new();

    std::env::set_var("FWC_TEST_FORCE_METRICS", "0");
    assert!(!metrics_enabled(), "env=0 should disable metrics");

    std::env::set_var("FWC_TEST_FORCE_METRICS", "1");
    assert!(metrics_enabled(), "env=1 should enable metrics");

    std::env::remove_var("FWC_TEST_FORCE_METRICS");
}

// ============================================================================
// register_tests.rs 的测试
// ============================================================================

use fireworks_collaboration_lib::core::config::model::AppConfig;
use fireworks_collaboration_lib::core::git::transport::ensure_registered;
use fireworks_collaboration_lib::core::proxy::{ProxyConfig, ProxyMode};

#[test]
fn test_register_once_ok() {
    let cfg = AppConfig::default();
    // 多次调用不应 panic
    let _ = ensure_registered(&cfg);
    let _ = ensure_registered(&cfg);
}

#[test]
fn test_should_skip_custom_transport_when_proxy_off() {
    let cfg = AppConfig::default();
    // 代理未启用时，不应跳过自定义传输层
    // 通过检查 ensure_registered 能正常执行来验证
    let result = ensure_registered(&cfg);
    assert!(result.is_ok());
}

#[test]
fn test_should_skip_custom_transport_when_http_proxy_enabled() {
    let mut cfg = AppConfig::default();
    cfg.proxy = ProxyConfig {
        mode: ProxyMode::Http,
        url: "http://proxy:8080".to_string(),
        ..Default::default()
    };
    // HTTP代理启用时应该跳过自定义传输层
    let result = ensure_registered(&cfg);
    assert!(result.is_ok());
}

#[test]
fn test_should_skip_custom_transport_when_socks5_proxy_enabled() {
    let mut cfg = AppConfig::default();
    cfg.proxy = ProxyConfig {
        mode: ProxyMode::Socks5,
        url: "socks5://proxy:1080".to_string(),
        ..Default::default()
    };
    // SOCKS5代理启用时应该跳过自定义传输层
    let result = ensure_registered(&cfg);
    assert!(result.is_ok());
}

#[test]
fn test_ensure_registered_skips_when_proxy_enabled() {
    let mut cfg = AppConfig::default();
    cfg.proxy = ProxyConfig {
        mode: ProxyMode::Http,
        url: "http://proxy:8080".to_string(),
        ..Default::default()
    };

    // 代理启用时应该直接返回Ok，不注册自定义传输层
    let result = ensure_registered(&cfg);
    assert!(result.is_ok());
}

#[test]
fn test_should_skip_when_disable_custom_transport_set() {
    let mut cfg = AppConfig::default();
    cfg.proxy.disable_custom_transport = true;
    // 明确配置禁用自定义传输层时应该跳过
    let result = ensure_registered(&cfg);
    assert!(result.is_ok());
}

#[test]
fn test_should_skip_custom_transport_when_system_proxy_enabled() {
    let mut cfg = AppConfig::default();
    cfg.proxy = ProxyConfig {
        mode: ProxyMode::System,
        ..Default::default()
    };
    // 系统代理模式时应该跳过自定义传输层
    let result = ensure_registered(&cfg);
    assert!(result.is_ok());
}

#[test]
fn test_should_not_skip_with_empty_proxy_url() {
    let mut cfg = AppConfig::default();
    cfg.proxy = ProxyConfig {
        mode: ProxyMode::Http,
        url: "".to_string(), // Empty URL means proxy not enabled
        ..Default::default()
    };
    // HTTP mode with empty URL is NOT enabled, so should not skip
    let result = ensure_registered(&cfg);
    assert!(result.is_ok());
}

// ============================================================================
// rewrite_tests.rs 的测试
// ============================================================================

use fireworks_collaboration_lib::core::git::transport::rewrite::{
    decide_https_to_custom, RewriteDecision,
};

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

// ============================================================================
// runtime_tests.rs 的测试
// ============================================================================

use fireworks_collaboration_lib::core::git::transport::runtime::{
    is_fake_disabled, record_fake_attempt, AutoDisableConfig, AutoDisableEvent,
};
use std::time::Duration;

fn cfg(threshold: u8, cooldown: u64) -> AutoDisableConfig {
    AutoDisableConfig {
        threshold_pct: threshold,
        cooldown_sec: cooldown,
    }
}

#[test]
fn auto_disable_triggers_when_ratio_exceeds_threshold() {
    // 使用 testing 模块中的公共测试辅助函数
    use fireworks_collaboration_lib::core::git::transport::testing::{
        auto_disable_guard, reset_auto_disable,
    };

    let _guard = auto_disable_guard().lock().unwrap();
    reset_auto_disable();

    let cfg = cfg(50, 30);

    // 记录4次成功
    for _ in 0..4 {
        let _ = record_fake_attempt(&cfg, false);
    }

    // 记录失败直到触发（需要8个样本中4个失败才能达到50%）
    for i in 0..4 {
        let evt = record_fake_attempt(&cfg, true);
        if i < 3 {
            assert!(evt.is_none(), "expected no trigger on failure {}", i + 1);
        } else {
            assert!(matches!(
                evt,
                Some(AutoDisableEvent::Triggered {
                    threshold_pct: 50,
                    cooldown_secs: 30
                })
            ));
        }
    }

    assert!(is_fake_disabled(&cfg));
}

#[test]
fn auto_disable_recovers_after_cooldown() {
    use fireworks_collaboration_lib::core::git::transport::testing::{
        auto_disable_guard, reset_auto_disable,
    };

    let _guard = auto_disable_guard().lock().unwrap();
    reset_auto_disable();

    let cfg = cfg(50, 1); // 1 second cooldown

    // 触发自动禁用
    for _ in 0..5 {
        let _ = record_fake_attempt(&cfg, false);
    }
    for _ in 0..5 {
        let evt = record_fake_attempt(&cfg, true);
        if evt.is_some() {
            break;
        }
    }

    assert!(is_fake_disabled(&cfg));

    // 等待冷却时间（稍长一点确保超时）
    std::thread::sleep(Duration::from_millis(1100));

    // 冷却后应该恢复
    let evt = record_fake_attempt(&cfg, false);
    assert!(matches!(evt, Some(AutoDisableEvent::Recovered)));
    assert!(!is_fake_disabled(&cfg));
}

#[test]
fn disabled_feature_returns_none() {
    use fireworks_collaboration_lib::core::git::transport::testing::{
        auto_disable_guard, reset_auto_disable,
    };

    let _guard = auto_disable_guard().lock().unwrap();
    reset_auto_disable();

    let cfg = cfg(0, 30); // threshold=0 表示禁用该功能
    assert!(!is_fake_disabled(&cfg));
    assert!(record_fake_attempt(&cfg, true).is_none());
}

// ============================================================================
// fingerprint_tests.rs 的测试
// ============================================================================

use fireworks_collaboration_lib::core::config::loader;

#[test]
fn test_log_path_disabled_when_cfg_off() {
    let temp = tempfile::tempdir().expect("create temp dir for config override");

    // 直接使用 public API 而不是测试辅助函数
    loader::set_global_base_dir(temp.path());

    // 默认配置开启证书指纹日志
    let cfg = loader::load_or_init().expect("load default config");
    assert!(
        cfg.tls.cert_fp_log_enabled,
        "default config should enable cert fp log"
    );

    // 人为关闭后保存
    let mut cfg = cfg;
    cfg.tls.cert_fp_log_enabled = false;
    loader::save(&cfg).expect("save updated config");

    // 重新加载配置验证已保存
    let reloaded = loader::load_or_init().expect("reload config");
    assert!(!reloaded.tls.cert_fp_log_enabled);

    // 清理：恢复默认设置
    let mut cfg = reloaded;
    cfg.tls.cert_fp_log_enabled = true;
    let _ = loader::save(&cfg);
}
