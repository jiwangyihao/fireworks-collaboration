// 从 src/core/tls/util.rs 迁移的测试
use fireworks_collaboration_lib::core::{
    config::model::AppConfig,
    tls::util::{decide_sni_host_with_proxy, match_domain, set_last_good_sni, should_use_fake},
};

#[test]
fn test_should_use_fake() {
    let mut cfg = AppConfig::default();
    cfg.http.fake_sni_enabled = true;
    assert!(should_use_fake(&cfg, false));
    assert!(!should_use_fake(&cfg, true));
    cfg.http.fake_sni_enabled = false;
    assert!(!should_use_fake(&cfg, false));
}

#[test]
fn test_match_domain_exact_and_wildcard() {
    assert!(match_domain("github.com", "github.com"));
    assert!(!match_domain("github.com", "api.github.com"));
    assert!(match_domain("*.github.com", "api.github.com"));
    assert!(!match_domain("*.github.com", "github.com"));
    assert!(!match_domain("*.github.com", "x.ygithub.com"));
}

#[test]
fn test_match_domain_case_insensitive_and_multi_sub() {
    assert!(match_domain("GITHUB.COM", "github.com"));
    assert!(match_domain("*.GitHub.com", "API.GitHub.Com"));
    assert!(match_domain("*.github.com", "a.b.github.com"));
    assert!(!match_domain("*.*.github.com", "a.b.github.com"));
}

#[test]
fn test_decide_sni_host_with_proxy_and_candidates() {
    let mut cfg = AppConfig::default();
    cfg.http.fake_sni_enabled = true;
    cfg.http.fake_sni_hosts = vec!["a.com".into(), "b.com".into(), "c.com".into()];
    let (sni, used_fake) = decide_sni_host_with_proxy(&cfg, false, "github.com", false);
    assert!(used_fake);
    assert!(sni == "a.com" || sni == "b.com" || sni == "c.com");

    let (sni2, used2) = decide_sni_host_with_proxy(&cfg, false, "github.com", true);
    assert_eq!(sni2, "github.com");
    assert!(!used2);

    cfg.http.fake_sni_enabled = false;
    let (sni3, used3) = decide_sni_host_with_proxy(&cfg, false, "github.com", false);
    assert_eq!(sni3, "github.com");
    assert!(!used3);
}

#[test]
fn test_last_good_preferred_when_present() {
    let mut cfg = AppConfig::default();
    cfg.http.fake_sni_enabled = true;
    cfg.http.fake_sni_hosts = vec!["x.com".into(), "y.com".into()];
    set_last_good_sni("github.com", "y.com");
    let (sni, used_fake) = decide_sni_host_with_proxy(&cfg, false, "github.com", false);
    assert!(used_fake);
    assert_eq!(sni, "y.com");
}
