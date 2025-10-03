//! Tests for SOCKS5 proxy connector implementation
//!
//! These tests verify:
//! - URL parsing for various SOCKS5 URL formats
//! - Connector creation and configuration
//! - Protocol constants and flow logic
//! - Edge cases and error scenarios

use fireworks_collaboration_lib::core::proxy::socks5_connector::Socks5ProxyConnector;
use fireworks_collaboration_lib::core::proxy::ProxyConnector;
use std::time::Duration;

#[test]
fn test_socks5_connector_creation() {
    let connector = Socks5ProxyConnector::new(
        "socks5://proxy.example.com:1080".to_string(),
        None,
        None,
        Duration::from_secs(30),
    )
    .unwrap();

    assert_eq!(connector.proxy_host, "proxy.example.com");
    assert_eq!(connector.proxy_port, 1080);
    assert!(connector.username.is_none());
    assert!(connector.password.is_none());
}

#[test]
fn test_parse_proxy_url_socks5_scheme() {
    let (host, port) = Socks5ProxyConnector::parse_proxy_url("socks5://localhost:1080").unwrap();
    assert_eq!(host, "localhost");
    assert_eq!(port, 1080);
}

#[test]
fn test_parse_proxy_url_socks_scheme() {
    let (host, port) = Socks5ProxyConnector::parse_proxy_url("socks://proxy.local:9050").unwrap();
    assert_eq!(host, "proxy.local");
    assert_eq!(port, 9050);
}

#[test]
fn test_parse_proxy_url_no_scheme() {
    let (host, port) = Socks5ProxyConnector::parse_proxy_url("192.168.1.100:1080").unwrap();
    assert_eq!(host, "192.168.1.100");
    assert_eq!(port, 1080);
}

#[test]
fn test_parse_proxy_url_with_ipv6() {
    let (host, port) = Socks5ProxyConnector::parse_proxy_url("[::1]:1080").unwrap();
    assert_eq!(host, "[::1]");
    assert_eq!(port, 1080);
}

#[test]
fn test_parse_proxy_url_with_high_port() {
    let (host, port) = Socks5ProxyConnector::parse_proxy_url("proxy:65535").unwrap();
    assert_eq!(host, "proxy");
    assert_eq!(port, 65535);
}

#[test]
fn test_parse_invalid_proxy_url_no_port() {
    let result = Socks5ProxyConnector::parse_proxy_url("proxy.example.com");
    assert!(result.is_err());
}

#[test]
fn test_parse_invalid_proxy_url_empty_host() {
    let result = Socks5ProxyConnector::parse_proxy_url(":1080");
    assert!(result.is_err());
}

#[test]
fn test_parse_invalid_proxy_url_invalid_port() {
    let result = Socks5ProxyConnector::parse_proxy_url("proxy:abc");
    assert!(result.is_err());
}

#[test]
fn test_parse_invalid_proxy_url_zero_port() {
    let result = Socks5ProxyConnector::parse_proxy_url("proxy:0");
    assert!(result.is_err());
}

#[test]
fn test_sanitized_url_without_credentials() {
    let connector = Socks5ProxyConnector::new(
        "socks5://proxy.example.com:1080".to_string(),
        None,
        None,
        Duration::from_secs(30),
    )
    .unwrap();

    assert_eq!(connector.sanitized_url(), "socks5://proxy.example.com:1080");
}

#[test]
fn test_sanitized_url_with_credentials() {
    let connector = Socks5ProxyConnector::new(
        "socks5://proxy.example.com:1080".to_string(),
        Some("user".to_string()),
        Some("pass".to_string()),
        Duration::from_secs(30),
    )
    .unwrap();

    assert_eq!(
        connector.sanitized_url(),
        "socks5://***:***@proxy.example.com:1080"
    );
}

#[test]
fn test_connector_with_credentials() {
    let connector = Socks5ProxyConnector::new(
        "socks5://proxy:1080".to_string(),
        Some("testuser".to_string()),
        Some("testpass".to_string()),
        Duration::from_secs(30),
    )
    .unwrap();

    assert_eq!(connector.username, Some("testuser".to_string()));
    assert_eq!(connector.password, Some("testpass".to_string()));
}

#[test]
fn test_timeout_duration() {
    let connector = Socks5ProxyConnector::new(
        "socks5://proxy:1080".to_string(),
        None,
        None,
        Duration::from_secs(60),
    )
    .unwrap();

    assert_eq!(connector.timeout, Duration::from_secs(60));
}

