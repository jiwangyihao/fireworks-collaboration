use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HttpCfg {
    #[serde(default = "default_true")] pub fake_sni_enabled: bool,
    /// 可选：自定义多个伪 SNI 候选；若未提供，默认填充一组常见域名以便开箱即用
    #[serde(default = "default_fake_sni_hosts")] pub fake_sni_hosts: Vec<String>,
    /// 在服务端返回 403 时，是否尝试切换到其他 SNI 候选并自动重试（仅限 info/refs GET 阶段）
    #[serde(default = "default_true")] pub sni_rotate_on_403: bool,
    /// P3.1: 渐进放量百分比 (0..=100)。缺省 100 表示全量启用；0 表示禁用（等价 fake_sni_enabled=false）。
    #[serde(default = "default_rollout_percent")] pub fake_sni_rollout_percent: u8,
    /// P3.1: 附加允许进入自适应 TLS (Fake SNI) 采样的域名白名单（不自动加入 SAN 校验白名单，仅用于改写判定）。
    #[serde(default)] pub host_allow_list_extra: Vec<String>,
    #[serde(default = "default_true")] pub follow_redirects: bool,
    #[serde(default = "default_max_redirects")] pub max_redirects: u8,
    #[serde(default = "default_large_body_warn")] pub large_body_warn_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TlsCfg {
    #[serde(default = "default_san_whitelist")] pub san_whitelist: Vec<String>,
    /// 原型期可选：跳过证书链与域名校验（极不安全，默认关闭）
    #[serde(default)] pub insecure_skip_verify: bool,
    /// 可选：仅跳过自定义 SAN 白名单校验，但仍执行常规的证书链与域名校验（默认关闭）
    #[serde(default)] pub skip_san_whitelist: bool,
    /// P3.2: 是否启用自适应 TLS timing 指标采集（connect/tls/firstByte/total）。默认启用；关闭后不记录/不发事件。
    #[serde(default = "default_true")] pub metrics_enabled: bool,
    /// P3.2: 是否启用证书指纹日志与变更检测。默认启用；关闭后不写 cert-fp.log 也不触发指纹变更事件。
    #[serde(default = "default_true")] pub cert_fp_log_enabled: bool,
    /// P3.2: 证书指纹日志文件最大字节数（达到后进行简单滚动 rename 为 .1 并重新开始）。默认 5MB。
    #[serde(default = "default_cert_fp_max_bytes")] pub cert_fp_max_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoggingCfg {
    #[serde(default = "default_true")] pub auth_header_masked: bool,
    #[serde(default = "default_log_level")] pub log_level: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    pub http: HttpCfg,
    pub tls: TlsCfg,
    pub logging: LoggingCfg,
    #[serde(default)] pub retry: RetryCfg,
    /// 是否支持 partial filter（服务端 capability 已验证）。默认为 false；
    /// 可通过环境变量 `FWC_PARTIAL_FILTER_SUPPORTED=1` 在运行时覆盖（仅影响 fallback 行为，不启用真正 partial 传输）。
    #[serde(default)] pub partial_filter_supported: bool,
}

fn default_true() -> bool { true }
fn default_max_redirects() -> u8 { 5 }
fn default_large_body_warn() -> u64 { 5 * 1024 * 1024 }
fn default_san_whitelist() -> Vec<String> {
    vec![
        "github.com".into(),
        "*.github.com".into(),
        "*.githubusercontent.com".into(),
        "*.githubassets.com".into(),
        "codeload.github.com".into(),
    ]
}
fn default_log_level() -> String { "info".to_string() }
fn default_rollout_percent() -> u8 { 100 }
fn default_cert_fp_max_bytes() -> u64 { 5 * 1024 * 1024 }

