use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use crate::core::config::loader;

fn default_true() -> bool {
    true
}

fn default_max_parallel_probes() -> usize {
    4
}

fn default_score_ttl_seconds() -> u64 {
    300
}

fn default_preheat_ports() -> Vec<u16> {
    vec![443]
}

const IP_CONFIG_FILE_NAME: &str = "ip-config.json";

/// 运行期控制项，来自主配置文件（config.json）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct IpPoolRuntimeConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub sources: IpPoolSourceToggle,
    #[serde(default = "default_max_parallel_probes")]
    pub max_parallel_probes: usize,
    /// 单位：毫秒。后续阶段用于 TCP 握手超时控制。
    #[serde(default = "default_probe_timeout_ms")]
    pub probe_timeout_ms: u64,
    /// 可选：覆盖历史缓存文件路径，缺省时由模块自行推导。
    #[serde(default)]
    pub history_path: Option<String>,
}

fn default_probe_timeout_ms() -> u64 {
    1500
}

impl Default for IpPoolRuntimeConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            sources: IpPoolSourceToggle::default(),
            max_parallel_probes: default_max_parallel_probes(),
            probe_timeout_ms: default_probe_timeout_ms(),
            history_path: None,
        }
    }
}

/// 数据来源开关，控制是否采纳对应来源的候选 IP。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct IpPoolSourceToggle {
    #[serde(default = "default_true")]
    pub builtin: bool,
    #[serde(default = "default_true")]
    pub dns: bool,
    #[serde(default = "default_true")]
    pub history: bool,
    #[serde(default = "default_true")]
    pub user_static: bool,
    #[serde(default = "default_true")]
    pub fallback: bool,
}

impl Default for IpPoolSourceToggle {
    fn default() -> Self {
        Self {
            builtin: true,
            dns: true,
            history: true,
            user_static: true,
            fallback: true,
        }
    }
}

/// 外部配置（ip-config.json）结构，记录预热域名与评分 TTL。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct IpPoolFileConfig {
    #[serde(default)]
    pub preheat_domains: Vec<PreheatDomain>,
    #[serde(default = "default_score_ttl_seconds")]
    pub score_ttl_seconds: u64,
    #[serde(default)]
    pub user_static: Vec<UserStaticIp>,
}

impl Default for IpPoolFileConfig {
    fn default() -> Self {
        Self {
            preheat_domains: Vec::new(),
            score_ttl_seconds: default_score_ttl_seconds(),
            user_static: Vec::new(),
        }
    }
}

/// 预热域名配置，支持指定多个端口（默认仅 443）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub struct PreheatDomain {
    pub host: String,
    #[serde(default = "default_preheat_ports")]
    pub ports: Vec<u16>,
}

impl PreheatDomain {
    pub fn new<S: Into<String>>(host: S) -> Self {
        Self {
            host: host.into(),
            ports: default_preheat_ports(),
        }
    }
}

/// 用户静态 IP 配置，允许针对特定域名写入固定 IP。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub struct UserStaticIp {
    pub host: String,
    pub ip: String,
    #[serde(default = "default_preheat_ports")]
    pub ports: Vec<u16>,
}

/// 组合后的生效配置，便于 IpPool 管理运行期与外部文件配置。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EffectiveIpPoolConfig {
    #[serde(default)]
    pub runtime: IpPoolRuntimeConfig,
    #[serde(default)]
    pub file: IpPoolFileConfig,
}

impl Default for EffectiveIpPoolConfig {
    fn default() -> Self {
        Self {
            runtime: IpPoolRuntimeConfig::default(),
            file: IpPoolFileConfig::default(),
        }
    }
}

impl EffectiveIpPoolConfig {
    pub fn from_parts(runtime: IpPoolRuntimeConfig, file: IpPoolFileConfig) -> Self {
        Self { runtime, file }
    }
}

fn join_ip_config_path(base: &Path) -> PathBuf {
    let mut path = base.to_path_buf();
    path.push("config");
    path.push(IP_CONFIG_FILE_NAME);
    path
}

pub fn load_or_init_file() -> Result<IpPoolFileConfig> {
    load_or_init_file_at(&loader::base_dir())
}

pub fn save_file(cfg: &IpPoolFileConfig) -> Result<()> {
    save_file_at(cfg, &loader::base_dir())
}

