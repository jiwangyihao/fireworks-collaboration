//! System proxy detection for Windows, macOS, and Linux
//!
//! This module provides cross-platform detection of system proxy settings.

use super::{ProxyConfig, ProxyMode};

/// System proxy detector
pub struct SystemProxyDetector;

impl SystemProxyDetector {
    /// Detect system proxy configuration
    /// 
    /// Returns `Some(ProxyConfig)` if a system proxy is detected and can be parsed,
    /// `None` if no proxy is configured or detection fails.
    pub fn detect() -> Option<ProxyConfig> {
        // Try platform-specific detection first
        #[cfg(target_os = "windows")]
        {
            if let Some(config) = Self::detect_windows() {
                return Some(config);
            }
        }
        
        #[cfg(target_os = "macos")]
        {
            if let Some(config) = Self::detect_macos() {
                return Some(config);
            }
        }
        
        // Fall back to environment variable detection (works on all platforms)
        Self::detect_from_env()
    }
    
    /// Detect proxy from environment variables (Linux and fallback for other platforms)
    /// 
    /// Reuses the logic from tls::util::proxy_present() but also extracts the URL
    pub fn detect_from_env() -> Option<ProxyConfig> {
        let keys = [
            "HTTPS_PROXY",
            "https_proxy",
            "HTTP_PROXY",
            "http_proxy",
            "ALL_PROXY",
            "all_proxy",
        ];
        
        for key in keys {
            if let Ok(value) = std::env::var(key) {
                let trimmed = value.trim();
                if !trimmed.is_empty() {
                    return Self::parse_proxy_url(trimmed);
                }
            }
        }
        
        None
    }
    
    /// Detect proxy from Windows registry
    #[cfg(target_os = "windows")]
    fn detect_windows() -> Option<ProxyConfig> {
        use winreg::enums::*;
        use winreg::RegKey;
        
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let internet_settings = hkcu
            .open_subkey("Software\\Microsoft\\Windows\\CurrentVersion\\Internet Settings")
            .ok()?;
        
        // Check if proxy is enabled
        let proxy_enable: u32 = internet_settings.get_value("ProxyEnable").ok()?;
        if proxy_enable == 0 {
            tracing::debug!("Windows proxy is disabled (ProxyEnable=0)");
            return None;
        }
        
        // Get proxy server
        let proxy_server: String = internet_settings.get_value("ProxyServer").ok()?;
        if proxy_server.trim().is_empty() {
            tracing::debug!("Windows proxy server is empty");
            return None;
        }
        
        tracing::info!("Detected Windows system proxy: {}", proxy_server);
        Self::parse_proxy_url(&proxy_server)
    }
    
    /// Detect proxy from macOS scutil
    #[cfg(target_os = "macos")]
    fn detect_macos() -> Option<ProxyConfig> {
        use std::process::Command;
        
        let output = Command::new("scutil")
            .arg("--proxy")
            .output()
            .ok()?;
        
        if !output.status.success() {
            tracing::debug!("scutil --proxy failed with status: {}", output.status);
            return None;
        }
        
        let stdout = String::from_utf8_lossy(&output.stdout);
        
        // Parse scutil output for HTTPEnable/HTTPProxy/HTTPPort or HTTPSEnable/HTTPSProxy/HTTPSPort
        let lines: Vec<&str> = stdout.lines().collect();
        
        // Try HTTPS proxy first
        if let Some(config) = Self::parse_scutil_output(&lines, "HTTPS") {
            tracing::info!("Detected macOS HTTPS system proxy: {}", config.sanitized_url());
            return Some(config);
        }
        
        // Fall back to HTTP proxy
        if let Some(config) = Self::parse_scutil_output(&lines, "HTTP") {
            tracing::info!("Detected macOS HTTP system proxy: {}", config.sanitized_url());
            return Some(config);
        }
        
        // Try SOCKS proxy
        if let Some(config) = Self::parse_scutil_output(&lines, "SOCKS") {
            tracing::info!("Detected macOS SOCKS system proxy: {}", config.sanitized_url());
            return Some(config);
        }
        
        None
    }
    
    /// Parse scutil output for a specific proxy type (HTTP/HTTPS/SOCKS)
    #[cfg(target_os = "macos")]
    pub fn parse_scutil_output(lines: &[&str], proxy_type: &str) -> Option<ProxyConfig> {
        let enable_key = format!("{}Enable", proxy_type);
        let proxy_key = format!("{}Proxy", proxy_type);
        let port_key = format!("{}Port", proxy_type);
        
        // Check if enabled
        let enabled = lines
            .iter()
            .find(|line| line.contains(&enable_key))
            .and_then(|line| line.split(':').nth(1))
            .and_then(|val| val.trim().parse::<u32>().ok())
            .unwrap_or(0);
        
        if enabled == 0 {
            return None;
        }
        
        // Extract proxy host
        let host = lines
            .iter()
            .find(|line| line.contains(&proxy_key) && !line.contains("Port"))
            .and_then(|line| line.split(':').nth(1))
            .map(|s| s.trim().to_string())?;
        
        // Extract port
        let port = lines
            .iter()
            .find(|line| line.contains(&port_key))
            .and_then(|line| line.split(':').nth(1))
            .and_then(|val| val.trim().parse::<u16>().ok())
            .unwrap_or(8080); // Default port
        
        // Build URL
        let scheme = match proxy_type {
            "HTTPS" => "https",
            "SOCKS" => "socks5",
            _ => "http",
        };
        
        let url = format!("{}://{}:{}", scheme, host, port);
        Self::parse_proxy_url(&url)
    }
    
    /// Parse proxy URL and determine proxy type
    pub fn parse_proxy_url(url: &str) -> Option<ProxyConfig> {
        let url = url.trim();
        
        // Determine proxy mode from URL scheme
        let mode = if url.starts_with("socks5://") || url.starts_with("socks://") {
            ProxyMode::Socks5
        } else if url.starts_with("http://") || url.starts_with("https://") {
            ProxyMode::Http
        } else {
            // If no scheme, assume HTTP and add it
            return Self::parse_proxy_url(&format!("http://{}", url));
        };
        
        // Basic validation: ensure there's a host part after the scheme
        if let Some(host_part) = url.split("://").nth(1) {
            if host_part.is_empty() {
                tracing::warn!("Invalid proxy URL (empty host): {}", url);
                return None;
            }
        } else {
            tracing::warn!("Invalid proxy URL format: {}", url);
            return None;
        }
        
        let config = ProxyConfig {
            mode,
            url: url.to_string(),
            ..Default::default()
        };
        
        // Validate the config
        if let Err(e) = config.validate() {
            tracing::warn!("Failed to validate detected proxy config: {}", e);
            return None;
        }
        
        Some(config)
    }
}
