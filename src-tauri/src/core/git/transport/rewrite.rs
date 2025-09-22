use url::Url;
use sha1::{Digest, Sha1};
use std::sync::atomic::{AtomicU64, Ordering};

// 简单内存指标（P3.1 占位）：记录 rollout 命中与跳过统计。
static ROLLOUT_HIT: AtomicU64 = AtomicU64::new(0);
static ROLLOUT_MISS: AtomicU64 = AtomicU64::new(0);

use crate::core::config::model::AppConfig;
use crate::core::tls::util::{match_domain, proxy_present};

/// 若启用灰度且命中白名单，将 https:// 重写为 https+custom://
pub fn maybe_rewrite_https_to_custom(cfg: &AppConfig, url: &str) -> Option<String> {
    maybe_rewrite_https_to_custom_inner(cfg, url, proxy_present())
}

/// 纯函数：根据是否存在代理(proxy_present)决定是否进行改写，便于测试中指定环境。
fn maybe_rewrite_https_to_custom_inner(
    cfg: &AppConfig,
    url: &str,
    proxy_present: bool,
) -> Option<String> {
    // 仅处理 https://
    if !cfg.http.fake_sni_enabled {
        return None;
    }
    if proxy_present {
        return None;
    }
    let parsed = Url::parse(url).ok()?;
    if parsed.scheme() != "https" {
        return None;
    }
    let host = parsed.host_str()?;
    // 命中主白名单或附加白名单才继续
    let mut allowed = cfg
        .tls
        .san_whitelist
        .iter()
        .any(|p| match_domain(p, host));
    if !allowed {
        allowed = cfg.http.host_allow_list_extra.iter().any(|p| match_domain(p, host));
    }
    if !allowed { return None; }

    // P3.1 rollout 采样：对 host 做稳定哈希，取 0..=99 区间，与 percent 比较
    let percent = cfg.http.fake_sni_rollout_percent.min(100);
    if percent == 0 { ROLLOUT_MISS.fetch_add(1, Ordering::Relaxed); return None; }
    if percent < 100 {
        let mut hasher = Sha1::new();
        hasher.update(host.as_bytes());
        let digest = hasher.finalize();
        let bucket = (u16::from(digest[0]) << 8 | u16::from(digest[1])) % 100; // 0..99
        if bucket as u8 >= percent { ROLLOUT_MISS.fetch_add(1, Ordering::Relaxed); return None; }
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
    Some(format!(
        "https+custom://{}{}{}{}",
        authority, path, query, fragment
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rewrite_only_when_enabled_and_whitelisted() {
        let mut cfg = AppConfig::default();
        cfg.http.fake_sni_enabled = true;
        cfg.http.fake_sni_rollout_percent = 100; // 全量
        cfg.tls.san_whitelist = vec!["github.com".into()];
        let url = "https://github.com/rust-lang/git2-rs";
        let out = maybe_rewrite_https_to_custom_inner(&cfg, url, false).expect("should rewrite");
        assert_eq!(out, "https+custom://github.com/rust-lang/git2-rs.git");
        assert!(super::ROLLOUT_HIT.load(Ordering::Relaxed) > 0, "hit counter should increment");

        // 非 https 不改写
        assert!(maybe_rewrite_https_to_custom_inner(&cfg, "http://github.com/", false).is_none());

        // 关闭开关不改写
        cfg.http.fake_sni_enabled = false;
        assert!(maybe_rewrite_https_to_custom_inner(&cfg, url, false).is_none());

        // 非白名单域不改写
        let mut cfg2 = AppConfig::default();
        cfg2.http.fake_sni_enabled = true;
        cfg2.tls.san_whitelist = vec!["example.com".into()];
        assert!(maybe_rewrite_https_to_custom_inner(&cfg2, url, false).is_none());
    }

    #[test]
    fn test_rewrite_disabled_when_proxy_env_present() {
        let mut cfg = AppConfig::default();
        cfg.http.fake_sni_enabled = true;
        let url = "https://github.com/owner/repo";
        // 指定存在代理 -> 不改写
        assert!(maybe_rewrite_https_to_custom_inner(&cfg, url, true).is_none());
    }

    #[test]
    fn test_rollout_sampling_zero() {
        let mut cfg = AppConfig::default();
        cfg.http.fake_sni_enabled = true;
        cfg.http.fake_sni_rollout_percent = 0; // 禁用
        cfg.tls.san_whitelist = vec!["github.com".into()];
        let url = "https://github.com/rust-lang/git2-rs";
        assert!(maybe_rewrite_https_to_custom_inner(&cfg, url, false).is_none());
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
        let first = maybe_rewrite_https_to_custom_inner(&cfg, url, false).is_some();
        for _ in 0..10 { assert_eq!(first, maybe_rewrite_https_to_custom_inner(&cfg, url, false).is_some()); }
    }

    #[test]
    fn test_extra_allow_list() {
        let mut cfg = AppConfig::default();
        cfg.http.fake_sni_enabled = true;
        cfg.http.fake_sni_rollout_percent = 100;
        cfg.tls.san_whitelist = vec!["example.com".into()];
        cfg.http.host_allow_list_extra = vec!["github.com".into()];
        let url = "https://github.com/owner/repo";
        assert!(maybe_rewrite_https_to_custom_inner(&cfg, url, false).is_some());
    }
}
