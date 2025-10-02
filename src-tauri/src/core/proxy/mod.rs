//! Proxy module for supporting HTTP/HTTPS/SOCKS5 proxies with automatic fallback
//!
//! This module provides:
//! - Proxy configuration and state management
//! - System proxy detection (Windows/macOS/Linux)
//! - Proxy connector trait for unified interface
//! - Automatic fallback and recovery mechanisms (P5.4: fallback, P5.5: recovery)
//! - Failure detection with sliding window statistics (P5.4)

pub mod config;
pub mod detector;
pub mod errors;
pub mod events;
pub mod http_connector;
pub mod manager;
pub mod socks5_connector;
pub mod state;
pub mod system_detector;

pub use config::{ProxyConfig, ProxyMode};
pub use detector::{FailureStats, ProxyFailureDetector};
pub use errors::ProxyError;
pub use events::{
    ProxyFallbackEvent, ProxyHealthCheckEvent, ProxyRecoveredEvent, ProxyStateEvent,
};
pub use http_connector::HttpProxyConnector;
pub use manager::ProxyManager;
pub use socks5_connector::Socks5ProxyConnector;
pub use state::{ProxyState, ProxyStateContext, StateTransition};
pub use system_detector::SystemProxyDetector;

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
    /// A TCP stream connected through the proxy, or a ProxyError
    fn connect(&self, host: &str, port: u16) -> Result<TcpStream, ProxyError>;
    
    /// Get the proxy type name for logging
    fn proxy_type(&self) -> &str {
        "unknown"
    }
}

/// Placeholder proxy connector that always falls back to direct connection
/// 
/// This is used in P5.0 to establish the interface without implementing
/// actual proxy logic. Real implementations will be added in P5.1/P5.2.
pub struct PlaceholderConnector;

impl ProxyConnector for PlaceholderConnector {
    fn connect(&self, host: &str, port: u16) -> Result<TcpStream, ProxyError> {
        tracing::debug!("PlaceholderConnector: falling back to direct connection to {host}:{port}");
        TcpStream::connect((host, port))
            .map_err(|e| ProxyError::network(format!("Direct connection failed: {e}")))
    }
    
    fn proxy_type(&self) -> &str {
        "placeholder"
    }
}