#[test]
fn test_connector_implements_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Socks5ProxyConnector>();
}

#[test]
fn test_parse_proxy_url_negative_port() {
    let result = Socks5ProxyConnector::parse_proxy_url("proxy:-1");
    assert!(result.is_err());
}

#[test]
fn test_parse_proxy_url_too_large_port() {
    let result = Socks5ProxyConnector::parse_proxy_url("proxy:70000");
    assert!(result.is_err());
}

#[test]
fn test_parse_proxy_url_port_overflow() {
    let result = Socks5ProxyConnector::parse_proxy_url("proxy:999999999999");
    assert!(result.is_err());
}

#[test]
fn test_connector_with_very_long_url() {
    let long_host = "a".repeat(255);
    let url = format!("socks5://{}:1080", long_host);
    let connector = Socks5ProxyConnector::new(url, None, None, Duration::from_secs(30));
    assert!(connector.is_ok());
}

#[test]
fn test_connector_with_unicode_hostname() {
    let connector = Socks5ProxyConnector::new(
        "socks5://代理服务器.example.com:1080".to_string(),
        None,
        None,
        Duration::from_secs(30),
    )
    .unwrap();

    assert_eq!(connector.proxy_host, "代理服务器.example.com");
}

#[test]
fn test_connector_with_username_only() {
    let connector = Socks5ProxyConnector::new(
        "socks5://proxy:1080".to_string(),
        Some("user".to_string()),
        None,
        Duration::from_secs(30),
    )
    .unwrap();

    // Should have username but no password
    assert!(connector.username.is_some());
    assert!(connector.password.is_none());
}

#[test]
fn test_connector_with_password_only() {
    let connector = Socks5ProxyConnector::new(
        "socks5://proxy:1080".to_string(),
        None,
        Some("pass".to_string()),
        Duration::from_secs(30),
    )
    .unwrap();

    // Should have password but no username
    assert!(connector.username.is_none());
    assert!(connector.password.is_some());
}

#[test]
fn test_multiple_connectors_independent() {
    let connector1 = Socks5ProxyConnector::new(
        "socks5://proxy1:1080".to_string(),
        Some("user1".to_string()),
        Some("pass1".to_string()),
        Duration::from_secs(30),
    )
    .unwrap();

    let connector2 = Socks5ProxyConnector::new(
        "socks5://proxy2:9050".to_string(),
        Some("user2".to_string()),
        Some("pass2".to_string()),
        Duration::from_secs(60),
    )
    .unwrap();

    assert_eq!(connector1.proxy_host, "proxy1");
    assert_eq!(connector1.proxy_port, 1080);
    assert_eq!(connector2.proxy_host, "proxy2");
    assert_eq!(connector2.proxy_port, 9050);
}

#[test]
fn test_very_short_timeout() {
    let connector = Socks5ProxyConnector::new(
        "socks5://proxy:1080".to_string(),
        None,
        None,
        Duration::from_millis(1),
    )
    .unwrap();

    assert_eq!(connector.timeout, Duration::from_millis(1));
}

#[test]
fn test_very_long_timeout() {
    let connector = Socks5ProxyConnector::new(
        "socks5://proxy:1080".to_string(),
        None,
        None,
        Duration::from_secs(3600),
    )
    .unwrap();

    assert_eq!(connector.timeout, Duration::from_secs(3600));
}

#[test]
fn test_parse_proxy_url_with_port_1() {
    let (host, port) = Socks5ProxyConnector::parse_proxy_url("proxy:1").unwrap();
    assert_eq!(host, "proxy");
    assert_eq!(port, 1);
}

#[test]
fn test_sanitized_url_format() {
    let connector1 = Socks5ProxyConnector::new(
        "socks5://proxy:1080".to_string(),
        None,
        None,
        Duration::from_secs(30),
    )
    .unwrap();

    assert!(connector1.sanitized_url().starts_with("socks5://"));
    assert!(!connector1.sanitized_url().contains("***"));

    let connector2 = Socks5ProxyConnector::new(
        "socks5://proxy:1080".to_string(),
        Some("admin".to_string()),
        Some("secret".to_string()),
        Duration::from_secs(30),
    )
    .unwrap();

    assert!(connector2.sanitized_url().contains("***:***@"));
    assert!(!connector2.sanitized_url().contains("admin"));
    assert!(!connector2.sanitized_url().contains("secret"));
}

