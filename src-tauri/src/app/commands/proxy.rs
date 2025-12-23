//! Proxy management commands.

use tauri::State;

use crate::core::proxy::{ProxyManager, ProxyMode, SystemProxyDetector};

use super::super::types::{SharedConfig, SystemProxy, SystemProxyResult};

/// Detect system proxy settings.
///
/// Returns the configured system proxy (if any) including its type and URL.
#[tauri::command(rename_all = "camelCase")]
pub async fn detect_system_proxy() -> Result<SystemProxyResult, String> {
    tracing::info!(target = "proxy", "Detecting system proxy");

    match SystemProxyDetector::detect() {
        Some(config) => {
            let proxy_type = match config.mode {
                ProxyMode::Http => Some("http".to_string()),
                ProxyMode::Socks5 => Some("socks5".to_string()),
                ProxyMode::System => Some("system".to_string()),
                ProxyMode::Off => None,
            };

            tracing::info!(
                target = "proxy",
                url = %config.url,
                proxy_type = ?proxy_type,
                "System proxy detected"
            );

            Ok(SystemProxyResult {
                url: Some(config.url.clone()),
                proxy_type,
            })
        }
        None => {
            tracing::info!(target = "proxy", "No system proxy detected");
            Ok(SystemProxyResult {
                url: None,
                proxy_type: None,
            })
        }
    }
}

/// Force proxy fallback to direct connection.
///
/// Triggers an immediate fallback from proxy to direct connection,
/// optionally with a custom reason message.
#[tauri::command(rename_all = "camelCase")]
pub async fn force_proxy_fallback(
    reason: Option<String>,
    cfg: State<'_, SharedConfig>,
) -> Result<bool, String> {
    tracing::info!(target = "proxy", "Force proxy fallback requested");

    let config = cfg
        .lock()
        .map_err(|e| format!("Failed to lock config: {}", e))?;

    let mut manager = ProxyManager::new(config.proxy.clone());

    let reason = reason.unwrap_or_else(|| "Manual fallback triggered".to_string());

    manager
        .force_fallback(&reason)
        .map_err(|e| format!("Failed to trigger fallback: {}", e))?;

    tracing::info!(target = "proxy", reason = %reason, "Proxy fallback triggered");

    Ok(true)
}

/// Force proxy recovery from fallback.
///
/// Attempts to restore proxy functionality after a fallback.
#[tauri::command(rename_all = "camelCase")]
pub async fn force_proxy_recovery(cfg: State<'_, SharedConfig>) -> Result<bool, String> {
    tracing::info!(target = "proxy", "Force proxy recovery requested");

    let config = cfg
        .lock()
        .map_err(|e| format!("Failed to lock config: {}", e))?;

    let mut manager = ProxyManager::new(config.proxy.clone());

    manager
        .force_recovery()
        .map_err(|e| format!("Failed to trigger recovery: {}", e))?;

    tracing::info!(target = "proxy", "Proxy recovery triggered");

    Ok(true)
}

/// Get system proxy configuration (legacy command).
///
/// This is a legacy command that returns basic system proxy information.
/// Consider using `detect_system_proxy` for more detailed information.
#[tauri::command(rename_all = "camelCase")]
pub fn get_system_proxy() -> Result<SystemProxy, String> {
    #[cfg(windows)]
    {
        // Windows implementation would go here
        // For now, return default
        Ok(SystemProxy::default())
    }

    #[cfg(not(windows))]
    {
        // Unix-like implementation would go here
        // For now, return default
        Ok(SystemProxy::default())
    }
}
