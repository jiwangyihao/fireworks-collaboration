use crate::core::config::model::AppConfig;

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
}