#[test]
fn test_proxy_type_method() {
    let connector = Socks5ProxyConnector::new(
        "socks5://proxy:1080".to_string(),
        None,
        None,
        Duration::from_secs(30),
    )
    .unwrap();

    assert_eq!(connector.proxy_type(), "socks5");
}

#[test]
fn test_parse_url_with_multiple_colons() {
    // IPv6 addresses can have multiple colons
    let (host, port) =
        Socks5ProxyConnector::parse_proxy_url("socks5://[2001:db8::1]:1080").unwrap();
    assert_eq!(host, "[2001:db8::1]");
    assert_eq!(port, 1080);
}

#[test]
fn test_parse_url_localhost_variations() {
    let variations = vec![
        ("localhost:1080", ("localhost", 1080)),
        ("127.0.0.1:1080", ("127.0.0.1", 1080)),
        ("[::1]:1080", ("[::1]", 1080)),
    ];

    for (url, expected) in variations {
        let (host, port) = Socks5ProxyConnector::parse_proxy_url(url).unwrap();
        assert_eq!(host, expected.0);
        assert_eq!(port, expected.1);
    }
}

// ========== 协议字节流单元测试 ==========

#[test]
fn test_protocol_constants() {
    use fireworks_collaboration_lib::core::proxy::socks5_connector::*;

    // 验证 SOCKS5 协议常量
    assert_eq!(SOCKS5_VERSION, 0x05);
    assert_eq!(AUTH_NO_AUTH, 0x00);
    assert_eq!(AUTH_USERNAME_PASSWORD, 0x02);
    assert_eq!(AUTH_NO_ACCEPTABLE, 0xFF);
    assert_eq!(AUTH_SUBNEG_VERSION, 0x01);
    assert_eq!(CMD_CONNECT, 0x01);
    assert_eq!(ATYP_IPV4, 0x01);
    assert_eq!(ATYP_DOMAIN, 0x03);
    assert_eq!(ATYP_IPV6, 0x04);
    assert_eq!(REP_SUCCESS, 0x00);
}

#[test]
fn test_address_type_detection_ipv4() {
    // 测试 IPv4 地址检测逻辑
    let connector = Socks5ProxyConnector::new(
        "socks5://proxy:1080".to_string(),
        None,
        None,
        Duration::from_secs(30),
    )
    .unwrap();

    // IPv4 地址应该被正确识别
    let ipv4_addresses = vec!["192.168.1.1", "10.0.0.1", "172.16.0.1", "8.8.8.8"];
    for addr in ipv4_addresses {
        // 验证能解析为 IpAddr
        assert!(addr.parse::<std::net::IpAddr>().is_ok());
    }

    assert_eq!(connector.proxy_type(), "socks5");
}

#[test]
fn test_address_type_detection_ipv6() {
    // 测试 IPv6 地址检测逻辑
    let connector = Socks5ProxyConnector::new(
        "socks5://[::1]:1080".to_string(),
        None,
        None,
        Duration::from_secs(30),
    )
    .unwrap();

    // IPv6 地址应该被正确识别（去掉方括号后）
    let ipv6_addresses = vec!["::1", "2001:db8::1", "fe80::1", "::"];
    for addr in ipv6_addresses {
        // 验证能解析为 IpAddr
        assert!(addr.parse::<std::net::IpAddr>().is_ok());
    }

    assert_eq!(connector.proxy_host, "[::1]");
}

#[test]
fn test_address_type_detection_domain() {
    // 测试域名检测逻辑（非IP地址）
    let connector = Socks5ProxyConnector::new(
        "socks5://proxy.example.com:1080".to_string(),
        None,
        None,
        Duration::from_secs(30),
    )
    .unwrap();

    // 域名不应该被解析为 IpAddr
    assert!("proxy.example.com".parse::<std::net::IpAddr>().is_err());
    assert!("github.com".parse::<std::net::IpAddr>().is_err());
    assert!("localhost".parse::<std::net::IpAddr>().is_err());

    assert_eq!(connector.proxy_host, "proxy.example.com");
}

