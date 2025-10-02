//! Tests for system proxy detector

use fireworks_collaboration_lib::core::proxy::system_detector::SystemProxyDetector;
use fireworks_collaboration_lib::core::proxy::ProxyMode;

#[test]
fn test_parse_proxy_url_http() {
    let config = SystemProxyDetector::parse_proxy_url("http://proxy.example.com:8080");
    assert!(config.is_some());
    let config = config.unwrap();
    assert_eq!(config.mode, ProxyMode::Http);
    assert_eq!(config.url, "http://proxy.example.com:8080");
}

#[test]
fn test_parse_proxy_url_https() {
    let config = SystemProxyDetector::parse_proxy_url("https://proxy.example.com:8443");
    assert!(config.is_some());
    let config = config.unwrap();
    assert_eq!(config.mode, ProxyMode::Http);
    assert_eq!(config.url, "https://proxy.example.com:8443");
}

#[test]
fn test_parse_proxy_url_socks5() {
    let config = SystemProxyDetector::parse_proxy_url("socks5://127.0.0.1:1080");
    assert!(config.is_some());
    let config = config.unwrap();
    assert_eq!(config.mode, ProxyMode::Socks5);
    assert_eq!(config.url, "socks5://127.0.0.1:1080");
}

#[test]
fn test_parse_proxy_url_no_scheme() {
    // Should auto-add http:// scheme
    let config = SystemProxyDetector::parse_proxy_url("proxy.example.com:8080");
    assert!(config.is_some());
    let config = config.unwrap();
    assert_eq!(config.mode, ProxyMode::Http);
    assert_eq!(config.url, "http://proxy.example.com:8080");
}

#[test]
fn test_parse_proxy_url_invalid() {
    // Empty host
    let config = SystemProxyDetector::parse_proxy_url("http://");
    assert!(config.is_none());
    
    // Invalid format
    let config = SystemProxyDetector::parse_proxy_url("not-a-url");
    assert!(config.is_some()); // Will be parsed as "http://not-a-url"
}

#[test]
fn test_detect_from_env() {
    // This test depends on actual environment variables
    // Just verify it doesn't panic
    let _ = SystemProxyDetector::detect_from_env();
}

#[test]
fn test_detect() {
    // This test depends on actual system configuration
    // Just verify it doesn't panic and returns a valid option
    let result = SystemProxyDetector::detect();
    if let Some(config) = result {
        // If detected, validate it
        assert!(config.validate().is_ok());
    }
}

#[cfg(target_os = "macos")]
#[test]
fn test_parse_scutil_output() {
    let lines = vec![
        "  HTTPEnable : 1",
        "  HTTPProxy : proxy.example.com",
        "  HTTPPort : 8080",
    ];
    
    let config = SystemProxyDetector::parse_scutil_output(&lines, "HTTP");
    assert!(config.is_some());
    let config = config.unwrap();
    assert_eq!(config.mode, ProxyMode::Http);
    assert!(config.url.contains("proxy.example.com"));
    assert!(config.url.contains("8080"));
}

#[cfg(target_os = "macos")]
#[test]
fn test_parse_scutil_output_disabled() {
    let lines = vec![
        "  HTTPEnable : 0",
        "  HTTPProxy : proxy.example.com",
        "  HTTPPort : 8080",
    ];
    
    let config = SystemProxyDetector::parse_scutil_output(&lines, "HTTP");
    assert!(config.is_none());
}
