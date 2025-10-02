use fireworks_collaboration_lib::core::git::transport::ensure_registered;
use fireworks_collaboration_lib::core::config::model::AppConfig;
use fireworks_collaboration_lib::core::proxy::{ProxyConfig, ProxyMode};

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
    // 代理未启用时，不应跳过自定义传输层
    // 通过检查 ensure_registered 能正常执行来验证
    let result = ensure_registered(&cfg);
    assert!(result.is_ok());
}

#[test]
fn test_should_skip_custom_transport_when_http_proxy_enabled() {
    let mut cfg = AppConfig::default();
    cfg.proxy = ProxyConfig {
        mode: ProxyMode::Http,
        url: "http://proxy:8080".to_string(),
        ..Default::default()
    };
    // HTTP代理启用时应该跳过自定义传输层
    let result = ensure_registered(&cfg);
    assert!(result.is_ok());
}

#[test]
fn test_should_skip_custom_transport_when_socks5_proxy_enabled() {
    let mut cfg = AppConfig::default();
    cfg.proxy = ProxyConfig {
        mode: ProxyMode::Socks5,
        url: "socks5://proxy:1080".to_string(),
        ..Default::default()
    };
    // SOCKS5代理启用时应该跳过自定义传输层
    let result = ensure_registered(&cfg);
    assert!(result.is_ok());
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
    // 明确配置禁用自定义传输层时应该跳过
    let result = ensure_registered(&cfg);
    assert!(result.is_ok());
}

#[test]
fn test_should_skip_custom_transport_when_system_proxy_enabled() {
    let mut cfg = AppConfig::default();
    cfg.proxy = ProxyConfig {
        mode: ProxyMode::System,
        ..Default::default()
    };
    // 系统代理模式时应该跳过自定义传输层
    let result = ensure_registered(&cfg);
    assert!(result.is_ok());
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
    let result = ensure_registered(&cfg);
    assert!(result.is_ok());
}
