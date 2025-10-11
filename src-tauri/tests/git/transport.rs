//! Git Transport 模块综合测试
//! 合并了 `git/transport/fallback_tests.rs`, `git/transport/metrics_tests.rs`,
//! `git/transport/register_tests.rs`, `git/transport/rewrite_tests.rs`,
//! `git/transport/runtime_tests.rs`, `git/transport/fingerprint_tests.rs`

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
    cfg.http.fake_sni_target_hosts = vec!["github.com".into()];
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
    cfg2.http.fake_sni_target_hosts = vec!["example.com".into()];
    assert!(decision_with_proxy(&cfg2, url, false).rewritten.is_none());
}

#[test]
fn test_rewrite_disabled_when_proxy_env_present() {
    let mut cfg = AppConfig::default();
    cfg.http.fake_sni_enabled = true;
    cfg.http.fake_sni_target_hosts = vec!["github.com".into()];
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
    cfg.http.fake_sni_target_hosts = vec!["github.com".into()];
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
    cfg.http.fake_sni_target_hosts = vec!["github.com".into()];
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
    cfg.http.fake_sni_target_hosts = vec!["example.com".into()];
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
    cfg.http.fake_sni_target_hosts = vec!["github.com".into()];
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
    cfg.http.fake_sni_target_hosts = vec!["github.com".into()];
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
    cfg.http.fake_sni_target_hosts = vec!["github.com".into()];
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

// ============================================================================
// P5.3 Proxy-Transport Integration Tests (from proxy_transport_integration.rs)
// ============================================================================

#[test]
fn test_transport_skipped_when_http_proxy_enabled() {
    use fireworks_collaboration_lib::core::config::model::AppConfig;
    use fireworks_collaboration_lib::core::git::transport::ensure_registered;
    use fireworks_collaboration_lib::core::proxy::{ProxyConfig, ProxyMode};

    let mut cfg = AppConfig::default();
    cfg.proxy = ProxyConfig {
        mode: ProxyMode::Http,
        url: "http://proxy.example.com:8080".to_string(),
        ..Default::default()
    };

    // Should succeed without registering custom transport
    let result = ensure_registered(&cfg);
    assert!(
        result.is_ok(),
        "ensure_registered should succeed when proxy enabled"
    );
}

#[test]
fn test_transport_skipped_when_socks5_proxy_enabled() {
    use fireworks_collaboration_lib::core::config::model::AppConfig;
    use fireworks_collaboration_lib::core::git::transport::ensure_registered;
    use fireworks_collaboration_lib::core::proxy::{ProxyConfig, ProxyMode};

    let mut cfg = AppConfig::default();
    cfg.proxy = ProxyConfig {
        mode: ProxyMode::Socks5,
        url: "socks5://proxy.example.com:1080".to_string(),
        ..Default::default()
    };

    // Should succeed without registering custom transport
    let result = ensure_registered(&cfg);
    assert!(
        result.is_ok(),
        "ensure_registered should succeed when proxy enabled"
    );
}

#[test]
fn test_transport_registered_when_proxy_off() {
    use fireworks_collaboration_lib::core::config::model::AppConfig;
    use fireworks_collaboration_lib::core::git::transport::ensure_registered;

    let cfg = AppConfig::default();

    // Should register custom transport
    let result = ensure_registered(&cfg);
    assert!(
        result.is_ok(),
        "ensure_registered should succeed when proxy off"
    );
}

#[test]
fn test_transport_skipped_when_disable_custom_transport_set() {
    use fireworks_collaboration_lib::core::config::model::AppConfig;
    use fireworks_collaboration_lib::core::git::transport::ensure_registered;

    let mut cfg = AppConfig::default();
    cfg.proxy.disable_custom_transport = true;

    // Should skip even if proxy is off
    let result = ensure_registered(&cfg);
    assert!(
        result.is_ok(),
        "ensure_registered should succeed with disable_custom_transport"
    );
}

#[test]
fn test_proxy_forces_disable_custom_transport() {
    use fireworks_collaboration_lib::core::proxy::{ProxyConfig, ProxyManager, ProxyMode};

    // HTTP proxy
    let cfg_http = ProxyConfig {
        mode: ProxyMode::Http,
        url: "http://proxy:8080".to_string(),
        disable_custom_transport: false, // Explicitly set to false
        ..Default::default()
    };
    let manager_http = ProxyManager::new(cfg_http);
    assert!(
        manager_http.should_disable_custom_transport(),
        "HTTP proxy should force disable custom transport"
    );

    // SOCKS5 proxy
    let cfg_socks5 = ProxyConfig {
        mode: ProxyMode::Socks5,
        url: "socks5://proxy:1080".to_string(),
        disable_custom_transport: false, // Explicitly set to false
        ..Default::default()
    };
    let manager_socks5 = ProxyManager::new(cfg_socks5);
    assert!(
        manager_socks5.should_disable_custom_transport(),
        "SOCKS5 proxy should force disable custom transport"
    );

    // Proxy off
    let cfg_off = ProxyConfig {
        mode: ProxyMode::Off,
        disable_custom_transport: false,
        ..Default::default()
    };
    let manager_off = ProxyManager::new(cfg_off);
    assert!(
        !manager_off.should_disable_custom_transport(),
        "Proxy off should not disable custom transport"
    );
}

#[test]
fn test_proxy_mode_transitions() {
    use fireworks_collaboration_lib::core::proxy::{ProxyConfig, ProxyManager, ProxyMode};

    // Start with proxy off
    let cfg_off = ProxyConfig {
        mode: ProxyMode::Off,
        ..Default::default()
    };
    let manager = ProxyManager::new(cfg_off);
    assert!(!manager.should_disable_custom_transport());

    // Enable HTTP proxy
    let cfg_http = ProxyConfig {
        mode: ProxyMode::Http,
        url: "http://proxy:8080".to_string(),
        ..Default::default()
    };
    let manager_http = ProxyManager::new(cfg_http);
    assert!(manager_http.should_disable_custom_transport());

    // Switch to SOCKS5
    let cfg_socks5 = ProxyConfig {
        mode: ProxyMode::Socks5,
        url: "socks5://proxy:1080".to_string(),
        ..Default::default()
    };
    let manager_socks5 = ProxyManager::new(cfg_socks5);
    assert!(manager_socks5.should_disable_custom_transport());

    // Disable proxy
    let cfg_off_again = ProxyConfig {
        mode: ProxyMode::Off,
        ..Default::default()
    };
    let manager_off = ProxyManager::new(cfg_off_again);
    assert!(!manager_off.should_disable_custom_transport());
}

#[test]
fn test_explicit_disable_custom_transport() {
    use fireworks_collaboration_lib::core::proxy::{ProxyConfig, ProxyManager, ProxyMode};

    // Explicit disable with proxy off
    let cfg = ProxyConfig {
        mode: ProxyMode::Off,
        disable_custom_transport: true,
        ..Default::default()
    };
    let manager = ProxyManager::new(cfg);
    assert!(
        manager.should_disable_custom_transport(),
        "Explicit disable_custom_transport should work even when proxy off"
    );
}

#[test]
fn test_transport_skipped_when_system_proxy_enabled() {
    use fireworks_collaboration_lib::core::config::model::AppConfig;
    use fireworks_collaboration_lib::core::git::transport::ensure_registered;
    use fireworks_collaboration_lib::core::proxy::{ProxyConfig, ProxyMode};

    let mut cfg = AppConfig::default();
    cfg.proxy = ProxyConfig {
        mode: ProxyMode::System,
        ..Default::default()
    };

    // System proxy should also skip custom transport
    let result = ensure_registered(&cfg);
    assert!(
        result.is_ok(),
        "ensure_registered should succeed with System proxy"
    );
}

#[test]
fn test_system_proxy_forces_disable_custom_transport() {
    use fireworks_collaboration_lib::core::proxy::{ProxyConfig, ProxyManager, ProxyMode};

    let cfg_system = ProxyConfig {
        mode: ProxyMode::System,
        disable_custom_transport: false,
        ..Default::default()
    };
    let manager_system = ProxyManager::new(cfg_system);
    assert!(
        manager_system.should_disable_custom_transport(),
        "System proxy should force disable custom transport"
    );
}

#[test]
fn test_metrics_data_flow_with_proxy() {
    use fireworks_collaboration_lib::core::git::transport::metrics::{
        tl_reset, tl_set_proxy_usage, tl_snapshot,
    };

    // Reset to clean state
    tl_reset();

    // Simulate proxy usage recording
    tl_set_proxy_usage(true, Some("http".to_string()), Some(50), true);

    // Capture snapshot
    let snapshot = tl_snapshot();

    // Verify all proxy fields are captured
    assert_eq!(snapshot.used_proxy, Some(true), "used_proxy should be true");
    assert_eq!(
        snapshot.proxy_type,
        Some("http".to_string()),
        "proxy_type should be 'http'"
    );
    assert_eq!(
        snapshot.proxy_latency_ms,
        Some(50),
        "proxy_latency_ms should be 50"
    );
    assert_eq!(
        snapshot.custom_transport_disabled,
        Some(true),
        "custom_transport_disabled should be true"
    );

    // Reset and verify clean state
    tl_reset();
    let snapshot_after_reset = tl_snapshot();
    assert_eq!(
        snapshot_after_reset.used_proxy, None,
        "used_proxy should be None after reset"
    );
    assert_eq!(
        snapshot_after_reset.proxy_type, None,
        "proxy_type should be None after reset"
    );
    assert_eq!(
        snapshot_after_reset.proxy_latency_ms, None,
        "proxy_latency_ms should be None after reset"
    );
    assert_eq!(
        snapshot_after_reset.custom_transport_disabled, None,
        "custom_transport_disabled should be None after reset"
    );
}

#[test]
fn test_metrics_data_flow_without_proxy() {
    use fireworks_collaboration_lib::core::git::transport::metrics::{
        tl_reset, tl_set_proxy_usage, tl_snapshot,
    };

    // Reset to clean state
    tl_reset();

    // Simulate no proxy usage
    tl_set_proxy_usage(false, None, None, false);

    // Capture snapshot
    let snapshot = tl_snapshot();

    // Verify proxy fields reflect no usage
    assert_eq!(
        snapshot.used_proxy,
        Some(false),
        "used_proxy should be false"
    );
    assert_eq!(snapshot.proxy_type, None, "proxy_type should be None");
    assert_eq!(
        snapshot.proxy_latency_ms, None,
        "proxy_latency_ms should be None"
    );
    assert_eq!(
        snapshot.custom_transport_disabled,
        Some(false),
        "custom_transport_disabled should be false"
    );
}

#[test]
fn test_empty_proxy_url_behavior() {
    use fireworks_collaboration_lib::core::config::model::AppConfig;
    use fireworks_collaboration_lib::core::git::transport::ensure_registered;
    use fireworks_collaboration_lib::core::proxy::{ProxyConfig, ProxyManager, ProxyMode};

    let mut cfg = AppConfig::default();
    cfg.proxy = ProxyConfig {
        mode: ProxyMode::Http,
        url: "".to_string(),
        ..Default::default()
    };

    // Empty URL with HTTP mode means proxy NOT enabled
    let result = ensure_registered(&cfg);
    assert!(
        result.is_ok(),
        "ensure_registered should handle empty proxy URL gracefully"
    );

    // ProxyManager should report as not enabled
    let manager = ProxyManager::new(cfg.proxy.clone());
    assert!(
        !manager.is_enabled(),
        "Empty URL should mean proxy is not enabled"
    );
}

#[test]
fn test_concurrent_registration_safety() {
    use fireworks_collaboration_lib::core::config::model::AppConfig;
    use fireworks_collaboration_lib::core::git::transport::ensure_registered;
    use std::sync::Arc;
    use std::thread;

    let cfg = Arc::new(AppConfig::default());

    // Spawn multiple threads trying to register simultaneously
    let handles: Vec<_> = (0..10)
        .map(|_| {
            let cfg_clone = Arc::clone(&cfg);
            thread::spawn(move || ensure_registered(&cfg_clone))
        })
        .collect();

    // All should succeed
    for handle in handles {
        let result = handle.join().unwrap();
        assert!(result.is_ok(), "Concurrent registration should be safe");
    }
}

// ============================================================================
// HTTP Module Tests (from http.rs)
// ============================================================================

#[tokio::test]
async fn test_reject_non_https() {
    use fireworks_collaboration_lib::core::config::model::AppConfig;
    use fireworks_collaboration_lib::core::http::client::HttpClient;
    use fireworks_collaboration_lib::core::http::types::HttpRequestInput;
    use std::collections::HashMap;

    let client = HttpClient::new(AppConfig::default());
    let input = HttpRequestInput {
        url: "http://example.com/".into(),
        method: "GET".into(),
        headers: HashMap::new(),
        body_base64: None,
        timeout_ms: 100,
        force_real_sni: false,
        follow_redirects: false,
        max_redirects: 0,
    };
    let err = client.send(input).await.expect_err("should fail");
    let msg = format!("{err}");
    assert!(msg.contains("only https"));
}

#[tokio::test]
async fn test_invalid_base64_early() {
    use fireworks_collaboration_lib::core::config::model::AppConfig;
    use fireworks_collaboration_lib::core::http::client::HttpClient;
    use fireworks_collaboration_lib::core::http::types::HttpRequestInput;
    use std::collections::HashMap;

    let client = HttpClient::new(AppConfig::default());
    let input = HttpRequestInput {
        url: "https://example.com/".into(),
        method: "POST".into(),
        headers: HashMap::new(),
        body_base64: Some("***not-base64***".into()),
        timeout_ms: 100,
        force_real_sni: false,
        follow_redirects: false,
        max_redirects: 0,
    };
    let err = client.send(input).await.expect_err("should fail");
    let msg = format!("{err}");
    assert!(msg.contains("decode bodyBase64"));
}

#[test]
fn test_compute_sni_host_fake_and_real() {
    use fireworks_collaboration_lib::core::config::model::AppConfig;
    use fireworks_collaboration_lib::core::http::client::HttpClient;

    let mut cfg = AppConfig::default();
    cfg.http.fake_sni_enabled = true;
    cfg.http.fake_sni_hosts = vec!["baidu.com".into()];
    cfg.http.fake_sni_target_hosts = vec!["github.com".into()];
    let client = HttpClient::new(cfg.clone());
    let (sni, used_fake) = client.compute_sni_host(false, "github.com");
    assert_eq!(sni, "baidu.com");
    assert!(used_fake);
    let (sni2, used_fake2) = client.compute_sni_host(true, "github.com");
    assert_eq!(sni2, "github.com");
    assert!(!used_fake2);
}

#[test]
fn test_upsert_host_header_overrides() {
    use fireworks_collaboration_lib::core::config::model::AppConfig;
    use fireworks_collaboration_lib::core::http::client::HttpClient;
    use hyper::header::{HeaderMap, HOST};

    let client = HttpClient::new(AppConfig::default());
    let mut h = HeaderMap::new();
    client.upsert_host_header(&mut h, "example.com");
    assert_eq!(h.get(HOST).unwrap(), "example.com");
    // override
    client.upsert_host_header(&mut h, "another.test");
    assert_eq!(h.get(HOST).unwrap(), "another.test");
}

#[test]
fn test_should_warn_large_body_boundary() {
    use fireworks_collaboration_lib::core::config::model::AppConfig;
    use fireworks_collaboration_lib::core::http::client::HttpClient;

    let mut cfg = AppConfig::default();
    cfg.http.large_body_warn_bytes = 10;
    let client = HttpClient::new(cfg);
    assert!(!client.should_warn_large_body(10)); // equal -> no warn
    assert!(client.should_warn_large_body(11)); // greater -> warn
}

#[test]
fn test_roundtrip_serde() {
    use fireworks_collaboration_lib::core::http::types::{
        HttpResponseOutput, RedirectInfo, TimingInfo,
    };
    use std::collections::HashMap;

    let out = HttpResponseOutput {
        ok: true,
        status: 200,
        headers: HashMap::from([("content-type".into(), "text/plain".into())]),
        body_base64: "SGVsbG8=".into(),
        used_fake_sni: false,
        ip: Some("1.2.3.4".into()),
        timing: TimingInfo {
            connect_ms: 1,
            tls_ms: 2,
            first_byte_ms: 3,
            total_ms: 4,
        },
        redirects: vec![RedirectInfo {
            status: 301,
            location: "https://example.com".into(),
            count: 1,
        }],
        body_size: 5,
    };
    let s = serde_json::to_string(&out).unwrap();
    let back: HttpResponseOutput = serde_json::from_str(&s).unwrap();
    assert_eq!(back.status, 200);
    assert_eq!(back.timing.total_ms, 4);
    assert_eq!(back.redirects.len(), 1);
}