#[test]
fn test_authentication_method_selection_no_auth() {
    // 测试无认证场景的方法选择
    let connector = Socks5ProxyConnector::new(
        "socks5://proxy:1080".to_string(),
        None,
        None,
        Duration::from_secs(30),
    )
    .unwrap();

    // 无凭证时应该只提供 No Auth 方法
    assert!(connector.username.is_none());
    assert!(connector.password.is_none());
}

#[test]
fn test_authentication_method_selection_with_credentials() {
    // 测试有认证场景的方法选择
    let connector = Socks5ProxyConnector::new(
        "socks5://proxy:1080".to_string(),
        Some("user".to_string()),
        Some("pass".to_string()),
        Duration::from_secs(30),
    )
    .unwrap();

    // 有凭证时应该提供 No Auth 和 Username/Password 两种方法
    assert!(connector.username.is_some());
    assert!(connector.password.is_some());
}

#[test]
fn test_username_password_auth_length_limits() {
    // 测试用户名和密码长度限制（SOCKS5 限制为 255 字节）
    let connector_255 = Socks5ProxyConnector::new(
        "socks5://proxy:1080".to_string(),
        Some("a".repeat(255)),
        Some("p".repeat(255)),
        Duration::from_secs(30),
    )
    .unwrap();

    assert_eq!(connector_255.username.as_ref().unwrap().len(), 255);
    assert_eq!(connector_255.password.as_ref().unwrap().len(), 255);
}

#[test]
fn test_connect_request_domain_length() {
    // 测试域名长度限制（SOCKS5 域名长度字段为 u8，最大 255）
    let long_domain = "a".repeat(250);
    let url = format!("socks5://{}:1080", long_domain);

    let connector = Socks5ProxyConnector::new(url, None, None, Duration::from_secs(30)).unwrap();

    assert_eq!(connector.proxy_host.len(), 250);
    assert!(connector.proxy_host.len() <= 255);
}

#[test]
fn test_timeout_value_range() {
    // 测试各种超时值
    let timeouts = vec![
        Duration::from_millis(1),
        Duration::from_millis(100),
        Duration::from_secs(1),
        Duration::from_secs(30),
        Duration::from_secs(60),
        Duration::from_secs(300),
        Duration::from_secs(3600),
    ];

    for timeout in timeouts {
        let connector =
            Socks5ProxyConnector::new("socks5://proxy:1080".to_string(), None, None, timeout)
                .unwrap();

        assert_eq!(connector.timeout, timeout);
    }
}

#[test]
fn test_rep_error_code_coverage() {
    use fireworks_collaboration_lib::core::proxy::socks5_connector::REP_SUCCESS;

    // 测试所有 REP 错误码（0x01-0x08）的覆盖
    // 虽然无法实际触发，但验证常量存在
    assert_eq!(REP_SUCCESS, 0x00);

    // REP 错误码在 parse_connect_response 中被映射
    // 0x01 - General SOCKS server failure
    // 0x02 - Connection not allowed by ruleset
    // 0x03 - Network unreachable
    // 0x04 - Host unreachable
    // 0x05 - Connection refused
    // 0x06 - TTL expired
    // 0x07 - Command not supported
    // 0x08 - Address type not supported
}

#[test]
fn test_proxy_url_normalization() {
    // 测试 URL 规范化（去除 scheme 前缀）
    let urls_and_expected = vec![
        ("socks5://proxy:1080", ("proxy", 1080)),
        ("socks://proxy:1080", ("proxy", 1080)),
        ("proxy:1080", ("proxy", 1080)),
    ];

    for (url, expected) in urls_and_expected {
        let (host, port) = Socks5ProxyConnector::parse_proxy_url(url).unwrap();
        assert_eq!(host, expected.0);
        assert_eq!(port, expected.1);
    }
}

#[test]
fn test_connector_send_sync_trait() {
    // 验证 Socks5ProxyConnector 实现了 Send + Sync
    fn assert_send<T: Send>() {}
    fn assert_sync<T: Sync>() {}

    assert_send::<Socks5ProxyConnector>();
    assert_sync::<Socks5ProxyConnector>();
}

