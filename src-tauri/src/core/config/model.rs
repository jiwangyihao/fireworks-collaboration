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
    #[serde(default = "default_san_whitelist")]
    pub san_whitelist: Vec<String>,
    /// 原型期可选：跳过证书链与域名校验（极不安全，默认关闭）
    #[serde(default)]
    pub insecure_skip_verify: bool,
    /// 可选：仅跳过自定义 SAN 白名单校验，但仍执行常规的证书链与域名校验（默认关闭）
    #[serde(default)]
    pub skip_san_whitelist: bool,
    /// P3.4: SPKI Pin 列表（Base64URL 无填充，长度=43）。非空即启用强校验。
    #[serde(default)]
    pub spki_pins: Vec<String>,
    /// P3.3: Real-Host 验证开关。启用后即便握手使用了 Fake SNI，证书链与主机名验证也以真实目标域名为准；
    /// 失败时会触发一次回退至 Real SNI 的重握手。默认启用。
    #[serde(default = "default_true")]
    pub real_host_verify_enabled: bool,
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
fn default_san_whitelist() -> Vec<String> {
    vec![
        "github.com".into(),
        "*.github.com".into(),
        "*.githubusercontent.com".into(),
        "*.githubassets.com".into(),
        "codeload.github.com".into(),
    ]
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
                auto_disable_fake_threshold_pct: default_auto_disable_threshold_pct(),
                auto_disable_fake_cooldown_sec: default_auto_disable_cooldown_sec(),
            },
            tls: TlsCfg {
                san_whitelist: default_san_whitelist(),
                insecure_skip_verify: false,
                skip_san_whitelist: false,
                spki_pins: Vec::new(),
                real_host_verify_enabled: true,
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
        }
    }
}
