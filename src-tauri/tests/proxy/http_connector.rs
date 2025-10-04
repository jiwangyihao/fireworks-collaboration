//! Tests for HTTP/HTTPS proxy connector implementation
//!
//! These tests verify:
//! - HTTP proxy connector creation and configuration
//! - URL parsing for various formats (HTTP/HTTPS, IPv4/IPv6, ports)
//! - Basic authentication header generation
//! - Edge cases and error scenarios

use fireworks_collaboration_lib::core::proxy::http_connector::HttpProxyConnector;
use fireworks_collaboration_lib::core::proxy::{ProxyConnector, ProxyError};
use std::time::Duration;

#[test]
fn test_http_connector_creation() {
    let connector = HttpProxyConnector::new(
        "http://proxy.example.com:8080".to_string(),
        Some("user".to_string()),
        Some("pass".to_string()),
        Duration::from_secs(30),
    );

    assert_eq!(connector.proxy_type(), "http");
}

#[test]
fn test_parse_proxy_url_http() {
    let connector = HttpProxyConnector::new(
        "http://proxy.example.com:8080".to_string(),
        None,
        None,
        Duration::from_secs(30),
    );

    let result = connector.parse_proxy_url().unwrap();
    assert_eq!(result.0, "proxy.example.com");
    assert_eq!(result.1, 8080);
}

#[test]
fn test_parse_proxy_url_https() {
    let connector = HttpProxyConnector::new(
        "https://secure-proxy.example.com:3128".to_string(),
        None,
        None,
        Duration::from_secs(30),
    );

    let result = connector.parse_proxy_url().unwrap();
    assert_eq!(result.0, "secure-proxy.example.com");
    assert_eq!(result.1, 3128);
}

#[test]
fn test_parse_proxy_url_default_port() {
    let connector = HttpProxyConnector::new(
        "http://proxy.example.com".to_string(),
        None,
        None,
        Duration::from_secs(30),
    );

    let result = connector.parse_proxy_url().unwrap();
    assert_eq!(result.0, "proxy.example.com");
    assert_eq!(result.1, 8080); // Default port
}

#[test]
fn test_parse_proxy_url_with_ipv4() {
    let connector = HttpProxyConnector::new(
        "http://192.168.1.100:8080".to_string(),
        None,
        None,
        Duration::from_secs(30),
    );

    let result = connector.parse_proxy_url().unwrap();
    assert_eq!(result.0, "192.168.1.100");
    assert_eq!(result.1, 8080);
}

#[test]
fn test_generate_auth_header_with_credentials() {
    let connector = HttpProxyConnector::new(
        "http://proxy.example.com:8080".to_string(),
        Some("testuser".to_string()),
        Some("testpass".to_string()),
        Duration::from_secs(30),
    );

    let auth = connector.generate_auth_header();
    assert!(auth.is_some());
    let auth_value = auth.unwrap();
    assert!(auth_value.starts_with("Basic "));

    // Verify it's valid base64
    let base64_part = auth_value.strip_prefix("Basic ").unwrap();
    assert!(!base64_part.is_empty());
}

#[test]
fn test_generate_auth_header_without_credentials() {
    let connector = HttpProxyConnector::new(
        "http://proxy.example.com:8080".to_string(),
        None,
        None,
        Duration::from_secs(30),
    );

    let auth = connector.generate_auth_header();
    assert!(auth.is_none());
}

#[test]
fn test_generate_auth_header_partial_credentials_user_only() {
    let connector = HttpProxyConnector::new(
        "http://proxy.example.com:8080".to_string(),
        Some("user".to_string()),
        None, // Missing password
        Duration::from_secs(30),
    );

    let auth = connector.generate_auth_header();
    assert!(auth.is_none()); // Should not generate auth without password
}

#[test]
fn test_generate_auth_header_partial_credentials_password_only() {
    let connector = HttpProxyConnector::new(
        "http://proxy.example.com:8080".to_string(),
        None, // Missing username
        Some("pass".to_string()),
        Duration::from_secs(30),
    );

    let auth = connector.generate_auth_header();
    assert!(auth.is_none()); // Should not generate auth without username
}

