use crate::core::config::model::AppConfig;
use rand::{seq::SliceRandom, thread_rng};
use std::env;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

/// 判断是否应启用伪 SNI
/// - cfg.http.fake_sni_enabled 为真 且 未强制 real 时，返回 true
pub fn should_use_fake(cfg: &AppConfig, force_real: bool) -> bool {
    if force_real { return false; }
    cfg.http.fake_sni_enabled
}

/// 简单通配符匹配：仅支持前缀 "*." 的域名通配符
/// 例如：pattern "*.github.com" 可匹配 "api.github.com" 但不匹配 "github.com"
pub fn match_domain(pattern: &str, host: &str) -> bool {
    let p = pattern.to_ascii_lowercase();
    let h = host.to_ascii_lowercase();
    if p == h { return true; }
    if let Some(rest) = p.strip_prefix("*.") {
        // host 需要至少包含一个子域，且以 rest 结尾
        if h.ends_with(rest) {
            // 确保有一个 '.' 分隔，且不是恰好等于 rest
            return h.len() > rest.len() && h.as_bytes()[h.len()-rest.len()-1] == b'.';
        }
    }
    false
}

/// 检测是否存在系统/环境代理（HTTP/HTTPS/ALL_PROXY），Windows 将在上层通过 get_system_proxy 注入，此处先检查常见环境变量。
pub fn proxy_present() -> bool {
    let keys = [
        "HTTPS_PROXY", "https_proxy",
        "HTTP_PROXY", "http_proxy",
        "ALL_PROXY", "all_proxy",
    ];
    for k in keys {
        if let Ok(v) = env::var(k) {
            if !v.trim().is_empty() { return true; }
        }
    }
    false
}

// 记录最近一次“成功”的 SNI（按真实主机划分），仅用于优先选择。不会持久化到配置文件。
// 注意：仅在 InfoRefs 等返回 2xx 且使用了伪 SNI 时才更新。
static LAST_GOOD_SNI: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();

fn last_map() -> &'static Mutex<HashMap<String, String>> {
    LAST_GOOD_SNI.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn get_last_good_sni(real_host: &str) -> Option<String> {
    let m = last_map().lock().ok()?;
    m.get(real_host).cloned()
}

pub fn set_last_good_sni(real_host: &str, sni: &str) {
    if let Ok(mut m) = last_map().lock() {
        m.insert(real_host.to_string(), sni.to_string());
    }
}

/// 结合代理情况，决定最终使用的 SNI 主机名。
/// - 代理存在时：强制使用真实 host（减少指纹风险）。
/// - 无代理时：若启用 fakeSni，则从候选中选一个（若提供多个可随机）；否则使用真实 host。
pub fn decide_sni_host_with_proxy(cfg: &AppConfig, force_real: bool, real_host: &str, proxy_present: bool) -> (String, bool) {
    if force_real || proxy_present { return (real_host.to_string(), false); }
    if !cfg.http.fake_sni_enabled { return (real_host.to_string(), false); }
    // 构造候选：仅来自 fake_sni_hosts（受用户控制）；忽略 legacy fake_sni_host
    let candidates: Vec<String> = cfg.http.fake_sni_hosts
        .iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    if candidates.is_empty() {
        return (real_host.to_string(), false);
    }
    // 若有最近成功记录，且仍在候选中，则优先使用它
    if let Some(last) = get_last_good_sni(real_host) {
        if candidates.iter().any(|c| c == &last) {
            return (last, true);
        }
    }
    // 随机挑选其一，降低同一 SNI 重复命中带来的风控概率
    let mut rng = thread_rng();
    let pick = candidates.choose(&mut rng).cloned().unwrap_or_else(|| candidates[0].clone());
    (pick, true)
}

#[cfg(test)]
mod tests {
    use super::*;

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
        // 大小写不敏感
        assert!(match_domain("GITHUB.COM", "github.com"));
        assert!(match_domain("*.GitHub.com", "API.GitHub.Com"));
        // 多级子域也应匹配（只要以基域结尾且有 '.' 边界）
        assert!(match_domain("*.github.com", "a.b.github.com"));
        // 不支持的通配模式（非前缀"*.") 视为不匹配
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

        // 有代理则强制真实
        let (sni2, used2) = decide_sni_host_with_proxy(&cfg, false, "github.com", true);
        assert_eq!(sni2, "github.com");
        assert!(!used2);

        // 关闭开关回退真实
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
}
