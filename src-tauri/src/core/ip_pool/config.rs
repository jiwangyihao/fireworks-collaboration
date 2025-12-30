use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    io::Write,
    path::{Path, PathBuf},
};
use url::Url;

use crate::core::config::loader;

fn default_true() -> bool {
    true
}

pub fn default_max_parallel_probes() -> usize {
    4
}

pub fn default_score_ttl_seconds() -> u64 {
    300
}

fn default_preheat_ports() -> Vec<u16> {
    vec![443]
}

fn default_probe_path() -> String {
    "/".to_string()
}

/// 延迟探测方法：HTTP 应用层协议测试或 TCP 握手测试
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum ProbeMethod {
    /// HTTPing：使用 HTTPS HEAD 请求测量应用层延迟（默认，TUN 模式兼容）
    #[default]
    Http,
    /// TCP 握手延迟测试（传统方式，TUN 模式下可能不准确）
    Tcp,
}

const IP_CONFIG_FILE_NAME: &str = "ip-config.json";

/// 运行期控制项，来自主配置文件（config.json）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct IpPoolRuntimeConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub sources: IpPoolSourceToggle,
    #[serde(default)]
    pub dns: DnsRuntimeConfig,
    #[serde(default = "default_max_parallel_probes")]
    pub max_parallel_probes: usize,
    /// 单位：毫秒。后续阶段用于 TCP 握手超时控制。
    #[serde(default = "default_probe_timeout_ms")]
    pub probe_timeout_ms: u64,
    /// 可选：覆盖历史缓存文件路径，缺省时由模块自行推导。
    #[serde(default)]
    pub history_path: Option<String>,
    /// TTL 定期清理间隔（秒）。
    #[serde(default = "default_cache_prune_interval_secs")]
    pub cache_prune_interval_secs: u64,
    /// 缓存最大条目数（仅统计非预热域名）。0 表示无限制。
    #[serde(default = "default_max_cache_entries")]
    pub max_cache_entries: usize,
    /// 单飞等待超时时间（毫秒）。
    #[serde(default = "default_singleflight_timeout_ms")]
    pub singleflight_timeout_ms: u64,
    /// 熔断器：连续失败次数阈值。
    #[serde(default = "default_failure_threshold")]
    pub failure_threshold: u32,
    /// 熔断器：失败率阈值（0.0-1.0）。
    #[serde(default = "default_failure_rate_threshold")]
    pub failure_rate_threshold: f64,
    /// 熔断器：时间窗口大小（秒）。
    #[serde(default = "default_failure_window_seconds")]
    pub failure_window_seconds: u32,
    /// 熔断器：窗口内最小样本数。
    #[serde(default = "default_min_samples_in_window")]
    pub min_samples_in_window: u32,
    /// 熔断器：冷却时间（秒）。
    #[serde(default = "default_cooldown_seconds")]
    pub cooldown_seconds: u32,
    /// 熔断器：是否启用。
    #[serde(default = "default_true")]
    pub circuit_breaker_enabled: bool,
    /// 延迟探测方法：HTTP（默认，TUN 兼容）或 TCP
    #[serde(default)]
    pub probe_method: ProbeMethod,
    /// HTTP 探测路径（默认 "/"）
    #[serde(default = "default_probe_path")]
    pub probe_path: String,
}

pub fn default_probe_timeout_ms() -> u64 {
    1500
}

pub fn default_cache_prune_interval_secs() -> u64 {
    60
}

pub fn default_max_cache_entries() -> usize {
    256
}

pub fn default_singleflight_timeout_ms() -> u64 {
    10_000
}

pub fn default_failure_threshold() -> u32 {
    3
}

pub fn default_failure_rate_threshold() -> f64 {
    0.5
}

pub fn default_failure_window_seconds() -> u32 {
    60
}

pub fn default_min_samples_in_window() -> u32 {
    5
}

pub fn default_cooldown_seconds() -> u32 {
    300
}

impl Default for IpPoolRuntimeConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            sources: IpPoolSourceToggle::default(),
            dns: DnsRuntimeConfig::default(),
            max_parallel_probes: default_max_parallel_probes(),
            probe_timeout_ms: default_probe_timeout_ms(),
            history_path: None,
            cache_prune_interval_secs: default_cache_prune_interval_secs(),
            max_cache_entries: default_max_cache_entries(),
            singleflight_timeout_ms: default_singleflight_timeout_ms(),
            failure_threshold: default_failure_threshold(),
            failure_rate_threshold: default_failure_rate_threshold(),
            failure_window_seconds: default_failure_window_seconds(),
            min_samples_in_window: default_min_samples_in_window(),
            cooldown_seconds: default_cooldown_seconds(),
            circuit_breaker_enabled: true,
            probe_method: ProbeMethod::default(),
            probe_path: default_probe_path(),
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

/// DNS 解析策略：允许配置系统解析与自定义 DoH/DoT 解析器。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum DnsResolverProtocol {
    Udp,
    Doh,
    Dot,
}

