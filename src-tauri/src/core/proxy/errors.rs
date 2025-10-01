//! Proxy error types for classification and handling

use std::fmt;

/// Proxy-specific error types for better error classification
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProxyError {
    /// Network connectivity error (DNS resolution, connection refused, timeout)
    Network(String),
    
    /// Authentication error (407, invalid credentials)
    Auth(String),
    
    /// Proxy server error (5xx responses, protocol errors)
    Proxy(String),
    
    /// Connection timeout
    Timeout(String),
    
    /// Invalid configuration
    Config(String),
}

impl fmt::Display for ProxyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProxyError::Network(msg) => write!(f, "Network error: {msg}"),
            ProxyError::Auth(msg) => write!(f, "Authentication error: {msg}"),
            ProxyError::Proxy(msg) => write!(f, "Proxy error: {msg}"),
            ProxyError::Timeout(msg) => write!(f, "Timeout error: {msg}"),
            ProxyError::Config(msg) => write!(f, "Configuration error: {msg}"),
        }
    }
}

impl std::error::Error for ProxyError {}

impl ProxyError {
    /// Create a network error
    pub fn network(msg: impl Into<String>) -> Self {
        ProxyError::Network(msg.into())
    }

    /// Create an authentication error
    pub fn auth(msg: impl Into<String>) -> Self {
        ProxyError::Auth(msg.into())
    }

    /// Create a proxy server error
    pub fn proxy(msg: impl Into<String>) -> Self {
        ProxyError::Proxy(msg.into())
    }

    /// Create a timeout error
    pub fn timeout(msg: impl Into<String>) -> Self {
        ProxyError::Timeout(msg.into())
    }

    /// Create a configuration error
    pub fn config(msg: impl Into<String>) -> Self {
        ProxyError::Config(msg.into())
    }

    /// Get error category for logging
    pub fn category(&self) -> &'static str {
        match self {
            ProxyError::Network(_) => "network",
            ProxyError::Auth(_) => "auth",
            ProxyError::Proxy(_) => "proxy",
            ProxyError::Timeout(_) => "timeout",
            ProxyError::Config(_) => "config",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
