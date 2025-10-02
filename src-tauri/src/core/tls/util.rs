use crate::core::config::model::AppConfig;
use rand::{seq::SliceRandom, thread_rng};
use std::collections::HashMap;
use std::env;
use std::sync::{Mutex, OnceLock};

/// 判断是否应启用伪 SNI
/// - cfg.http.fake_sni_enabled 为真 且 未强制 real 时，返回 true
pub fn should_use_fake(cfg: &AppConfig, force_real: bool) -> bool {
    if force_real {
        return false;
    }
    cfg.http.fake_sni_enabled
}

/// 简单通配符匹配：仅支持前缀 "*." 的域名通配符
/// 例如：pattern "*.github.com" 可匹配 "api.github.com" 但不匹配 "github.com"
pub fn match_domain(pattern: &str, host: &str) -> bool {
    let p = pattern.to_ascii_lowercase();
    let h = host.to_ascii_lowercase();
    if p == h {
        return true;
    }
    if let Some(rest) = p.strip_prefix("*.") {
        // host 需要至少包含一个子域，且以 rest 结尾
        if h.ends_with(rest) {
            // 确保有一个 '.' 分隔，且不是恰好等于 rest
            return h.len() > rest.len() && h.as_bytes()[h.len() - rest.len() - 1] == b'.';
        }
    }
    false
}

/// 检测是否存在系统/环境代理（HTTP/HTTPS/ALL_PROXY），Windows 将在上层通过 get_system_proxy 注入，此处先检查常见环境变量。
pub fn proxy_present() -> bool {
    let keys = [
        "HTTPS_PROXY",
        "https_proxy",
        "HTTP_PROXY",
        "http_proxy",
        "ALL_PROXY",
        "all_proxy",
    ];
    for k in keys {
        if let Ok(v) = env::var(k) {
            if !v.trim().is_empty() {
                return true;
            }
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
pub fn decide_sni_host_with_proxy(
    cfg: &AppConfig,
    force_real: bool,
    real_host: &str,
    proxy_present: bool,
) -> (String, bool) {
    if force_real || proxy_present {
        return (real_host.to_string(), false);
    }
    if !cfg.http.fake_sni_enabled {
        return (real_host.to_string(), false);
    }
    // 构造候选：仅来自 fake_sni_hosts（受用户控制）；忽略 legacy fake_sni_host
    let candidates: Vec<String> = cfg
        .http
        .fake_sni_hosts
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
    let pick = candidates
        .choose(&mut rng)
        .cloned()
        .unwrap_or_else(|| candidates[0].clone());
    (pick, true)
}