impl Default for DnsResolverProtocol {
    fn default() -> Self {
        Self::Udp
    }
}

impl DnsResolverProtocol {
    pub fn display_name(&self) -> &'static str {
        match self {
            DnsResolverProtocol::Udp => "UDP",
            DnsResolverProtocol::Doh => "DoH",
            DnsResolverProtocol::Dot => "DoT",
        }
    }
}

/// 预设的 DoH/DoT 解析服务配置。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DnsResolverPreset {
    pub server: String,
    #[serde(default)]
    pub r#type: Option<String>,
    #[serde(default)]
    pub sni: Option<String>,
    #[serde(default, rename = "cacheSize")]
    pub cache_size: Option<usize>,
    #[serde(default, rename = "desc")]
    pub description: Option<String>,
    #[serde(default, rename = "forSNI")]
    pub for_sni: bool,
}

impl DnsResolverPreset {
    fn infer_protocol(&self) -> DnsResolverProtocol {
        if let Some(kind) = self.r#type.as_deref() {
            match kind.to_ascii_lowercase().as_str() {
                "https" | "doh" => return DnsResolverProtocol::Doh,
                "tls" | "dot" => return DnsResolverProtocol::Dot,
                "udp" => return DnsResolverProtocol::Udp,
                _ => {}
            }
        }

        if let Ok(url) = Url::parse(&self.server) {
            match url.scheme() {
                "https" | "doh" => return DnsResolverProtocol::Doh,
                "tls" | "dot" => return DnsResolverProtocol::Dot,
                "udp" | "udp4" | "udp6" => return DnsResolverProtocol::Udp,
                _ => {}
            }
        }

        if self.server.starts_with("https://") {
            DnsResolverProtocol::Doh
        } else if self.server.starts_with("tls://") {
            DnsResolverProtocol::Dot
        } else {
            DnsResolverProtocol::Doh
        }
    }

    pub fn to_resolver_config(&self, key: &str) -> Result<DnsResolverConfig> {
        let protocol = self.infer_protocol();
        let mut endpoint = self.server.clone();

        let normalized = if endpoint.contains("://") {
            endpoint.clone()
        } else {
            match protocol {
                DnsResolverProtocol::Doh => format!("https://{}", endpoint),
                DnsResolverProtocol::Dot => format!("tls://{}", endpoint),
                DnsResolverProtocol::Udp => format!("udp://{}", endpoint),
            }
        };

        let url = Url::parse(&normalized)
            .with_context(|| format!("parse dns preset '{}' server '{}'", key, self.server))?;

        let host = url
            .host_str()
            .ok_or_else(|| anyhow!("dns preset '{}' missing host", key))?
            .to_string();
        let url_port = url.port();

        endpoint = match protocol {
            DnsResolverProtocol::Doh => normalized,
            DnsResolverProtocol::Dot | DnsResolverProtocol::Udp => {
                if let Some(port) = url_port {
                    format!("{}:{}", host, port)
                } else {
                    host
                }
            }
        };

        Ok(DnsResolverConfig {
            label: key.to_string(),
            protocol,
            endpoint,
            port: url_port,
            bootstrap_ips: Vec::new(),
            sni: self.sni.clone(),
            cache_size: self.cache_size,
            description: self.description.clone(),
            preset_key: Some(key.to_string()),
        })
    }
}

fn default_dns_preset_entries() -> Vec<(String, DnsResolverPreset)> {
    vec![
        (
            "cf-DoT".to_string(),
            DnsResolverPreset {
                server: "tls://1.1.1.1".to_string(),
                r#type: None,
                sni: Some("baidu.com".to_string()),
                cache_size: Some(1_000),
                description: None,
                for_sni: false,
            },
        ),
        (
            "cf-DoH".to_string(),
            DnsResolverPreset {
                server: "https://cloudflare-dns.com/dns-query".to_string(),
                r#type: None,
                sni: Some("baidu.com".to_string()),
                cache_size: Some(1_000),
                description: None,
                for_sni: false,
            },
        ),
        (
            "Google-DoH".to_string(),
            DnsResolverPreset {
                server: "https://dns.google/dns-query".to_string(),
                r#type: None,
                sni: Some("www.google.cn".to_string()),
                cache_size: Some(1_000),
                description: Some("不可用".to_string()),
                for_sni: false,
            },
        ),
        (
            "aliyun".to_string(),
            DnsResolverPreset {
                server: "https://dns.alidns.com/dns-query".to_string(),
                r#type: Some("https".to_string()),
                sni: None,
                cache_size: Some(1_000),
                description: None,
                for_sni: false,
            },
        ),
        (
            "cloudflare".to_string(),
            DnsResolverPreset {
                server: "https://1.1.1.1/dns-query".to_string(),
                r#type: Some("https".to_string()),
                sni: None,
                cache_size: Some(1_000),
                description: None,
                for_sni: false,
            },
        ),
        (
            "quad9".to_string(),
            DnsResolverPreset {
                server: "https://9.9.9.9/dns-query".to_string(),
                r#type: Some("https".to_string()),
                sni: None,
                cache_size: Some(1_000),
                description: None,
                for_sni: false,
            },
        ),
        (
            "safe360".to_string(),
            DnsResolverPreset {
                server: "https://doh.360.cn/dns-query".to_string(),
                r#type: Some("https".to_string()),
                sni: None,
                cache_size: Some(1_000),
                description: None,
                for_sni: true,
            },
        ),
        (
            "rubyfish".to_string(),
            DnsResolverPreset {
                server: "https://rubyfish.cn/dns-query".to_string(),
                r#type: Some("https".to_string()),
                sni: None,
                cache_size: Some(1_000),
                description: None,
                for_sni: false,
            },
        ),
    ]
}