#[test]
fn test_parse_invalid_proxy_url() {
    let connector = HttpProxyConnector::new(
        "not-a-valid-url".to_string(),
        None,
        None,
        Duration::from_secs(30),
    );

    let result = connector.parse_proxy_url();
    assert!(result.is_err());

    match result {
        Err(ProxyError::Config(_)) => (),
        _ => panic!("Expected Config error"),
    }
}

#[test]
fn test_parse_proxy_url_no_host() {
    let connector =
        HttpProxyConnector::new("http://".to_string(), None, None, Duration::from_secs(30));

    let result = connector.parse_proxy_url();
    assert!(result.is_err());

    match result {
        Err(ProxyError::Config(_)) => (),
        _ => panic!("Expected Config error"),
    }
}

#[test]
fn test_generate_auth_header_special_characters() {
    let connector = HttpProxyConnector::new(
        "http://proxy.example.com:8080".to_string(),
        Some("user@domain".to_string()),
        Some("p@ss:word!".to_string()),
        Duration::from_secs(30),
    );

    let auth = connector.generate_auth_header();
    assert!(auth.is_some());
    assert!(auth.unwrap().starts_with("Basic "));
}

#[test]
fn test_timeout_duration() {
    let connector = HttpProxyConnector::new(
        "http://proxy.example.com:8080".to_string(),
        None,
        None,
        Duration::from_secs(45),
    );

    assert_eq!(connector.timeout, Duration::from_secs(45));
}

#[test]
fn test_connector_implements_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<HttpProxyConnector>();
}

#[test]
fn test_proxy_url_with_path() {
    let connector = HttpProxyConnector::new(
        "http://proxy.example.com:8080/path".to_string(),
        None,
        None,
        Duration::from_secs(30),
    );

    let result = connector.parse_proxy_url().unwrap();
    assert_eq!(result.0, "proxy.example.com");
    assert_eq!(result.1, 8080);
}

#[test]
fn test_proxy_url_with_credentials_in_url() {
    let connector = HttpProxyConnector::new(
        "http://urluser:urlpass@proxy.example.com:8080".to_string(),
        None,
        None,
        Duration::from_secs(30),
    );

    let result = connector.parse_proxy_url().unwrap();
    assert_eq!(result.0, "proxy.example.com");
    assert_eq!(result.1, 8080);
}

#[test]
fn test_auth_header_with_empty_strings() {
    let connector = HttpProxyConnector::new(
        "http://proxy.example.com:8080".to_string(),
        Some("".to_string()),
        Some("".to_string()),
        Duration::from_secs(30),
    );

    // Empty credentials should still generate auth header
    let auth = connector.generate_auth_header();
    assert!(auth.is_some());
}

#[test]
fn test_auth_header_with_unicode() {
    let connector = HttpProxyConnector::new(
        "http://proxy.example.com:8080".to_string(),
        Some("用户名".to_string()),
        Some("密码123".to_string()),
        Duration::from_secs(30),
    );

    let auth = connector.generate_auth_header();
    assert!(auth.is_some());
    assert!(auth.unwrap().starts_with("Basic "));
}

#[test]
fn test_very_short_timeout() {
    let connector = HttpProxyConnector::new(
        "http://proxy.example.com:8080".to_string(),
        None,
        None,
        Duration::from_millis(1),
    );

    assert_eq!(connector.timeout, Duration::from_millis(1));
}

#[test]
fn test_very_long_timeout() {
    let connector = HttpProxyConnector::new(
        "http://proxy.example.com:8080".to_string(),
        None,
        None,
        Duration::from_secs(3600),
    );

    assert_eq!(connector.timeout, Duration::from_secs(3600));
}

#[test]
fn test_parse_proxy_url_with_ipv6() {
    let connector = HttpProxyConnector::new(
        "http://[::1]:8080".to_string(),
        None,
        None,
        Duration::from_secs(30),
    );

    let result = connector.parse_proxy_url().unwrap();
    // URL parser keeps brackets for IPv6
    assert_eq!(result.0, "[::1]");
    assert_eq!(result.1, 8080);
}

#[test]
fn test_parse_proxy_url_with_high_port() {
    let connector = HttpProxyConnector::new(
        "http://proxy.example.com:65535".to_string(),
        None,
        None,
        Duration::from_secs(30),
    );

    let result = connector.parse_proxy_url().unwrap();
    assert_eq!(result.0, "proxy.example.com");
    assert_eq!(result.1, 65535);
}

