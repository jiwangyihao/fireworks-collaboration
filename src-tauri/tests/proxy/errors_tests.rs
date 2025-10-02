//! Tests for proxy error types

use fireworks_collaboration_lib::core::proxy::errors::ProxyError;

#[test]
fn test_proxy_error_display() {
    let error = ProxyError::network("Connection refused");
    assert_eq!(error.to_string(), "Network error: Connection refused");

    let error = ProxyError::auth("Invalid credentials");
    assert_eq!(error.to_string(), "Authentication error: Invalid credentials");

    let error = ProxyError::proxy("Bad gateway");
    assert_eq!(error.to_string(), "Proxy error: Bad gateway");

    let error = ProxyError::timeout("Connection timeout");
    assert_eq!(error.to_string(), "Timeout error: Connection timeout");

    let error = ProxyError::config("Invalid URL");
    assert_eq!(error.to_string(), "Configuration error: Invalid URL");
}

#[test]
fn test_proxy_error_category() {
    assert_eq!(ProxyError::network("test").category(), "network");
    assert_eq!(ProxyError::auth("test").category(), "auth");
    assert_eq!(ProxyError::proxy("test").category(), "proxy");
    assert_eq!(ProxyError::timeout("test").category(), "timeout");
    assert_eq!(ProxyError::config("test").category(), "config");
}

#[test]
fn test_proxy_error_equality() {
    let error1 = ProxyError::network("test");
    let error2 = ProxyError::network("test");
    let error3 = ProxyError::network("other");

    assert_eq!(error1, error2);
    assert_ne!(error1, error3);
}
