//! Shared types for the Tauri application.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::core::{
    config::model::AppConfig,
    ip_pool::IpPool,
    proxy::ProxyManager,
    tasks::SharedTaskRegistry,
};

// ===== OAuth Types =====

/// OAuth callback data received from the authorization server.
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct OAuthCallbackData {
    pub code: Option<String>,
    pub state: Option<String>,
    pub error: Option<String>,
    pub error_description: Option<String>,
}

/// Shared state for OAuth callback data.
pub type OAuthState = Arc<Mutex<Option<OAuthCallbackData>>>;

// ===== System Proxy Types =====

/// System proxy configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemProxy {
    pub enabled: bool,
    pub host: String,
    pub port: u16,
    pub bypass: String,
}

impl Default for SystemProxy {
    fn default() -> Self {
        Self {
            enabled: false,
            host: String::new(),
            port: 0,
            bypass: String::new(),
        }
    }
}

/// System proxy detection result.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemProxyResult {
    pub url: Option<String>,
    #[serde(rename = "type")]
    pub proxy_type: Option<String>,
}

// ===== Application State Type Aliases =====

/// Shared application configuration.
pub type SharedConfig = Arc<Mutex<AppConfig>>;

/// Configuration base directory path.
pub type ConfigBaseDir = PathBuf;

/// Shared task registry.
pub type TaskRegistryState = SharedTaskRegistry;

/// Shared IP pool.
pub type SharedIpPool = Arc<Mutex<IpPool>>;

/// Shared proxy manager.
pub type SharedProxyManager = Arc<Mutex<ProxyManager>>;