/// 默认的假 SNI 候选列表（中国常见网站域名）
fn default_fake_sni_hosts() -> Vec<String> {
    vec![
        // 综合门户/搜索/视频/社交/电商等常见大站
        "baidu.com".into(),
        "qq.com".into(),
        "weibo.com".into(),
        "bilibili.com".into(),
        "jd.com".into(),
        "taobao.com".into(),
        "tmall.com".into(),
        "csdn.net".into(),
        "163.com".into(),
        "126.com".into(),
        "sina.com.cn".into(),
        "sohu.com".into(),
        "youku.com".into(),
        "iqiyi.com".into(),
        "douyin.com".into(),
        "xiaomi.com".into(),
        "mi.com".into(),
        "huawei.com".into(),
        "360.cn".into(),
        "kuaishou.com".into(),
    ]
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            http: HttpCfg {
                fake_sni_enabled: default_true(),
                fake_sni_hosts: default_fake_sni_hosts(),
                sni_rotate_on_403: default_true(),
                fake_sni_rollout_percent: default_rollout_percent(),
                host_allow_list_extra: Vec::new(),
                follow_redirects: default_true(),
                max_redirects: default_max_redirects(),
                large_body_warn_bytes: default_large_body_warn(),
            },
            tls: TlsCfg { san_whitelist: default_san_whitelist(), insecure_skip_verify: false, skip_san_whitelist: false, metrics_enabled: true, cert_fp_log_enabled: true, cert_fp_max_bytes: default_cert_fp_max_bytes() },
            logging: LoggingCfg { auth_header_masked: default_true(), log_level: default_log_level() },
            retry: RetryCfg::default(),
            partial_filter_supported: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RetryCfg {
    /// 最大重试次数（不含首次尝试）；例如 3 表示最多共尝试 4 次
    #[serde(default = "default_retry_max")] pub max: u32,
    /// 初始基准延迟（毫秒）
    #[serde(default = "default_retry_base_ms")] pub base_ms: u64,
    /// 指数因子
    #[serde(default = "default_retry_factor")] pub factor: f64,
    /// 是否开启随机抖动（±50%）
    #[serde(default = "default_true")] pub jitter: bool,
}

fn default_retry_max() -> u32 { 6 }
fn default_retry_base_ms() -> u64 { 300 }
fn default_retry_factor() -> f64 { 1.5 }

impl Default for RetryCfg {
    fn default() -> Self {
        Self { max: default_retry_max(), base_ms: default_retry_base_ms(), factor: default_retry_factor(), jitter: true }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serialize_camel_case_keys() {
        let cfg = AppConfig::default();
        let s = serde_json::to_string(&cfg).unwrap();
        // 关键字段以 camelCase 出现
        assert!(s.contains("\"fakeSniEnabled\""));
        assert!(s.contains("\"followRedirects\""));
        assert!(s.contains("\"maxRedirects\""));
        assert!(s.contains("\"largeBodyWarnBytes\""));
        assert!(s.contains("\"sanWhitelist\""));
        assert!(s.contains("\"authHeaderMasked\""));
        assert!(s.contains("\"logLevel\""));
    assert!(s.contains("\"retry\""));
    assert!(s.contains("\"baseMs\""));
    assert!(s.contains("\"factor\""));
    assert!(s.contains("\"jitter\""));
    assert!(s.contains("\"partialFilterSupported\""));
    assert!(s.contains("\"metricsEnabled\""));
    assert!(s.contains("\"certFpLogEnabled\""));
    assert!(s.contains("\"certFpMaxBytes\""));
    }

    #[test]
    fn test_deserialize_with_defaults() {
        // 只提供部分字段，其他应回退默认
        let json = r#"{
          "http": { "fakeSniEnabled": false },
          "tls": {},
                    "logging": { "logLevel": "debug" },
                    "retry": { "max": 2, "baseMs": 200, "factor": 2.0, "jitter": false }
        }"#;
        let cfg: AppConfig = serde_json::from_str(json).unwrap();
        // 提供的值覆盖
        assert!(!cfg.http.fake_sni_enabled);
        assert_eq!(cfg.logging.log_level, "debug");
                assert_eq!(cfg.retry.max, 2);
                assert_eq!(cfg.retry.base_ms, 200);
                assert!(!cfg.retry.jitter);
        // 未提供的保持默认
        assert!(cfg.http.follow_redirects);
        assert_eq!(cfg.http.max_redirects, 5);
        assert!(cfg.tls.san_whitelist.iter().any(|d| d.ends_with("github.com")));
    assert!(cfg.tls.metrics_enabled, "metricsEnabled default true");
    assert!(cfg.tls.cert_fp_log_enabled, "certFpLogEnabled default true");
    assert_eq!(cfg.tls.cert_fp_max_bytes, 5*1024*1024);
    }
}