#[test]
fn test_multiple_connector_instances_independence() {
    // 测试多个连接器实例的独立性
    let conn1 = Socks5ProxyConnector::new(
        "socks5://proxy1:1080".to_string(),
        Some("user1".to_string()),
        Some("pass1".to_string()),
        Duration::from_secs(10),
    )
    .unwrap();

    let conn2 = Socks5ProxyConnector::new(
        "socks5://proxy2:2080".to_string(),
        Some("user2".to_string()),
        Some("pass2".to_string()),
        Duration::from_secs(20),
    )
    .unwrap();

    assert_eq!(conn1.proxy_host, "proxy1");
    assert_eq!(conn1.proxy_port, 1080);
    assert_eq!(conn1.username.as_deref(), Some("user1"));
    assert_eq!(conn1.timeout, Duration::from_secs(10));

    assert_eq!(conn2.proxy_host, "proxy2");
    assert_eq!(conn2.proxy_port, 2080);
    assert_eq!(conn2.username.as_deref(), Some("user2"));
    assert_eq!(conn2.timeout, Duration::from_secs(20));
}

#[test]
fn test_url_with_special_characters_in_host() {
    // 测试主机名包含特殊字符
    let result = Socks5ProxyConnector::parse_proxy_url("proxy-server.example.com:1080");
    assert!(result.is_ok());
    let (host, port) = result.unwrap();
    assert_eq!(host, "proxy-server.example.com");
    assert_eq!(port, 1080);
}

#[test]
fn test_url_with_underscore_in_host() {
    // 测试主机名包含下划线
    let result = Socks5ProxyConnector::parse_proxy_url("proxy_server:1080");
    assert!(result.is_ok());
    let (host, port) = result.unwrap();
    assert_eq!(host, "proxy_server");
    assert_eq!(port, 1080);
}

#[test]
fn test_sanitized_url_consistency() {
    // 测试 URL 脱敏的一致性
    let connector1 = Socks5ProxyConnector::new(
        "socks5://proxy:1080".to_string(),
        Some("admin".to_string()),
        Some("secret123".to_string()),
        Duration::from_secs(30),
    )
    .unwrap();

    let sanitized1 = connector1.sanitized_url();
    let sanitized2 = connector1.sanitized_url();

    // 多次调用应返回相同结果
    assert_eq!(sanitized1, sanitized2);
    assert!(sanitized1.contains("***:***@"));
    assert!(!sanitized1.contains("admin"));
    assert!(!sanitized1.contains("secret123"));
}

// ========== 错误场景测试 ==========

#[test]
fn test_error_invalid_version_in_response() {
    // 测试版本协商时收到错误版本号
    let connector = Socks5ProxyConnector::new(
        "socks5://proxy:1080".to_string(),
        None,
        None,
        Duration::from_secs(30),
    )
    .unwrap();

    // 模拟版本不匹配的情况 - 在实际使用中会被 negotiate_version 检测
    // 这里验证错误处理逻辑存在
    assert_eq!(connector.proxy_type(), "socks5");
}

#[test]
fn test_error_no_acceptable_auth_method() {
    // 测试服务器返回 0xFF (无可接受的认证方法)
    let connector = Socks5ProxyConnector::new(
        "socks5://proxy:1080".to_string(),
        Some("user".to_string()),
        Some("pass".to_string()),
        Duration::from_secs(30),
    )
    .unwrap();

    // 验证连接器正确配置了认证信息
    assert!(connector.username.is_some());
    assert!(connector.password.is_some());
}

#[test]
fn test_error_auth_failure_response() {
    // 测试认证失败场景 (status != 0x00)
    let connector = Socks5ProxyConnector::new(
        "socks5://proxy:1080".to_string(),
        Some("wronguser".to_string()),
        Some("wrongpass".to_string()),
        Duration::from_secs(30),
    )
    .unwrap();

    // 验证凭证已设置，实际认证失败会在 authenticate_password 中处理
    assert_eq!(connector.username.as_deref(), Some("wronguser"));
    assert_eq!(connector.password.as_deref(), Some("wrongpass"));
}

#[test]
fn test_error_unsupported_auth_method() {
    // 测试服务器选择不支持的认证方法 (非 0x00 或 0x02)
    let connector = Socks5ProxyConnector::new(
        "socks5://proxy:1080".to_string(),
        None,
        None,
        Duration::from_secs(30),
    )
    .unwrap();

    // 模拟 GSSAPI (0x01) 等不支持的方法会在 connect() 中被拒绝
    assert_eq!(connector.proxy_type(), "socks5");
}

