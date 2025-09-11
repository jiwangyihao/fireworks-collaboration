use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HttpCfg {
    #[serde(default = "default_true")] pub fake_sni_enabled: bool,
    #[serde(default = "default_fake_sni_host")] pub fake_sni_host: String,
    #[serde(default = "default_true")] pub follow_redirects: bool,
    #[serde(default = "default_max_redirects")] pub max_redirects: u8,
    #[serde(default = "default_large_body_warn")] pub large_body_warn_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TlsCfg {
    #[serde(default = "default_san_whitelist")] pub san_whitelist: Vec<String>,
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
fn default_fake_sni_host() -> String { "baidu.com".to_string() }
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

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            http: HttpCfg {
                fake_sni_enabled: default_true(),
                fake_sni_host: default_fake_sni_host(),
                follow_redirects: default_true(),
                max_redirects: default_max_redirects(),
                large_body_warn_bytes: default_large_body_warn(),
            },
            tls: TlsCfg { san_whitelist: default_san_whitelist() },
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
        assert!(s.contains("\"fakeSniHost\""));
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
        assert_eq!(cfg.http.fake_sni_host, "baidu.com");
        assert!(cfg.http.follow_redirects);
        assert_eq!(cfg.http.max_redirects, 5);
        assert!(cfg.tls.san_whitelist.iter().any(|d| d.ends_with("github.com")));
    }
}