fn default_dns_preset_catalog() -> BTreeMap<String, DnsResolverPreset> {
    let mut map = BTreeMap::new();
    for (key, preset) in default_dns_preset_entries() {
        map.insert(key, preset);
    }
    map
}

fn default_enabled_dns_presets() -> Vec<String> {
    default_dns_preset_entries()
        .into_iter()
        .filter(|(_, preset)| preset.description.as_deref() != Some("不可用"))
        .map(|(key, _)| key)
        .collect()
}

/// 自定义 DNS 解析器配置。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DnsResolverConfig {
    pub label: String,
    #[serde(default)]
    pub protocol: DnsResolverProtocol,
    pub endpoint: String,
    #[serde(default)]
    pub port: Option<u16>,
    #[serde(default)]
    pub bootstrap_ips: Vec<String>,
    #[serde(default)]
    pub sni: Option<String>,
    #[serde(default, rename = "cacheSize")]
    pub cache_size: Option<usize>,
    #[serde(default, rename = "desc")]
    pub description: Option<String>,
    #[serde(default, rename = "presetKey")]
    pub preset_key: Option<String>,
}

impl DnsResolverConfig {
    pub fn effective_port(&self) -> u16 {
        match self.protocol {
            DnsResolverProtocol::Udp => self.port.unwrap_or(53),
            DnsResolverProtocol::Doh => self.port.unwrap_or(443),
            DnsResolverProtocol::Dot => self.port.unwrap_or(853),
        }
    }

    pub fn display_tag(&self) -> String {
        let protocol = self.protocol.display_name();
        if let Some(key) = self
            .preset_key
            .as_ref()
            .map(|k| k.trim())
            .filter(|k| !k.is_empty())
        {
            if let Some(desc) = self
                .description
                .as_deref()
                .map(str::trim)
                .filter(|d| !d.is_empty())
            {
                return format!("{} ({}, {})", key, protocol, desc);
            }
            return format!("{} ({})", key, protocol);
        }

        let label = self.label.trim();
        if label.is_empty() {
            protocol.to_string()
        } else {
            format!("{} ({})", label, protocol)
        }
    }
}

/// DNS 运行时配置。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DnsRuntimeConfig {
    #[serde(default = "default_true")]
    pub use_system: bool,
    #[serde(default)]
    pub resolvers: Vec<DnsResolverConfig>,
    #[serde(default = "default_dns_preset_catalog")]
    pub preset_catalog: BTreeMap<String, DnsResolverPreset>,
    #[serde(default = "default_enabled_dns_presets")]
    pub enabled_presets: Vec<String>,
}

impl Default for DnsRuntimeConfig {
    fn default() -> Self {
        Self {
            use_system: true,
            resolvers: Vec::new(),
            preset_catalog: default_dns_preset_catalog(),
            enabled_presets: default_enabled_dns_presets(),
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
    /// IP 黑名单（支持单个 IP 地址或 CIDR 表示法）
    #[serde(default)]
    pub blacklist: Vec<String>,
    /// IP 白名单（支持单个 IP 地址或 CIDR 表示法，优先级高于黑名单）
    #[serde(default)]
    pub whitelist: Vec<String>,
    /// 禁用内置预热域名列表（按域名匹配，不区分大小写）
    #[serde(default)]
    pub disabled_builtin_preheat: Vec<String>,
}

impl Default for IpPoolFileConfig {
    fn default() -> Self {
        Self {
            preheat_domains: Vec::new(),
            score_ttl_seconds: default_score_ttl_seconds(),
            user_static: Vec::new(),
            blacklist: Vec::new(),
            whitelist: Vec::new(),
            disabled_builtin_preheat: Vec::new(),
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

/// 组合后的生效配置，便于 `IpPool` 管理运行期与外部文件配置。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct EffectiveIpPoolConfig {
    #[serde(default)]
    pub runtime: IpPoolRuntimeConfig,
    #[serde(default)]
    pub file: IpPoolFileConfig,
}

impl EffectiveIpPoolConfig {
    pub fn from_parts(runtime: IpPoolRuntimeConfig, file: IpPoolFileConfig) -> Self {
        Self { runtime, file }
    }
}

pub fn join_ip_config_path(base: &Path) -> PathBuf {
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
