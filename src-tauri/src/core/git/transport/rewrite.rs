use sha1::{Digest, Sha1};
use std::sync::atomic::{AtomicU64, Ordering};
use url::Url;

// 简单内存指标（P3.1 占位）：记录 rollout 命中与跳过统计。
static ROLLOUT_HIT: AtomicU64 = AtomicU64::new(0);
static ROLLOUT_MISS: AtomicU64 = AtomicU64::new(0);

use crate::core::config::model::AppConfig;
use crate::core::tls::util::{builtin_fake_sni_targets, match_domain, proxy_present};

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
    // 仅对允许伪装 SNI 的域名进行改写
    let allowed = builtin_fake_sni_targets()
        .iter()
        .any(|p| match_domain(p, host));
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
    let query = parsed.query().map(|q| format!("?{q}")).unwrap_or_default();
    let fragment = parsed
        .fragment()
        .map(|f| format!("#{f}"))
        .unwrap_or_default();
    let authority = parsed.authority();
    decision.sampled = true;
    decision.rewritten = Some(format!("https+custom://{authority}{path}{query}{fragment}"));
    decision
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::model::AppConfig;

    #[test]
    fn test_rewrite_basic_flow() {
        let mut cfg = AppConfig::default();
        cfg.http.fake_sni_enabled = true;
        cfg.http.fake_sni_rollout_percent = 100;

        // Whitelisted target (analytics.githubassets.com is usually in BUILTIN_IPS)
        let url = "https://analytics.githubassets.com/foo";
        let decision = decide_https_to_custom_inner(&cfg, url, false);

        assert!(decision.eligible);
        assert!(decision.sampled);
        assert_eq!(
            decision.rewritten,
            Some("https+custom://analytics.githubassets.com/foo.git".to_string())
        );
    }

    #[test]
    fn test_rewrite_disabled_by_config() {
        let mut cfg = AppConfig::default();
        cfg.http.fake_sni_enabled = false;

        let url = "https://analytics.githubassets.com/foo";
        let decision = decide_https_to_custom_inner(&cfg, url, false);

        assert!(!decision.eligible);
        assert!(decision.rewritten.is_none());
    }

    #[test]
    fn test_rewrite_skipped_if_proxy_present() {
        let mut cfg = AppConfig::default();
        cfg.http.fake_sni_enabled = true;

        let url = "https://analytics.githubassets.com/foo";
        let decision = decide_https_to_custom_inner(&cfg, url, true);

        assert!(!decision.eligible);
        assert!(decision.rewritten.is_none());
    }

    #[test]
    fn test_rewrite_skipped_if_not_whitelisted() {
        let mut cfg = AppConfig::default();
        cfg.http.fake_sni_enabled = true;

        let url = "https://example.com/repo.git";
        let decision = decide_https_to_custom_inner(&cfg, url, false);

        assert!(!decision.eligible);
        assert!(decision.rewritten.is_none());
    }

    #[test]
    fn test_rewrite_rollout_sampling() {
        let mut cfg = AppConfig::default();
        cfg.http.fake_sni_enabled = true;

        let url = "https://analytics.githubassets.com/foo";

        // 0% rollout
        cfg.http.fake_sni_rollout_percent = 0;
        let d0 = decide_https_to_custom_inner(&cfg, url, false);
        assert!(d0.eligible);
        assert!(!d0.sampled);

        // 100% rollout
        cfg.http.fake_sni_rollout_percent = 100;
        let d100 = decide_https_to_custom_inner(&cfg, url, false);
        assert!(d100.sampled);
    }

    #[test]
    fn test_rewrite_path_normalization() {
        let mut cfg = AppConfig::default();
        cfg.http.fake_sni_enabled = true;
        cfg.http.fake_sni_rollout_percent = 100;

        let url = "https://analytics.githubassets.com/repo.git";
        let res = decide_https_to_custom_inner(&cfg, url, false);
        assert_eq!(
            res.rewritten,
            Some("https+custom://analytics.githubassets.com/repo.git".to_string())
        );

        let url2 = "https://analytics.githubassets.com/repo";
        let res2 = decide_https_to_custom_inner(&cfg, url2, false);
        assert_eq!(
            res2.rewritten,
            Some("https+custom://analytics.githubassets.com/repo.git".to_string())
        );
    }
}