pub fn load_or_init_file_at(base_dir: &Path) -> Result<IpPoolFileConfig> {
    let path = join_ip_config_path(base_dir);
    if path.exists() {
        let data =
            fs::read(&path).with_context(|| format!("read ip config: {}", path.display()))?;
        let cfg: IpPoolFileConfig =
            serde_json::from_slice(&data).context("parse ip config json")?;
        Ok(cfg)
    } else {
        let cfg = IpPoolFileConfig::default();
        save_file_at(&cfg, base_dir)?;
        Ok(cfg)
    }
}

pub fn save_file_at(cfg: &IpPoolFileConfig, base_dir: &Path) -> Result<()> {
    let path = join_ip_config_path(base_dir);
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).ok();
    }
    let json = serde_json::to_string_pretty(cfg).context("serialize ip config")?;
    let mut file =
        fs::File::create(&path).with_context(|| format!("create ip config: {}", path.display()))?;
    file.write_all(json.as_bytes()).context("write ip config")?;
    tracing::info!(target = "ip_pool", path = %path.display(), "ip config saved");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};

    #[test]
    fn runtime_defaults_are_disabled() {
        let cfg = IpPoolRuntimeConfig::default();
        assert!(!cfg.enabled);
        assert_eq!(cfg.max_parallel_probes, default_max_parallel_probes());
        assert_eq!(cfg.probe_timeout_ms, default_probe_timeout_ms());
        assert!(cfg.history_path.is_none());
        assert!(cfg.sources.builtin);
        assert!(cfg.sources.dns);
        assert!(cfg.sources.history);
        assert!(cfg.sources.user_static);
        assert!(cfg.sources.fallback);
    }

    #[test]
    fn file_defaults_are_empty_preheat() {
        let cfg = IpPoolFileConfig::default();
        assert!(cfg.preheat_domains.is_empty());
        assert_eq!(cfg.score_ttl_seconds, default_score_ttl_seconds());
        assert!(cfg.user_static.is_empty());
    }

    #[test]
    fn deserializes_with_defaults() {
        let json = r#"{
            "runtime": {
                "enabled": true,
                "maxParallelProbes": 8,
                "historyPath": "custom/ip-history.json"
            },
            "file": {
                "preheatDomains": [{"host": "github.com", "ports": [443, 80]}]
            }
        }"#;
        let cfg: EffectiveIpPoolConfig = serde_json::from_str(json).unwrap();
        assert!(cfg.runtime.enabled);
        assert_eq!(cfg.runtime.max_parallel_probes, 8);
        assert_eq!(cfg.runtime.probe_timeout_ms, default_probe_timeout_ms());
        assert_eq!(cfg.file.score_ttl_seconds, default_score_ttl_seconds());
        assert_eq!(cfg.file.preheat_domains.len(), 1);
        let domain = &cfg.file.preheat_domains[0];
        assert_eq!(domain.host, "github.com");
        assert_eq!(domain.ports, vec![443, 80]);
        assert!(cfg.file.user_static.is_empty());
    }

    #[test]
    fn load_or_init_file_creates_default() {
        let guard = test_guard().lock().unwrap();
        let temp_dir = std::env::temp_dir().join(format!("fwc-ip-pool-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&temp_dir).unwrap();
        let cfg = load_or_init_file_at(&temp_dir).expect("create default ip config");
        assert!(cfg.preheat_domains.is_empty());
        assert_eq!(cfg.score_ttl_seconds, default_score_ttl_seconds());
        let path = join_ip_config_path(&temp_dir);
        assert!(path.exists());
        fs::remove_dir_all(&temp_dir).ok();
        drop(guard);
    }

    #[test]
    fn save_file_persists_changes() {
        let guard = test_guard().lock().unwrap();
        let temp_dir =
            std::env::temp_dir().join(format!("fwc-ip-pool-save-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&temp_dir).unwrap();
        let mut cfg = IpPoolFileConfig::default();
        cfg.preheat_domains.push(PreheatDomain::new("github.com"));
        cfg.score_ttl_seconds = 120;
        cfg.user_static.push(UserStaticIp {
            host: "github.com".into(),
            ip: "140.82.112.3".into(),
            ports: vec![443],
        });
        save_file_at(&cfg, &temp_dir).expect("save ip config");
        let loaded = load_or_init_file_at(&temp_dir).expect("load ip config");
        assert_eq!(loaded.preheat_domains.len(), 1);
        assert_eq!(loaded.score_ttl_seconds, 120);
        assert_eq!(loaded.user_static.len(), 1);
        fs::remove_dir_all(&temp_dir).ok();
        drop(guard);
    }

    fn test_guard() -> &'static Mutex<()> {
        static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
        GUARD.get_or_init(|| Mutex::new(()))
    }
}