#[test]
fn test_multiple_connectors_independent() {
    let connector1 = HttpProxyConnector::new(
        "http://proxy1.example.com:8080".to_string(),
        Some("user1".to_string()),
        Some("pass1".to_string()),
        Duration::from_secs(30),
    );

    let connector2 = HttpProxyConnector::new(
        "http://proxy2.example.com:3128".to_string(),
        Some("user2".to_string()),
        Some("pass2".to_string()),
        Duration::from_secs(60),
    );

    assert_eq!(connector1.proxy_type(), "http");
    assert_eq!(connector2.proxy_type(), "http");
    assert_ne!(connector1.proxy_url, connector2.proxy_url);
    assert_ne!(connector1.timeout, connector2.timeout);
}

#[test]
fn test_auth_header_credentials_order() {
    let connector = HttpProxyConnector::new(
        "http://proxy.example.com:8080".to_string(),
        Some("testuser".to_string()),
        Some("testpass".to_string()),
        Duration::from_secs(30),
    );

    let auth = connector.generate_auth_header().unwrap();
    // Decode and verify format (username:password)
    let base64_part = auth.strip_prefix("Basic ").unwrap();
    assert!(!base64_part.is_empty());

    // The encoded string should be "testuser:testpass"
    use base64::{engine::general_purpose::STANDARD, Engine};
    let expected = STANDARD.encode("testuser:testpass".as_bytes());
    assert_eq!(base64_part, expected);
}

#[test]
fn test_parse_proxy_url_invalid_port() {
    let connector = HttpProxyConnector::new(
        "http://proxy.example.com:abc".to_string(),
        None,
        None,
        Duration::from_secs(30),
    );
    let result = connector.parse_proxy_url();
    assert!(result.is_err());
    match result {
        Err(ProxyError::Config(_)) => (),
        _ => panic!("Expected Config error for non-numeric port"),
    }
}

#[test]
fn test_parse_proxy_url_negative_port() {
    let connector = HttpProxyConnector::new(
        "http://proxy.example.com:-123".to_string(),
        None,
        None,
        Duration::from_secs(30),
    );
    let result = connector.parse_proxy_url();
    assert!(result.is_err());
    match result {
        Err(ProxyError::Config(_)) => (),
        _ => panic!("Expected Config error for negative port"),
    }
}

#[test]
fn test_parse_proxy_url_too_large_port() {
    let connector = HttpProxyConnector::new(
        "http://proxy.example.com:70000".to_string(),
        None,
        None,
        Duration::from_secs(30),
    );
    let result = connector.parse_proxy_url();
    assert!(result.is_err());
    match result {
        Err(ProxyError::Config(_)) => (),
        _ => panic!("Expected Config error for too large port"),
    }
}

#[test]
fn test_generate_auth_header_very_long_credentials() {
    let long_user = "u".repeat(1024);
    let long_pass = "p".repeat(1024);
    let connector = HttpProxyConnector::new(
        "http://proxy.example.com:8080".to_string(),
        Some(long_user.clone()),
        Some(long_pass.clone()),
        Duration::from_secs(30),
    );
    let auth = connector.generate_auth_header();
    assert!(auth.is_some());
    let auth_value = auth.unwrap();
    assert!(auth_value.starts_with("Basic "));
    let base64_part = auth_value.strip_prefix("Basic ").unwrap();
    use base64::{engine::general_purpose::STANDARD, Engine};
    let expected = STANDARD.encode(format!("{long_user}:{long_pass}").as_bytes());
    assert_eq!(base64_part, expected);
}

#[test]
fn test_generate_auth_header_unicode_edge() {
    let connector = HttpProxyConnector::new(
        "http://proxy.example.com:8080".to_string(),
        Some("𝓤𝓼𝓮𝓻名".to_string()),
        Some("𝓟𝓪𝓼𝓼密".to_string()),
        Duration::from_secs(30),
    );
    let auth = connector.generate_auth_header();
    assert!(auth.is_some());
    let auth_value = auth.unwrap();
    assert!(auth_value.starts_with("Basic "));
}
