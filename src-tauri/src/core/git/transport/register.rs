use std::sync::OnceLock;

use git2::{transport, Error, Remote};

use crate::core::config::loader::load_or_init;
use crate::core::config::model::AppConfig;
use crate::core::proxy::ProxyManager;

use super::http::CustomHttpsSubtransport;
use super::metrics::tl_set_proxy_usage;

/// 仅注册一次自定义传输前缀 "https+custom"。注册后，所有该 scheme 的 URL 都会通过本实现建立连接。
static REGISTER_ONCE: OnceLock<()> = OnceLock::new();

/// 检查是否应该跳过自定义传输层注册
/// 
/// 当代理启用时，应跳过自定义传输层，直接使用libgit2默认HTTP传输
fn should_skip_custom_transport(cfg: &AppConfig) -> bool {
    // 创建临时ProxyManager检查配置
    let proxy_manager = ProxyManager::new(cfg.proxy.clone());
    let should_disable = proxy_manager.should_disable_custom_transport();
    let is_enabled = proxy_manager.is_enabled();
    
    // P5.3: 记录proxy使用状态到metrics
    if is_enabled {
        let proxy_type = Some(format!("{}", proxy_manager.mode()).to_lowercase());
        tl_set_proxy_usage(true, proxy_type, None, true);
    } else if should_disable {
        // 明确配置禁用自定义传输层但代理未启用
        tl_set_proxy_usage(false, None, None, true);
    }
    
    if should_disable {
        tracing::info!(
            proxy_enabled = is_enabled,
            custom_transport_disabled = true,
            "Custom transport disabled (proxy enabled or configured to disable), using libgit2 default HTTP"
        );
    }
    
    should_disable
}

pub fn ensure_registered(cfg: &AppConfig) -> Result<(), Error> {
    // P5.3: 如果代理启用，跳过自定义传输层注册
    if should_skip_custom_transport(cfg) {
        let proxy_manager = ProxyManager::new(cfg.proxy.clone());
        tracing::debug!(
            proxy_mode = %proxy_manager.mode(),
            proxy_enabled = proxy_manager.is_enabled(),
            "Skipping custom transport registration due to proxy configuration"
        );
        return Ok(());
    }
    
    tracing::debug!("Registering custom transport for https+custom");
    
    let mut err: Option<Error> = None;
    REGISTER_ONCE.get_or_init(|| {
        // 安全：register 需外部同步；我们用 OnceLock 保证只注册一次。
        let r = unsafe {
            transport::register("https+custom", move |remote: &Remote| {
                // HTTP(s) 是无状态 smart 协议：需要启用 stateless-rpc 模式
                let rpc = true;
                // 每次创建传输时加载“最新配置”，避免保存后需重启
                let cfg_now = load_or_init().unwrap_or_else(|_| AppConfig::default());
                let sub = CustomHttpsSubtransport::new(cfg_now);
                transport::Transport::smart(remote, rpc, sub)
            })
        };
        if let Err(e) = r {
            err = Some(e);
        }
    });
    if let Some(e) = err {
        return Err(e);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::proxy::{ProxyConfig, ProxyMode};

    #[test]
    fn test_register_once_ok() {
        let cfg = AppConfig::default();
        // 多次调用不应 panic
        let _ = ensure_registered(&cfg);
        let _ = ensure_registered(&cfg);
    }

    #[test]
    fn test_should_skip_custom_transport_when_proxy_off() {
        let cfg = AppConfig::default();
        assert!(!should_skip_custom_transport(&cfg));
    }

    #[test]
    fn test_should_skip_custom_transport_when_http_proxy_enabled() {
        let mut cfg = AppConfig::default();
        cfg.proxy = ProxyConfig {
            mode: ProxyMode::Http,
            url: "http://proxy:8080".to_string(),
            ..Default::default()
        };
        assert!(should_skip_custom_transport(&cfg));
    }

    #[test]
    fn test_should_skip_custom_transport_when_socks5_proxy_enabled() {
        let mut cfg = AppConfig::default();
        cfg.proxy = ProxyConfig {
            mode: ProxyMode::Socks5,
            url: "socks5://proxy:1080".to_string(),
            ..Default::default()
        };
        assert!(should_skip_custom_transport(&cfg));
    }

    #[test]
    fn test_ensure_registered_skips_when_proxy_enabled() {
        let mut cfg = AppConfig::default();
        cfg.proxy = ProxyConfig {
            mode: ProxyMode::Http,
            url: "http://proxy:8080".to_string(),
            ..Default::default()
        };
        
        // 代理启用时应该直接返回Ok，不注册自定义传输层
        let result = ensure_registered(&cfg);
        assert!(result.is_ok());
    }

    #[test]
    fn test_should_skip_when_disable_custom_transport_set() {
        let mut cfg = AppConfig::default();
        cfg.proxy.disable_custom_transport = true;
        assert!(should_skip_custom_transport(&cfg));
    }

    #[test]
    fn test_should_skip_custom_transport_when_system_proxy_enabled() {
        let mut cfg = AppConfig::default();
        cfg.proxy = ProxyConfig {
            mode: ProxyMode::System,
            ..Default::default()
        };
        assert!(should_skip_custom_transport(&cfg));
    }

    #[test]
    fn test_should_not_skip_with_empty_proxy_url() {
        let mut cfg = AppConfig::default();
        cfg.proxy = ProxyConfig {
            mode: ProxyMode::Http,
            url: "".to_string(),  // Empty URL means proxy not enabled
            ..Default::default()
        };
        // HTTP mode with empty URL is NOT enabled, so should not skip
        assert!(!should_skip_custom_transport(&cfg));
    }
}
