//! Proxy configuration types and parsing

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Proxy operating mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProxyMode {
    /// Proxy disabled, use direct connection
    Off,
    /// HTTP/HTTPS proxy (CONNECT method)
    Http,
    /// SOCKS5 proxy
    Socks5,
    /// Use system proxy settings (auto-detect)
    System,
}

impl Default for ProxyMode {
    fn default() -> Self {
        Self::Off
    }
}

impl std::fmt::Display for ProxyMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Off => write!(f, "off"),
            Self::Http => write!(f, "http"),
            Self::Socks5 => write!(f, "socks5"),
            Self::System => write!(f, "system"),
        }
    }
}

/// Proxy configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyConfig {
    /// Proxy mode
    #[serde(default)]
    pub mode: ProxyMode,
    
    /// Proxy server URL (e.g., "http://proxy.example.com:8080" or "socks5://127.0.0.1:1080")
    /// Only used when mode is Http or Socks5
    #[serde(default)]
    pub url: String,
    
    /// Optional username for proxy authentication
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    
    /// Optional password for proxy authentication
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
    
    /// Whether to disable custom transport layer when proxy is enabled
    /// When true and proxy is enabled, use libgit2 default HTTP transport instead of custom subtransport
    /// This also forces real SNI (disables Fake SNI) to reduce complexity and fingerprinting
    #[serde(default)]
    pub disable_custom_transport: bool,
    
    /// Connection timeout in seconds (default: 30)
    #[serde(default = "default_timeout_seconds")]
    pub timeout_seconds: u64,
    
    /// Fallback threshold: failure rate to trigger auto-fallback (0.0-1.0, default: 0.2)
    #[serde(default = "default_fallback_threshold")]
    pub fallback_threshold: f64,
    
    /// Fallback window in seconds for calculating failure rate (default: 300)
    #[serde(default = "default_fallback_window_seconds")]
    pub fallback_window_seconds: u64,
    
    /// Recovery cooldown in seconds before attempting recovery (default: 300)
    #[serde(default = "default_recovery_cooldown_seconds")]
    pub recovery_cooldown_seconds: u64,
    
    /// Health check interval in seconds (default: 60)
    #[serde(default = "default_health_check_interval_seconds")]
    pub health_check_interval_seconds: u64,
    
    /// Recovery strategy: "single" (one success), "consecutive" (multiple success), "rate" (success rate)
    #[serde(default = "default_recovery_strategy")]
    pub recovery_strategy: String,
    
    /// Health check probe URL (default: "www.github.com:443")
    /// Target host:port to probe for proxy availability
    #[serde(default = "default_probe_url")]
    pub probe_url: String,
    
    /// Health check probe timeout in seconds (default: 10)
    /// Should be shorter than connection timeout
    #[serde(default = "default_probe_timeout_seconds")]
    pub probe_timeout_seconds: u64,
    
    /// Number of consecutive successes required for recovery (default: 3)
    /// Used by "consecutive" recovery strategy
    #[serde(default = "default_recovery_consecutive_threshold")]
    pub recovery_consecutive_threshold: u32,
    
    /// Enable debug-level proxy logging (default: false)
    /// When true, outputs detailed connection info including sanitized URLs, auth status, timing
    #[serde(default)]
    pub debug_proxy_logging: bool,
}

pub fn default_timeout_seconds() -> u64 {
    30
}

pub fn default_fallback_threshold() -> f64 {
    0.2 // 20% failure rate
}

pub fn default_fallback_window_seconds() -> u64 {
    300 // 5 minutes
}

pub fn default_recovery_cooldown_seconds() -> u64 {
    300 // 5 minutes
}

pub fn default_health_check_interval_seconds() -> u64 {
    60 // 1 minute
}

pub fn default_recovery_strategy() -> String {
    "consecutive".to_string()
}

pub fn default_probe_url() -> String {
    "www.github.com:443".to_string()
}

pub fn default_probe_timeout_seconds() -> u64 {
    10 // 10 seconds, shorter than connection timeout
}

pub fn default_recovery_consecutive_threshold() -> u32 {
    3 // Require 3 consecutive successes
}

impl Default for ProxyConfig {
    fn default() -> Self {
        Self {
            mode: ProxyMode::Off,
            url: String::new(),
            username: None,
            password: None,
            disable_custom_transport: false,
            timeout_seconds: default_timeout_seconds(),
            fallback_threshold: default_fallback_threshold(),
            fallback_window_seconds: default_fallback_window_seconds(),
            recovery_cooldown_seconds: default_recovery_cooldown_seconds(),
            health_check_interval_seconds: default_health_check_interval_seconds(),
            recovery_strategy: default_recovery_strategy(),
            probe_url: default_probe_url(),
            probe_timeout_seconds: default_probe_timeout_seconds(),
            recovery_consecutive_threshold: default_recovery_consecutive_threshold(),
            debug_proxy_logging: false,
        }
    }
}

