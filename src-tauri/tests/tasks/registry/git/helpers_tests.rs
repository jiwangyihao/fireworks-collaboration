use fireworks_collaboration_lib::core::config::model::{AppConfig, RetryCfg};
use fireworks_collaboration_lib::core::git::default_impl::opts::{
    StrategyHttpOverride, StrategyRetryOverride, StrategyTlsOverride,
};
use fireworks_collaboration_lib::core::tasks::registry::TaskRegistry;
use uuid::Uuid;

#[test]
fn no_http_override() {
    let global = AppConfig::default();
    let (f, m, changed, conflict) =
        TaskRegistry::apply_http_override("GitClone", &Uuid::nil(), &global, None);
    assert_eq!(f, global.http.follow_redirects);
    assert_eq!(m, global.http.max_redirects);
    assert!(!changed);
    assert!(conflict.is_none());
}

#[test]
fn http_override_changes() {
    let global = AppConfig::default();
    let over = StrategyHttpOverride {
        follow_redirects: Some(!global.http.follow_redirects),
        max_redirects: Some(3),
        ..Default::default()
    };
    let (f, m, changed, conflict) =
        TaskRegistry::apply_http_override("GitClone", &Uuid::nil(), &global, Some(&over));
    if !global.http.follow_redirects {
        assert_eq!(f, true);
    }
    if f == false {
        assert_eq!(m, 0);
        assert!(conflict.is_some());
    } else {
        assert_eq!(m, 3);
        assert!(conflict.is_none());
    }
    assert!(changed);
}

#[test]
fn http_override_clamp_applies() {
    let global = AppConfig::default();
    let over = StrategyHttpOverride {
        follow_redirects: None,
        max_redirects: Some(99),
        ..Default::default()
    };
    let (_f, m, changed, _conflict) =
        TaskRegistry::apply_http_override("GitClone", &Uuid::nil(), &global, Some(&over));
    assert_eq!(m, 20);
    assert!(changed);
}

#[test]
fn no_retry_override() {
    let global = RetryCfg::default();
    let (plan, changed) = TaskRegistry::apply_retry_override(&global, None);
    assert_eq!(plan.max, global.max);
    assert_eq!(plan.base_ms, global.base_ms);
    assert!(!changed);
}

#[test]
fn retry_override_changes() {
    let mut global = RetryCfg::default();
    global.max = 6;
    let over = StrategyRetryOverride {
        max: Some(3),
        base_ms: Some(500),
        factor: Some(2.0),
        jitter: Some(false),
    };
    let (plan, changed) = TaskRegistry::apply_retry_override(&global, Some(&over));
    assert!(changed);
    assert_eq!(plan.max, 3);
    assert_eq!(plan.base_ms, 500);
    assert!((plan.factor - 2.0).abs() < f64::EPSILON);
    assert!(!plan.jitter);
}

#[test]
fn no_tls_override() {
    let global = AppConfig::default();
    let (ins, skip, changed, conflict) =
        TaskRegistry::apply_tls_override("GitClone", &Uuid::nil(), &global, None);
    assert_eq!(ins, global.tls.insecure_skip_verify);
    assert_eq!(skip, global.tls.skip_san_whitelist);
    assert!(!changed);
    assert!(conflict.is_none());
}

#[test]
fn tls_override_insecure_only() {
    let global = AppConfig::default();
    let over = StrategyTlsOverride {
        insecure_skip_verify: Some(!global.tls.insecure_skip_verify),
        skip_san_whitelist: None,
    };
    let (ins, skip, changed, conflict) =
        TaskRegistry::apply_tls_override("GitClone", &Uuid::nil(), &global, Some(&over));
    assert_eq!(ins, !global.tls.insecure_skip_verify);
    assert_eq!(skip, global.tls.skip_san_whitelist);
    assert!(changed);
    assert!(conflict.is_none());
}

#[test]
fn tls_override_skip_san_only() {
    let global = AppConfig::default();
    let over = StrategyTlsOverride {
        insecure_skip_verify: None,
        skip_san_whitelist: Some(!global.tls.skip_san_whitelist),
    };
    let (ins, skip, changed, conflict) =
        TaskRegistry::apply_tls_override("GitClone", &Uuid::nil(), &global, Some(&over));
    assert_eq!(ins, global.tls.insecure_skip_verify);
    assert_eq!(skip, !global.tls.skip_san_whitelist);
    assert!(changed);
    assert!(conflict.is_none());
}

#[test]
fn tls_override_both_changed() {
    let mut global = AppConfig::default();
    global.tls.insecure_skip_verify = false;
    global.tls.skip_san_whitelist = false;
    let over = StrategyTlsOverride {
        insecure_skip_verify: Some(true),
        skip_san_whitelist: Some(true),
    };
    let (ins, skip, changed, conflict) =
        TaskRegistry::apply_tls_override("GitClone", &Uuid::nil(), &global, Some(&over));
    assert!(changed);
    assert!(ins);
    assert!(!skip);
    assert!(conflict.is_some());
}

#[test]
fn tls_global_config_not_mutated() {
    let global = AppConfig::default();
    let over = StrategyTlsOverride {
        insecure_skip_verify: Some(true),
        skip_san_whitelist: Some(true),
    };
    let _ = TaskRegistry::apply_tls_override("GitClone", &Uuid::nil(), &global, Some(&over));
    assert!(!global.tls.insecure_skip_verify);
    assert!(!global.tls.skip_san_whitelist);
}
