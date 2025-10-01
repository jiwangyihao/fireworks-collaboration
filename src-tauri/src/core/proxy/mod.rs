//! Proxy module for supporting HTTP/HTTPS/SOCKS5 proxies with automatic fallback
//!
//! This module provides:
//! - Proxy configuration and state management
//! - System proxy detection (Windows/macOS/Linux)
//! - Proxy connector trait for unified interface
//! - Automatic fallback and recovery mechanisms (to be implemented in P5.4/P5.5)

pub mod config;
pub mod errors;
pub mod events;
pub mod http_connector;
pub mod manager;
pub mod state;
pub mod system_detector;

pub use config::{ProxyConfig, ProxyMode};
pub use errors::ProxyError;
pub use events::{
    ProxyFallbackEvent, ProxyHealthCheckEvent, ProxyRecoveredEvent, ProxyStateEvent,
};
pub use http_connector::HttpProxyConnector;
pub use manager::ProxyManager;
pub use state::{ProxyState, ProxyStateContext, StateTransition};
pub use system_detector::SystemProxyDetector;

use anyhow::Result;
use std::net::TcpStream;

/// Trait for proxy connectors
/// 
/// This trait defines the interface for different proxy types (HTTP/SOCKS5).
/// Implementations will be added in P5.1 and P5.2.
pub trait ProxyConnector: Send + Sync {
    /// Connect to the target host through the proxy
    /// 
    /// # Arguments
    /// * `host` - Target host to connect to
    /// * `port` - Target port to connect to
    /// 
    /// # Returns
    /// A TCP stream connected through the proxy, or an error
    fn connect(&self, host: &str, port: u16) -> Result<TcpStream>;
    
    /// Get the proxy type name for logging
    fn proxy_type(&self) -> &str;
}

/// Placeholder proxy connector that always falls back to direct connection
/// 
/// This is used in P5.0 to establish the interface without implementing
/// actual proxy logic. Real implementations will be added in P5.1/P5.2.
pub struct PlaceholderConnector;

impl ProxyConnector for PlaceholderConnector {
    fn connect(&self, host: &str, port: u16) -> Result<TcpStream> {
        tracing::debug!("PlaceholderConnector: falling back to direct connection to {host}:{port}");
        TcpStream::connect((host, port))
            .map_err(|e| anyhow::anyhow!("Direct connection failed: {}", e))
    }
    
    fn proxy_type(&self) -> &str {
        "placeholder"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_placeholder_connector() {
        let connector = PlaceholderConnector;
        assert_eq!(connector.proxy_type(), "placeholder");
        
        // Test connecting to a well-known host (will fail in CI but tests the interface)
        // This is just to verify the trait implementation compiles
        let _ = connector.connect("example.com", 80);
    }
}