impl ProxyConfig {
    /// Get connection timeout as Duration
    pub fn timeout(&self) -> Duration {
        Duration::from_secs(self.timeout_seconds)
    }
    
    /// Check if proxy is enabled:
    /// - Off: false
    /// - Http/Socks5: URL非空才启用
    /// - System: 允许空URL
    pub fn is_enabled(&self) -> bool {
        match self.mode {
            ProxyMode::Off => false,
            ProxyMode::Http | ProxyMode::Socks5 => !self.url.trim().is_empty(),
            ProxyMode::System => true,
        }
    }
    
    /// Validate configuration
    pub fn validate(&self) -> anyhow::Result<()> {
        match self.mode {
            ProxyMode::Off => {
                // No validation needed for disabled proxy
                Ok(())
            }
            ProxyMode::System => {
                // System mode doesn't need URL validation
                // But still validate other parameters
                self.validate_timeouts()?;
                self.validate_thresholds()?;
                Ok(())
            }
            ProxyMode::Http | ProxyMode::Socks5 => {
                // URL is required
                if self.url.is_empty() {
                    anyhow::bail!("Proxy URL is required when mode is {} ", self.mode);
                }
                
                // Validate URL format
                self.validate_url()?;
                
                // Validate port if present
                self.validate_port()?;
                
                // Validate timeouts
                self.validate_timeouts()?;
                
                // Validate thresholds
                self.validate_thresholds()?;
                
                // Validate recovery strategy
                self.validate_recovery_strategy()?;
                
                // Validate probe URL
                self.validate_probe_url()?;
                
                // Validate probe timeout
                self.validate_probe_timeout()?;
                
                // Validate recovery threshold
                self.validate_recovery_threshold()?;
                
                Ok(())
            }
        }
    }
    
    /// Validate proxy URL format
    fn validate_url(&self) -> anyhow::Result<()> {
        if !self.url.starts_with("http://") 
            && !self.url.starts_with("https://") 
            && !self.url.starts_with("socks5://") {
            anyhow::bail!("Invalid proxy URL: must start with http://, https://, or socks5://");
        }
        
        // Additional URL validation: check for valid characters
        if self.url.contains(' ') {
            anyhow::bail!("Invalid proxy URL: contains whitespace");
        }
        
        Ok(())
    }
    
    /// Validate port number in URL if present
    fn validate_port(&self) -> anyhow::Result<()> {
        // Try to extract port from URL
        if let Some(url_after_scheme) = self.url.strip_prefix("http://")
            .or_else(|| self.url.strip_prefix("https://"))
            .or_else(|| self.url.strip_prefix("socks5://")) {
            
            // Remove credentials if present
            let host_part = if let Some(at_pos) = url_after_scheme.find('@') {
                &url_after_scheme[at_pos + 1..]
            } else {
                url_after_scheme
            };
            
            // Check if port is specified
            if let Some(colon_pos) = host_part.rfind(':') {
                let port_str = &host_part[colon_pos + 1..];
                // Remove path if present
                let port_str = port_str.split('/').next().unwrap_or(port_str);
                
                if let Ok(port) = port_str.parse::<u16>() {
                    // Valid port range: 1-65535
                    if port == 0 {
                        anyhow::bail!("Invalid proxy port: must be between 1 and 65535");
                    }
                } else if !port_str.is_empty() {
                    anyhow::bail!("Invalid proxy port: not a valid number");
                }
            }
        }
        
        Ok(())
    }
    
    /// Validate timeout values
    fn validate_timeouts(&self) -> anyhow::Result<()> {
        // Connection timeout: reasonable range is 1-300 seconds (5 minutes)
        if self.timeout_seconds == 0 {
            anyhow::bail!("Timeout must be greater than 0");
        }
        if self.timeout_seconds > 300 {
            anyhow::bail!("Timeout must not exceed 300 seconds (5 minutes)");
        }
        
        // Fallback window: must be at least 10 seconds
        if self.fallback_window_seconds < 10 {
            anyhow::bail!("Fallback window must be at least 10 seconds");
        }
        if self.fallback_window_seconds > 3600 {
            anyhow::bail!("Fallback window must not exceed 3600 seconds (1 hour)");
        }
        
        // Recovery cooldown: must be at least 10 seconds
        if self.recovery_cooldown_seconds < 10 {
            anyhow::bail!("Recovery cooldown must be at least 10 seconds");
        }
        if self.recovery_cooldown_seconds > 3600 {
            anyhow::bail!("Recovery cooldown must not exceed 3600 seconds (1 hour)");
        }
        
        // Health check interval: must be at least 10 seconds
        if self.health_check_interval_seconds < 10 {
            anyhow::bail!("Health check interval must be at least 10 seconds");
        }
        if self.health_check_interval_seconds > 3600 {
            anyhow::bail!("Health check interval must not exceed 3600 seconds (1 hour)");
        }
        
        // Probe timeout: must be reasonable
        if self.probe_timeout_seconds == 0 {
            anyhow::bail!("probeTimeoutSeconds: Probe timeout must be greater than 0");
        }
        if self.probe_timeout_seconds > self.timeout_seconds {
            tracing::warn!(
                "Probe timeout ({}s) is greater than connection timeout ({}s), will be capped",
                self.probe_timeout_seconds,
                self.timeout_seconds
            );
        }
        
        Ok(())
    }
    
