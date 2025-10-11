use serde::{Deserialize, Serialize};

use crate::core::credential::config::CredentialConfig;
use crate::core::ip_pool::IpPoolRuntimeConfig;
use crate::core::proxy::ProxyConfig;
use crate::core::submodule::SubmoduleConfig;
use crate::core::workspace::WorkspaceConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HttpCfg {
    #[serde(default = "default_true")]
    pub fake_sni_enabled: bool,
    /// 可选：自定义多个伪 SNI 候选；若未提供，默认填充一组常见域名以便开箱即用
    #[serde(default = "default_fake_sni_hosts")]
    pub fake_sni_hosts: Vec<String>,
    /// 仅对这些域名启用伪 SNI 与自定义验证逻辑；为空表示禁用伪 SNI
    #[serde(default)]
    pub fake_sni_target_hosts: Vec<String>,
    /// 在服务端返回 403 时，是否尝试切换到其他 SNI 候选并自动重试（仅限 info/refs GET 阶段）
    #[serde(default = "default_true")]
    pub sni_rotate_on_403: bool,
    /// P3.1: 渐进放量百分比 (0..=100)。缺省 100 表示全量启用；0 表示禁用（等价 `fake_sni_enabled=false`）。
    #[serde(default = "default_rollout_percent")]
    pub fake_sni_rollout_percent: u8,
    /// P3.1: 附加允许进入自适应 TLS (Fake SNI) 采样的域名白名单（不自动加入 SAN 校验白名单，仅用于改写判定）。
    #[serde(default)]
    pub host_allow_list_extra: Vec<String>,
    #[serde(default = "default_true")]
    pub follow_redirects: bool,
    #[serde(default = "default_max_redirects")]
    pub max_redirects: u8,
    #[serde(default = "default_large_body_warn")]
    pub large_body_warn_bytes: u64,
    /// P3.5: 当 Fake SNI 在窗口内失败率超过阈值时，临时禁用 Fake 的比例阈值（0..=100）。0 表示禁用该功能。
    #[serde(default = "default_auto_disable_threshold_pct")]
    pub auto_disable_fake_threshold_pct: u8,
    /// P3.5: 触发自动禁用后保持禁用状态的冷却秒数。
    #[serde(default = "default_auto_disable_cooldown_sec")]
    pub auto_disable_fake_cooldown_sec: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TlsCfg {
    /// P3.4: SPKI Pin 列表（Base64URL 无填充，长度=43）。非空即启用强校验。
    #[serde(default)]
    pub spki_pins: Vec<String>,
    /// P3.2: 是否启用自适应 TLS timing 指标采集（connect/tls/firstByte/total）。默认启用；关闭后不记录/不发事件。
    #[serde(default = "default_true")]
    pub metrics_enabled: bool,
    /// P3.2: 是否启用证书指纹日志与变更检测。默认启用；关闭后不写 cert-fp.log 也不触发指纹变更事件。
    #[serde(default = "default_true")]
    pub cert_fp_log_enabled: bool,
    /// P3.2: 证书指纹日志文件最大字节数（达到后进行简单滚动 rename 为 .1 并重新开始）。默认 5MB。
    #[serde(default = "default_cert_fp_max_bytes")]
    pub cert_fp_max_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoggingCfg {
    #[serde(default = "default_true")]
    pub auth_header_masked: bool,
    #[serde(default = "default_log_level")]
    pub log_level: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    pub http: HttpCfg,
    pub tls: TlsCfg,
    pub logging: LoggingCfg,
    #[serde(default)]
    pub retry: RetryCfg,
    /// 是否支持 partial filter（服务端 capability 已验证）。默认为 false；
    /// 可通过环境变量 `FWC_PARTIAL_FILTER_SUPPORTED=1` 在运行时覆盖（仅影响 fallback 行为，不启用真正 partial 传输）。
    #[serde(default)]
    pub partial_filter_supported: bool,
    /// P4.0: IP 池运行期配置，默认关闭。
    #[serde(default)]
    pub ip_pool: IpPoolRuntimeConfig,
    /// P5.0: 代理配置，默认关闭。
    #[serde(default)]
    pub proxy: ProxyConfig,
    /// P6.0: 凭证存储与安全管理配置，默认使用系统钥匙串。
    #[serde(default)]
    pub credential: CredentialConfig,
    /// P7.0: 工作区与多仓库管理配置，默认禁用。
    #[serde(default)]
    pub workspace: WorkspaceConfig,
    /// P7.1: 子模块管理配置，默认启用自动递归。
    #[serde(default)]
    pub submodule: SubmoduleConfig,
    /// P8.1: 可观测性与指标配置，默认启用基础埋点。
    #[serde(default)]
    pub observability: ObservabilityConfig,
}

fn default_true() -> bool {
    true
}
fn default_max_redirects() -> u8 {
    5
}
fn default_large_body_warn() -> u64 {
    5 * 1024 * 1024
}
pub fn default_auto_disable_threshold_pct() -> u8 {
    20
}
pub fn default_auto_disable_cooldown_sec() -> u64 {
    300
}
fn default_log_level() -> String {
    "info".to_string()
}
fn default_rollout_percent() -> u8 {
    100
}
fn default_cert_fp_max_bytes() -> u64 {
    5 * 1024 * 1024
}

fn default_export_rate_limit_qps() -> u32 {
    5
}

fn default_export_max_series() -> u32 {
    1_000
}

fn default_export_bind_address() -> String {
    "127.0.0.1:9688".to_string()
}

fn default_alert_rules_path() -> String {
    "config/observability/alert-rules.json".into()
}

fn default_alert_eval_interval() -> u32 {
    30
}

fn default_alert_min_repeat() -> u32 {
    30
}

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
                fake_sni_target_hosts: Vec::new(),
                sni_rotate_on_403: default_true(),
                fake_sni_rollout_percent: default_rollout_percent(),
                host_allow_list_extra: Vec::new(),
                follow_redirects: default_true(),
                max_redirects: default_max_redirects(),
                large_body_warn_bytes: default_large_body_warn(),
                auto_disable_fake_threshold_pct: default_auto_disable_threshold_pct(),
                auto_disable_fake_cooldown_sec: default_auto_disable_cooldown_sec(),
            },
            tls: TlsCfg {
                spki_pins: Vec::new(),
                metrics_enabled: true,
                cert_fp_log_enabled: true,
                cert_fp_max_bytes: default_cert_fp_max_bytes(),
            },
            logging: LoggingCfg {
                auth_header_masked: default_true(),
                log_level: default_log_level(),
            },
            retry: RetryCfg::default(),
            partial_filter_supported: false,
            ip_pool: IpPoolRuntimeConfig::default(),
            proxy: ProxyConfig::default(),
            credential: CredentialConfig::default(),
            workspace: WorkspaceConfig::default(),
            submodule: SubmoduleConfig::default(),
            observability: ObservabilityConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RetryCfg {
    /// 最大重试次数（不含首次尝试）；例如 3 表示最多共尝试 4 次
    #[serde(default = "default_retry_max")]
    pub max: u32,
    /// 初始基准延迟（毫秒）
    #[serde(default = "default_retry_base_ms")]
    pub base_ms: u64,
    /// 指数因子
    #[serde(default = "default_retry_factor")]
    pub factor: f64,
    /// 是否开启随机抖动（±50%）
    #[serde(default = "default_true")]
    pub jitter: bool,
}

fn default_retry_max() -> u32 {
    6
}
fn default_retry_base_ms() -> u64 {
    300
}
fn default_retry_factor() -> f64 {
    1.5
}

fn default_observability_layer() -> ObservabilityLayer {
    ObservabilityLayer::Optimize
}

fn default_min_layer_residency_secs() -> u32 {
    300
}

fn default_downgrade_cooldown_secs() -> u32 {
    120
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
#[repr(u8)]
pub enum ObservabilityLayer {
    Basic = 0,
    Aggregate = 1,
    Export = 2,
    Ui = 3,
    Alerts = 4,
    Optimize = 5,
}

impl ObservabilityLayer {
    pub const fn as_u8(self) -> u8 {
        self as u8
    }

    pub fn as_str(self) -> &'static str {
        match self {
            ObservabilityLayer::Basic => "basic",
            ObservabilityLayer::Aggregate => "aggregate",
            ObservabilityLayer::Export => "export",
            ObservabilityLayer::Ui => "ui",
            ObservabilityLayer::Alerts => "alerts",
            ObservabilityLayer::Optimize => "optimize",
        }
    }

    pub fn from_u8(value: u8) -> Self {
        match value {
            0 => ObservabilityLayer::Basic,
            1 => ObservabilityLayer::Aggregate,
            2 => ObservabilityLayer::Export,
            3 => ObservabilityLayer::Ui,
            4 => ObservabilityLayer::Alerts,
            _ => ObservabilityLayer::Optimize,
        }
    }

    pub fn next_lower(self) -> Option<Self> {
        match self {
            ObservabilityLayer::Basic => None,
            ObservabilityLayer::Aggregate => Some(ObservabilityLayer::Basic),
            ObservabilityLayer::Export => Some(ObservabilityLayer::Aggregate),
            ObservabilityLayer::Ui => Some(ObservabilityLayer::Export),
            ObservabilityLayer::Alerts => Some(ObservabilityLayer::Ui),
            ObservabilityLayer::Optimize => Some(ObservabilityLayer::Alerts),
        }
    }

    pub fn next_higher(self) -> Option<Self> {
        match self {
            ObservabilityLayer::Basic => Some(ObservabilityLayer::Aggregate),
            ObservabilityLayer::Aggregate => Some(ObservabilityLayer::Export),
            ObservabilityLayer::Export => Some(ObservabilityLayer::Ui),
            ObservabilityLayer::Ui => Some(ObservabilityLayer::Alerts),
            ObservabilityLayer::Alerts => Some(ObservabilityLayer::Optimize),
            ObservabilityLayer::Optimize => None,
        }
    }
}

impl Default for RetryCfg {
    fn default() -> Self {
        Self {
            max: default_retry_max(),
            base_ms: default_retry_base_ms(),
            factor: default_retry_factor(),
            jitter: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObservabilityConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_true")]
    pub basic_enabled: bool,
    #[serde(default = "default_true")]
    pub aggregate_enabled: bool,
    #[serde(default = "default_true")]
    pub export_enabled: bool,
    #[serde(default = "default_true")]
    pub ui_enabled: bool,
    #[serde(default = "default_true")]
    pub alerts_enabled: bool,
    #[serde(default = "default_observability_layer")]
    pub layer: ObservabilityLayer,
    #[serde(default = "default_true")]
    pub auto_downgrade: bool,
    #[serde(default = "default_min_layer_residency_secs")]
    pub min_layer_residency_secs: u32,
    #[serde(default = "default_downgrade_cooldown_secs")]
    pub downgrade_cooldown_secs: u32,
    #[serde(default)]
    pub export: ObservabilityExportConfig,
    #[serde(default)]
    pub alerts: ObservabilityAlertsConfig,
    #[serde(default)]
    pub performance: ObservabilityPerformanceConfig,
}

impl Default for ObservabilityConfig {
    fn default() -> Self {
        Self {
            enabled: default_true(),
            basic_enabled: default_true(),
            aggregate_enabled: default_true(),
            export_enabled: default_true(),
            ui_enabled: default_true(),
            alerts_enabled: default_true(),
            layer: default_observability_layer(),
            auto_downgrade: default_true(),
            min_layer_residency_secs: default_min_layer_residency_secs(),
            downgrade_cooldown_secs: default_downgrade_cooldown_secs(),
            export: ObservabilityExportConfig::default(),
            alerts: ObservabilityAlertsConfig::default(),
            performance: ObservabilityPerformanceConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObservabilityAlertsConfig {
    #[serde(default = "default_alert_rules_path")]
    pub rules_path: String,
    #[serde(default = "default_alert_eval_interval")]
    pub eval_interval_secs: u32,
    #[serde(default = "default_alert_min_repeat")]
    pub min_repeat_interval_secs: u32,
}

impl Default for ObservabilityAlertsConfig {
    fn default() -> Self {
        Self {
            rules_path: default_alert_rules_path(),
            eval_interval_secs: default_alert_eval_interval(),
            min_repeat_interval_secs: default_alert_min_repeat(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObservabilityExportConfig {
    #[serde(default)]
    pub auth_token: Option<String>,
    #[serde(default = "default_export_rate_limit_qps")]
    pub rate_limit_qps: u32,
    #[serde(default = "default_export_max_series")]
    pub max_series_per_snapshot: u32,
    #[serde(default = "default_export_bind_address")]
    pub bind_address: String,
}

impl Default for ObservabilityExportConfig {
    fn default() -> Self {
        Self {
            auth_token: None,
            rate_limit_qps: default_export_rate_limit_qps(),
            max_series_per_snapshot: default_export_max_series(),
            bind_address: default_export_bind_address(),
        }
    }
}

fn default_redact_ip_mode() -> ObservabilityRedactIpMode {
    ObservabilityRedactIpMode::Mask
}

fn default_performance_batch_flush_interval_ms() -> u32 {
    500
}

fn default_performance_tls_sample_rate() -> u32 {
    5
}

fn default_performance_max_memory_bytes() -> u64 {
    8_000_000
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObservabilityPerformanceConfig {
    #[serde(default = "default_performance_batch_flush_interval_ms")]
    pub batch_flush_interval_ms: u32,
    #[serde(default = "default_performance_tls_sample_rate")]
    pub tls_sample_rate: u32,
    #[serde(default = "default_performance_max_memory_bytes")]
    pub max_memory_bytes: u64,
    #[serde(default = "default_true")]
    pub enable_sharding: bool,
    #[serde(default)]
    pub redact: ObservabilityRedactConfig,
    #[serde(default)]
    pub debug_mode: bool,
}

impl Default for ObservabilityPerformanceConfig {
    fn default() -> Self {
        Self {
            batch_flush_interval_ms: default_performance_batch_flush_interval_ms(),
            tls_sample_rate: default_performance_tls_sample_rate(),
            max_memory_bytes: default_performance_max_memory_bytes(),
            enable_sharding: default_true(),
            redact: ObservabilityRedactConfig::default(),
            debug_mode: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObservabilityRedactConfig {
    #[serde(default)]
    pub repo_hash_salt: String,
    #[serde(default = "default_redact_ip_mode")]
    pub ip_mode: ObservabilityRedactIpMode,
}

impl Default for ObservabilityRedactConfig {
    fn default() -> Self {
        Self {
            repo_hash_salt: String::new(),
            ip_mode: default_redact_ip_mode(),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
#[repr(u8)]
pub enum ObservabilityRedactIpMode {
    Mask,
    Classify,
    Full,
}
