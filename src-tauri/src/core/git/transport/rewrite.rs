use sha1::{Digest, Sha1};
use std::sync::atomic::{AtomicU64, Ordering};
use url::Url;

// 简单内存指标（P3.1 占位）：记录 rollout 命中与跳过统计。
static ROLLOUT_HIT: AtomicU64 = AtomicU64::new(0);
static ROLLOUT_MISS: AtomicU64 = AtomicU64::new(0);

use crate::core::config::model::AppConfig;
use crate::core::tls::util::{match_domain, proxy_present};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RewriteDecision {
    pub rewritten: Option<String>,
    pub sampled: bool,
    pub eligible: bool,
}

/// 返回是否改写以及此次请求是否命中 rollout 采样（eligible 表示已进入采样阶段）。
pub fn decide_https_to_custom(cfg: &AppConfig, url: &str) -> RewriteDecision {
    decide_https_to_custom_inner(cfg, url, proxy_present())
}

/// 若启用灰度且命中白名单，将 https:// 重写为 https+custom://
pub fn maybe_rewrite_https_to_custom(cfg: &AppConfig, url: &str) -> Option<String> {
    decide_https_to_custom(cfg, url).rewritten
}

fn decide_https_to_custom_inner(
    cfg: &AppConfig,
    url: &str,
    proxy_present: bool,
) -> RewriteDecision {
    let mut decision = RewriteDecision::default();
    if !cfg.http.fake_sni_enabled {
        return decision;
    }
    if proxy_present {
        return decision;
    }
    let parsed = match Url::parse(url) {
        Ok(v) => v,
        Err(_) => return decision,
    };
    if parsed.scheme() != "https" {
        return decision;
    }
    let host = match parsed.host_str() {
        Some(h) => h,
        None => return decision,
    };
    // 命中主白名单或附加白名单才继续
    let mut allowed = cfg.tls.san_whitelist.iter().any(|p| match_domain(p, host));
    if !allowed {
        allowed = cfg
            .http
            .host_allow_list_extra
            .iter()
            .any(|p| match_domain(p, host));
    }
    if !allowed {
        return decision;
    }
    decision.eligible = true;

    // P3.1 rollout 采样：对 host 做稳定哈希，取 0..=99 区间，与 percent 比较
    let percent = cfg.http.fake_sni_rollout_percent.min(100);
    if percent == 0 {
        ROLLOUT_MISS.fetch_add(1, Ordering::Relaxed);
        return decision;
    }
    if percent < 100 {
        let mut hasher = Sha1::new();
        hasher.update(host.as_bytes());
        let digest = hasher.finalize();
        let bucket = (u16::from(digest[0]) << 8 | u16::from(digest[1])) % 100; // 0..99
        if bucket as u8 >= percent {
            ROLLOUT_MISS.fetch_add(1, Ordering::Relaxed);
            return decision;
        }
    }
    ROLLOUT_HIT.fetch_add(1, Ordering::Relaxed);

    // 确保路径以 .git 结尾（Git 仓库标准）
    let mut path = parsed.path().to_string();
    if !path.ends_with(".git") {
        path.push_str(".git");
    }
    // 重构 URL 字符串：scheme 改为 https+custom，path 更新
    let query = parsed
        .query()
        .map(|q| format!("?{}", q))
        .unwrap_or_default();
    let fragment = parsed
        .fragment()
        .map(|f| format!("#{}", f))
        .unwrap_or_default();
    let authority = parsed.authority();
    decision.sampled = true;
    decision.rewritten = Some(format!(
        "https+custom://{}{}{}{}",
        authority, path, query, fragment
    ));
    decision
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decision(cfg: &AppConfig, url: &str, proxy_present: bool) -> RewriteDecision {
        decide_https_to_custom_inner(cfg, url, proxy_present)
    }

    #[test]
    fn test_rewrite_only_when_enabled_and_whitelisted() {
        let mut cfg = AppConfig::default();
        cfg.http.fake_sni_enabled = true;
        cfg.http.fake_sni_rollout_percent = 100; // 全量
        cfg.tls.san_whitelist = vec!["github.com".into()];
        let url = "https://github.com/rust-lang/git2-rs";
        let out = decision(&cfg, url, false);
        assert_eq!(
            out.rewritten.as_deref(),
            Some("https+custom://github.com/rust-lang/git2-rs.git")
        );
        assert!(out.sampled);
        assert!(out.eligible);
        assert!(
            super::ROLLOUT_HIT.load(Ordering::Relaxed) > 0,
            "hit counter should increment"
        );

        // 非 https 不改写
        assert!(decision(&cfg, "http://github.com/", false)
            .rewritten
            .is_none());

        // 关闭开关不改写
        cfg.http.fake_sni_enabled = false;
        let disabled = decision(&cfg, url, false);
        assert!(disabled.rewritten.is_none());
        assert!(!disabled.eligible);

        // 非白名单域不改写
        let mut cfg2 = AppConfig::default();
        cfg2.http.fake_sni_enabled = true;
        cfg2.tls.san_whitelist = vec!["example.com".into()];
        assert!(decision(&cfg2, url, false).rewritten.is_none());
    }

    #[test]
    fn test_rewrite_disabled_when_proxy_env_present() {
        let mut cfg = AppConfig::default();
        cfg.http.fake_sni_enabled = true;
        let url = "https://github.com/owner/repo";
        // 指定存在代理 -> 不改写
        let out = decision(&cfg, url, true);
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
        let out = decision(&cfg, url, false);
        assert!(out.rewritten.is_none());
        assert!(out.eligible);
        assert!(!out.sampled);
        assert!(ROLLOUT_MISS.load(Ordering::Relaxed) > 0);
    }

    #[test]
    fn test_rollout_sampling_partial_deterministic() {
        let mut cfg = AppConfig::default();
        cfg.http.fake_sni_enabled = true;
        cfg.http.fake_sni_rollout_percent = 10; // 10%
        cfg.tls.san_whitelist = vec!["github.com".into()];
        let url = "https://github.com/rust-lang/git2-rs";
        // 结果稳定（要么始终改写，要么始终不改写）
        let first = decision(&cfg, url, false).sampled;
        for _ in 0..10 {
            assert_eq!(first, decision(&cfg, url, false).sampled);
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
        let out = decision(&cfg, url, false);
        assert!(out.sampled);
        assert!(out.rewritten.is_some());
    }

    #[test]
    fn test_rewrite_preserves_query_and_fragment() {
        let mut cfg = AppConfig::default();
        cfg.http.fake_sni_enabled = true;
        cfg.http.fake_sni_rollout_percent = 100;
        let url = "https://github.com/rust-lang/git2-rs?foo=bar#frag";
        let out = decision(&cfg, url, false);
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
        let url = "https://github.com/rust-lang/git2-rs.git";
        let out = decision(&cfg, url, false);
        let rewritten = out.rewritten.expect("expected rewrite when rollout 100%");
        assert!(rewritten.ends_with("git2-rs.git"));
        assert!(!rewritten.ends_with(".git.git"));
    }

    #[test]
    fn test_rollout_sampling_clamps_percent() {
        let mut cfg = AppConfig::default();
        cfg.http.fake_sni_enabled = true;
        cfg.http.fake_sni_rollout_percent = 200; // clamp to 100
        let url = "https://github.com/rust-lang/git2-rs";
        let out = decision(&cfg, url, false);
        assert!(out.sampled, "percent above 100 should behave like 100");
        assert!(out.rewritten.is_some());
    }
}