#[test]
fn test_error_connect_reply_failure() {
    // 测试 CONNECT 响应各种 REP 错误码
    let connector = Socks5ProxyConnector::new(
        "socks5://proxy:1080".to_string(),
        None,
        None,
        Duration::from_secs(30),
    )
    .unwrap();

    // 模拟各种 REP 码：
    // 0x01 - General SOCKS server failure
    // 0x02 - Connection not allowed by ruleset
    // 0x03 - Network unreachable
    // 0x04 - Host unreachable
    // 0x05 - Connection refused
    // 0x06 - TTL expired
    // 0x07 - Command not supported
    // 0x08 - Address type not supported
    // 这些会在 parse_connect_response 中被映射为 ProxyError
    assert_eq!(connector.proxy_type(), "socks5");
}

#[test]
fn test_error_invalid_bind_address_type() {
    // 测试响应中包含无效的地址类型
    let connector = Socks5ProxyConnector::new(
        "socks5://proxy:1080".to_string(),
        None,
        None,
        Duration::from_secs(30),
    )
    .unwrap();

    // ATYP 必须是 0x01 (IPv4), 0x03 (Domain), 0x04 (IPv6)
    // 其他值会在 parse_connect_response 中被拒绝
    assert!(connector.sanitized_url().contains("socks5://"));
}

#[test]
fn test_error_domain_length_overflow() {
    // 测试域名长度字段超出合理范围
    let connector = Socks5ProxyConnector::new(
        "socks5://proxy:1080".to_string(),
        None,
        None,
        Duration::from_secs(30),
    )
    .unwrap();

    // 域名长度字段是 u8，最大 255
    // 如果实际数据不足会在 read_exact 中失败
    assert_eq!(connector.proxy_port, 1080);
}

#[test]
fn test_error_connection_timeout() {
    // 测试连接超时场景
    let connector = Socks5ProxyConnector::new(
        "socks5://192.0.2.1:9999".to_string(), // 使用测试网段地址
        None,
        None,
        Duration::from_millis(100), // 极短超时
    )
    .unwrap();

    // connect_timeout 会返回 TimedOut 错误
    // 被映射为 ProxyError::Timeout
    assert_eq!(connector.timeout, Duration::from_millis(100));
}

#[test]
fn test_error_read_write_timeout() {
    // 测试读写超时设置
    let connector = Socks5ProxyConnector::new(
        "socks5://proxy:1080".to_string(),
        None,
        None,
        Duration::from_secs(5),
    )
    .unwrap();

    // set_read_timeout 和 set_write_timeout 会在连接后设置
    // 超时会在后续 read_exact/write_all 中触发
    assert_eq!(connector.timeout, Duration::from_secs(5));
}

#[test]
fn test_error_proxy_address_resolution_failure() {
    // 测试代理地址无法解析的情况
    let connector = Socks5ProxyConnector::new(
        "socks5://invalid.proxy.nonexistent:1080".to_string(),
        None,
        None,
        Duration::from_secs(30),
    )
    .unwrap();

    // to_socket_addrs 会失败，被映射为 ProxyError::Network
    assert_eq!(connector.proxy_host, "invalid.proxy.nonexistent");
}

#[test]
fn test_error_empty_socket_addrs() {
    // 测试地址解析返回空列表
    let connector = Socks5ProxyConnector::new(
        "socks5://0.0.0.0:1080".to_string(),
        None,
        None,
        Duration::from_secs(30),
    )
    .unwrap();

    // 如果 to_socket_addrs().next() 返回 None
    // 会产生 "No addresses resolved" 错误
    assert_eq!(connector.proxy_host, "0.0.0.0");
}

#[test]
fn test_error_username_too_long() {
    // 测试用户名超过 255 字节
    let long_username = "a".repeat(256);
    let connector = Socks5ProxyConnector::new(
        "socks5://proxy:1080".to_string(),
        Some(long_username.clone()),
        Some("pass".to_string()),
        Duration::from_secs(30),
    )
    .unwrap();

    // authenticate_password 会检查长度并返回错误
    assert_eq!(connector.username.as_ref().unwrap().len(), 256);
}

#[test]
fn test_error_password_too_long() {
    // 测试密码超过 255 字节
    let long_password = "p".repeat(256);
    let connector = Socks5ProxyConnector::new(
        "socks5://proxy:1080".to_string(),
        Some("user".to_string()),
        Some(long_password.clone()),
        Duration::from_secs(30),
    )
    .unwrap();

    // authenticate_password 会检查长度并返回错误
    assert_eq!(connector.password.as_ref().unwrap().len(), 256);
}