    /// Validate threshold values
    fn validate_thresholds(&self) -> anyhow::Result<()> {
        // Fallback threshold: must be between 0.0 and 1.0
        if !(0.0..=1.0).contains(&self.fallback_threshold) {
            anyhow::bail!("Fallback threshold must be between 0.0 and 1.0");
        }
        
        // Additional check: threshold should be meaningful (not too small)
        if self.fallback_threshold < 0.1 && self.fallback_threshold > 0.0 {
            tracing::warn!(
                "Fallback threshold {} is very low, may trigger frequently",
                self.fallback_threshold
            );
        }
        
        Ok(())
    }
    
    /// Validate recovery strategy
    fn validate_recovery_strategy(&self) -> anyhow::Result<()> {
        // Check if strategy is one of the supported values
        match self.recovery_strategy.as_str() {
            "immediate" | "consecutive" | "exponential-backoff" => Ok(()),
            other => {
                anyhow::bail!(
                    "Invalid recovery strategy '{}'. Must be one of: immediate, consecutive, exponential-backoff",
                    other
                )
            }
        }
    }
    
    /// Validate probe URL format
    fn validate_probe_url(&self) -> anyhow::Result<()> {
        // Probe URL should be in format "host:port"
        if self.probe_url.is_empty() {
            anyhow::bail!("probeUrl: Probe URL cannot be empty");
        }
        
        // Check format: must contain ':'
        if !self.probe_url.contains(':') {
            anyhow::bail!("probeUrl: Probe URL must be in format 'host:port'");
        }
        
        // Parse and validate port
        if let Some(colon_pos) = self.probe_url.rfind(':') {
            let port_str = &self.probe_url[colon_pos + 1..];
            
            match port_str.parse::<u16>() {
                Ok(port) if port > 0 => Ok(()),
                Ok(_) => anyhow::bail!("probeUrl: Probe URL port must be between 1 and 65535"),
                Err(_) => anyhow::bail!("probeUrl: Probe URL port is not a valid number: {}", port_str),
            }
        } else {
            Ok(())
        }
    }
    
    /// Validate probe timeout
    fn validate_probe_timeout(&self) -> anyhow::Result<()> {
        if self.probe_timeout_seconds == 0 {
            anyhow::bail!("probeTimeoutSeconds: Probe timeout must be greater than 0");
        }
        
        if self.probe_timeout_seconds > 60 {
            anyhow::bail!("probeTimeoutSeconds: Probe timeout must not exceed 60 seconds");
        }
        
        // Warn if probe timeout is too close to connection timeout
        if self.probe_timeout_seconds > self.timeout_seconds * 80 / 100 {
            tracing::warn!(
                "Probe timeout ({}s) is close to connection timeout ({}s), may not provide benefit",
                self.probe_timeout_seconds,
                self.timeout_seconds
            );
        }
        
        Ok(())
    }
    
    /// Validate recovery consecutive threshold
    fn validate_recovery_threshold(&self) -> anyhow::Result<()> {
        if self.recovery_consecutive_threshold == 0 {
            anyhow::bail!("recoveryConsecutiveThreshold: Recovery consecutive threshold must be at least 1");
        }
        
        if self.recovery_consecutive_threshold > 10 {
            anyhow::bail!("recoveryConsecutiveThreshold: Recovery consecutive threshold must not exceed 10");
        }
        
        // Warn if threshold is 1 with consecutive strategy
        if self.recovery_consecutive_threshold == 1 && self.recovery_strategy == "consecutive" {
            tracing::warn!(
                "Recovery consecutive threshold is 1, consider using 'immediate' strategy instead"
            );
        }
        
        Ok(())
    }
    
    /// Get sanitized URL for logging (hide password if present)
    pub fn sanitized_url(&self) -> String {
        if self.url.is_empty() {
            return String::new();
        }
        
        // Simple sanitization: if URL contains '@', hide the credentials part
        if let Some(at_pos) = self.url.find('@') {
            if let Some(scheme_end) = self.url.find("://") {
                let scheme = &self.url[..=scheme_end + 2];
                let host_part = &self.url[at_pos..];
                return format!("{}***{}", scheme, host_part);
            }
        }
        
        self.url.clone()
    }
}
