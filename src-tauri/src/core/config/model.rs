use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HttpCfg {
    #[serde(default = "default_true")] pub fake_sni_enabled: bool,
    /// 可选：自定义多个伪 SNI 候选；若未提供，默认填充一组常见域名以便开箱即用
    #[serde(default = "default_fake_sni_hosts")] pub fake_sni_hosts: Vec<String>,
    /// 在服务端返回 403 时，是否尝试切换到其他 SNI 候选并自动重试（仅限 info/refs GET 阶段）
    #[serde(default = "default_true")] pub sni_rotate_on_403: bool,
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
                follow_redirects: default_true(),
                max_redirects: default_max_redirects(),
                large_body_warn_bytes: default_large_body_warn(),
            },
            tls: TlsCfg { san_whitelist: default_san_whitelist(), insecure_skip_verify: false, skip_san_whitelist: false },
            logging: LoggingCfg { auth_header_masked: default_true(), log_level: default_log_level() },
        }
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
    }

    #[test]
    fn test_deserialize_with_defaults() {
        // 只提供部分字段，其他应回退默认
        let json = r#"{
          "http": { "fakeSniEnabled": false },
          "tls": {},
          "logging": { "logLevel": "debug" }
        }"#;
        let cfg: AppConfig = serde_json::from_str(json).unwrap();
        // 提供的值覆盖
        assert!(!cfg.http.fake_sni_enabled);
        assert_eq!(cfg.logging.log_level, "debug");
        // 未提供的保持默认
        assert!(cfg.http.follow_redirects);
        assert_eq!(cfg.http.max_redirects, 5);
        assert!(cfg.tls.san_whitelist.iter().any(|d| d.ends_with("github.com")));
    }
}
