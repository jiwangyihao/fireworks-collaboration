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
