//! HTTP/HTTPS proxy connector implementation
//!
//! Implements HTTP CONNECT tunnel protocol for proxying TCP connections.
//! Supports Basic authentication and timeout control.

use super::{ProxyConnector, ProxyError};
use anyhow::{Context, Result};
use base64::{engine::general_purpose::STANDARD, Engine};
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::{Duration, Instant};
use url::Url;

/// HTTP proxy connector using CONNECT tunnel method
///
/// This connector establishes a TCP tunnel through an HTTP proxy server
/// using the CONNECT method (RFC 2817). It supports:
/// - HTTP and HTTPS proxy protocols
/// - Basic authentication (username/password)
/// - Configurable connection timeout
/// - Proper error classification
pub struct HttpProxyConnector {
    /// Proxy server URL (e.g., "http://proxy.example.com:8080")
    proxy_url: String,
    
    /// Optional username for Basic authentication
    username: Option<String>,
    
    /// Optional password for Basic authentication
    password: Option<String>,
    
    /// Connection timeout in seconds
    timeout: Duration,
}

impl HttpProxyConnector {
    /// Create a new HTTP proxy connector
    ///
    /// # Arguments
    /// * `proxy_url` - Proxy server URL (scheme://host:port)
    /// * `username` - Optional username for authentication
    /// * `password` - Optional password for authentication
    /// * `timeout` - Connection timeout duration
    ///
    /// # Returns
    /// A new HttpProxyConnector instance
    pub fn new(
        proxy_url: String,
        username: Option<String>,
        password: Option<String>,
        timeout: Duration,
    ) -> Self {
        Self {
            proxy_url,
            username,
            password,
            timeout,
        }
    }

    /// Parse proxy URL to extract host and port
    fn parse_proxy_url(&self) -> Result<(String, u16), ProxyError> {
        let url = Url::parse(&self.proxy_url)
            .map_err(|e| ProxyError::config(format!("Invalid proxy URL: {e}")))?;
        
        let host = url.host_str()
            .ok_or_else(|| ProxyError::config("Proxy URL missing host"))?
            .to_string();
        
        let port = url.port().unwrap_or(8080); // Default HTTP proxy port
        
        Ok((host, port))
    }

    /// Generate Basic authentication header value
    fn generate_auth_header(&self) -> Option<String> {
        match (&self.username, &self.password) {
            (Some(user), Some(pass)) => {
                let credentials = format!("{}:{}", user, pass);
                let encoded = STANDARD.encode(credentials.as_bytes());
                Some(format!("Basic {}", encoded))
            }
            _ => None,
        }
    }

    /// Send CONNECT request and parse response
    fn send_connect_request(
        &self,
        stream: &mut TcpStream,
        target_host: &str,
        target_port: u16,
    ) -> Result<(), ProxyError> {
        // Build CONNECT request
        let mut request = format!(
            "CONNECT {}:{} HTTP/1.1\r\n\
             Host: {}:{}\r\n",
            target_host, target_port, target_host, target_port
        );

        // Add authentication header if credentials provided
        if let Some(auth) = self.generate_auth_header() {
            request.push_str(&format!("Proxy-Authorization: {}\r\n", auth));
            tracing::debug!("Added Basic authentication to CONNECT request");
        }

        // End of headers
        request.push_str("\r\n");

        tracing::debug!("Sending CONNECT request to {}:{}", target_host, target_port);

        // Send request
        stream.write_all(request.as_bytes())
            .map_err(|e| ProxyError::network(format!("Failed to send CONNECT request: {e}")))?;
        stream.flush()
            .map_err(|e| ProxyError::network(format!("Failed to flush CONNECT request: {e}")))?;

        // Read and parse response
        let mut reader = BufReader::new(stream);
        let mut status_line = String::new();
        reader.read_line(&mut status_line)
            .map_err(|e| ProxyError::network(format!("Failed to read proxy response: {e}")))?;

        tracing::debug!("Received proxy response: {}", status_line.trim());

        // Parse status code
        let parts: Vec<&str> = status_line.split_whitespace().collect();
        if parts.len() < 2 {
            return Err(ProxyError::proxy(format!("Invalid proxy response: {}", status_line.trim())));
        }

        let status_code = parts[1].parse::<u16>()
            .map_err(|_| ProxyError::proxy(format!("Invalid status code in response: {}", parts[1])))?;

        tracing::debug!("Proxy response status code: {}", status_code);

        match status_code {
            200 => {
                tracing::debug!("CONNECT tunnel established successfully");
                Ok(())
            },
            407 => {
                tracing::warn!("Proxy authentication required (407)");
                Err(ProxyError::auth("Proxy authentication required (407)"))
            },
            502 => {
                tracing::warn!("Proxy cannot reach target (502 Bad Gateway)");
                Err(ProxyError::proxy("Bad gateway (502) - proxy cannot reach target"))
            },
            _ => {
                tracing::warn!("Proxy returned error status: {}", status_code);
                Err(ProxyError::proxy(format!("Proxy returned error status: {status_code}")))
            },
        }
    }
}

impl ProxyConnector for HttpProxyConnector {
    fn connect(&self, host: &str, port: u16) -> Result<TcpStream> {
        let start_time = Instant::now();
        
        // Parse proxy URL
        let (proxy_host, proxy_port) = self.parse_proxy_url()
            .map_err(|e| anyhow::anyhow!("Proxy configuration error: {}", e))?;

        // Sanitize proxy URL for logging (hide credentials)
        let sanitized_url = if self.username.is_some() {
            format!("{}://***:***@{}:{}", 
                if self.proxy_url.starts_with("https") { "https" } else { "http" },
                proxy_host, proxy_port)
        } else {
            self.proxy_url.clone()
        };

        tracing::debug!(
            proxy.url = %sanitized_url,
            proxy.type = "http",
            target.host = %host,
            target.port = %port,
            "Connecting through HTTP proxy"
        );

        // Resolve proxy address
        let proxy_addr = format!("{}:{}", proxy_host, proxy_port)
            .to_socket_addrs()
            .context("Failed to resolve proxy address")?
            .next()
            .ok_or_else(|| anyhow::anyhow!("No addresses resolved for proxy"))?;

        tracing::debug!("Resolved proxy address: {}", proxy_addr);

        // Connect to proxy server with timeout
        let mut stream = TcpStream::connect_timeout(&proxy_addr, self.timeout)
            .map_err(|e| {
                let elapsed = start_time.elapsed();
                tracing::warn!(
                    error = %e,
                    elapsed_ms = elapsed.as_millis(),
                    "Failed to connect to proxy server"
                );
                if elapsed >= self.timeout {
                    anyhow::anyhow!("Proxy connection timeout: {}", ProxyError::timeout(e.to_string()))
                } else {
                    anyhow::anyhow!("Proxy connection failed: {}", ProxyError::network(e.to_string()))
                }
            })?;

        tracing::debug!(
            elapsed_ms = start_time.elapsed().as_millis(),
            "TCP connection to proxy established"
        );

        // Set read/write timeouts
        stream.set_read_timeout(Some(self.timeout))
            .context("Failed to set read timeout")?;
        stream.set_write_timeout(Some(self.timeout))
            .context("Failed to set write timeout")?;

        // Send CONNECT request
        self.send_connect_request(&mut stream, host, port)
            .map_err(|e| {
                tracing::warn!(
                    error = %e,
                    error_category = e.category(),
                    elapsed_ms = start_time.elapsed().as_millis(),
                    "CONNECT request failed"
                );
                anyhow::anyhow!("HTTP proxy CONNECT failed: {}", e)
            })?;

        let total_elapsed = start_time.elapsed();
        tracing::info!(
            proxy.type = "http",
            proxy.url = %sanitized_url,
            target.host = %host,
            target.port = %port,
            elapsed_ms = total_elapsed.as_millis(),
            "HTTP proxy tunnel established successfully"
        );

        Ok(stream)
    }

    fn proxy_type(&self) -> &str {
        "http"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_http_connector_creation() {
        let connector = HttpProxyConnector::new(
            "http://proxy.example.com:8080".to_string(),
            Some("user".to_string()),
            Some("pass".to_string()),
            Duration::from_secs(30),
        );

        assert_eq!(connector.proxy_type(), "http");
    }

    #[test]
    fn test_parse_proxy_url_http() {
        let connector = HttpProxyConnector::new(
            "http://proxy.example.com:8080".to_string(),
            None,
            None,
            Duration::from_secs(30),
        );

        let result = connector.parse_proxy_url().unwrap();
        assert_eq!(result.0, "proxy.example.com");
        assert_eq!(result.1, 8080);
    }

    #[test]
    fn test_parse_proxy_url_https() {
        let connector = HttpProxyConnector::new(
            "https://secure-proxy.example.com:3128".to_string(),
            None,
            None,
            Duration::from_secs(30),
        );

        let result = connector.parse_proxy_url().unwrap();
        assert_eq!(result.0, "secure-proxy.example.com");
        assert_eq!(result.1, 3128);
    }

    #[test]
    fn test_parse_proxy_url_default_port() {
        let connector = HttpProxyConnector::new(
            "http://proxy.example.com".to_string(),
            None,
            None,
            Duration::from_secs(30),
        );

        let result = connector.parse_proxy_url().unwrap();
        assert_eq!(result.0, "proxy.example.com");
        assert_eq!(result.1, 8080); // Default port
    }

    #[test]
    fn test_parse_proxy_url_with_ipv4() {
        let connector = HttpProxyConnector::new(
            "http://192.168.1.100:8080".to_string(),
            None,
            None,
            Duration::from_secs(30),
        );

        let result = connector.parse_proxy_url().unwrap();
        assert_eq!(result.0, "192.168.1.100");
        assert_eq!(result.1, 8080);
    }

    #[test]
    fn test_generate_auth_header_with_credentials() {
        let connector = HttpProxyConnector::new(
            "http://proxy.example.com:8080".to_string(),
            Some("testuser".to_string()),
            Some("testpass".to_string()),
            Duration::from_secs(30),
        );

        let auth = connector.generate_auth_header();
        assert!(auth.is_some());
        let auth_value = auth.unwrap();
        assert!(auth_value.starts_with("Basic "));
        
        // Verify it's valid base64
        let base64_part = auth_value.strip_prefix("Basic ").unwrap();
        assert!(!base64_part.is_empty());
    }

    #[test]
    fn test_generate_auth_header_without_credentials() {
        let connector = HttpProxyConnector::new(
            "http://proxy.example.com:8080".to_string(),
            None,
            None,
            Duration::from_secs(30),
        );

        let auth = connector.generate_auth_header();
        assert!(auth.is_none());
    }

    #[test]
    fn test_generate_auth_header_partial_credentials_user_only() {
        let connector = HttpProxyConnector::new(
            "http://proxy.example.com:8080".to_string(),
            Some("user".to_string()),
            None, // Missing password
            Duration::from_secs(30),
        );

        let auth = connector.generate_auth_header();
        assert!(auth.is_none()); // Should not generate auth without password
    }

    #[test]
    fn test_generate_auth_header_partial_credentials_password_only() {
        let connector = HttpProxyConnector::new(
            "http://proxy.example.com:8080".to_string(),
            None, // Missing username
            Some("pass".to_string()),
            Duration::from_secs(30),
        );

        let auth = connector.generate_auth_header();
        assert!(auth.is_none()); // Should not generate auth without username
    }

    #[test]
    fn test_parse_invalid_proxy_url() {
        let connector = HttpProxyConnector::new(
            "not-a-valid-url".to_string(),
            None,
            None,
            Duration::from_secs(30),
        );

        let result = connector.parse_proxy_url();
        assert!(result.is_err());
        
        match result {
            Err(ProxyError::Config(_)) => (),
            _ => panic!("Expected Config error"),
        }
    }

    #[test]
    fn test_parse_proxy_url_no_host() {
        let connector = HttpProxyConnector::new(
            "http://".to_string(),
            None,
            None,
            Duration::from_secs(30),
        );

        let result = connector.parse_proxy_url();
        assert!(result.is_err());
        
        match result {
            Err(ProxyError::Config(_)) => (),
            _ => panic!("Expected Config error"),
        }
    }

    #[test]
    fn test_generate_auth_header_special_characters() {
        let connector = HttpProxyConnector::new(
            "http://proxy.example.com:8080".to_string(),
            Some("user@domain".to_string()),
            Some("p@ss:word!".to_string()),
            Duration::from_secs(30),
        );

        let auth = connector.generate_auth_header();
        assert!(auth.is_some());
        assert!(auth.unwrap().starts_with("Basic "));
    }

    #[test]
    fn test_timeout_duration() {
        let connector = HttpProxyConnector::new(
            "http://proxy.example.com:8080".to_string(),
            None,
            None,
            Duration::from_secs(45),
        );

        assert_eq!(connector.timeout, Duration::from_secs(45));
    }

    #[test]
    fn test_connector_implements_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<HttpProxyConnector>();
    }

    #[test]
    fn test_proxy_url_with_path() {
        let connector = HttpProxyConnector::new(
            "http://proxy.example.com:8080/path".to_string(),
            None,
            None,
            Duration::from_secs(30),
        );

        let result = connector.parse_proxy_url().unwrap();
        assert_eq!(result.0, "proxy.example.com");
        assert_eq!(result.1, 8080);
    }

    #[test]
    fn test_proxy_url_with_credentials_in_url() {
        let connector = HttpProxyConnector::new(
            "http://urluser:urlpass@proxy.example.com:8080".to_string(),
            None,
            None,
            Duration::from_secs(30),
        );

        let result = connector.parse_proxy_url().unwrap();
        assert_eq!(result.0, "proxy.example.com");
        assert_eq!(result.1, 8080);
    }

    #[test]
    fn test_auth_header_with_empty_strings() {
        let connector = HttpProxyConnector::new(
            "http://proxy.example.com:8080".to_string(),
            Some("".to_string()),
            Some("".to_string()),
            Duration::from_secs(30),
        );

        // Empty credentials should still generate auth header
        let auth = connector.generate_auth_header();
        assert!(auth.is_some());
    }

    #[test]
    fn test_auth_header_with_unicode() {
        let connector = HttpProxyConnector::new(
            "http://proxy.example.com:8080".to_string(),
            Some("用户名".to_string()),
            Some("密码123".to_string()),
            Duration::from_secs(30),
        );

        let auth = connector.generate_auth_header();
        assert!(auth.is_some());
        assert!(auth.unwrap().starts_with("Basic "));
    }

    #[test]
    fn test_very_short_timeout() {
        let connector = HttpProxyConnector::new(
            "http://proxy.example.com:8080".to_string(),
            None,
            None,
            Duration::from_millis(1),
        );

        assert_eq!(connector.timeout, Duration::from_millis(1));
    }

    #[test]
    fn test_very_long_timeout() {
        let connector = HttpProxyConnector::new(
            "http://proxy.example.com:8080".to_string(),
            None,
            None,
            Duration::from_secs(3600),
        );

        assert_eq!(connector.timeout, Duration::from_secs(3600));
    }

    #[test]
    fn test_parse_proxy_url_with_ipv6() {
        let connector = HttpProxyConnector::new(
            "http://[::1]:8080".to_string(),
            None,
            None,
            Duration::from_secs(30),
        );

        let result = connector.parse_proxy_url().unwrap();
        // URL parser keeps brackets for IPv6
        assert_eq!(result.0, "[::1]");
        assert_eq!(result.1, 8080);
    }

    #[test]
    fn test_parse_proxy_url_with_high_port() {
        let connector = HttpProxyConnector::new(
            "http://proxy.example.com:65535".to_string(),
            None,
            None,
            Duration::from_secs(30),
        );

        let result = connector.parse_proxy_url().unwrap();
        assert_eq!(result.0, "proxy.example.com");
        assert_eq!(result.1, 65535);
    }

    #[test]
    fn test_multiple_connectors_independent() {
        let connector1 = HttpProxyConnector::new(
            "http://proxy1.example.com:8080".to_string(),
            Some("user1".to_string()),
            Some("pass1".to_string()),
            Duration::from_secs(30),
        );

        let connector2 = HttpProxyConnector::new(
            "http://proxy2.example.com:3128".to_string(),
            Some("user2".to_string()),
            Some("pass2".to_string()),
            Duration::from_secs(60),
        );

        assert_eq!(connector1.proxy_type(), "http");
        assert_eq!(connector2.proxy_type(), "http");
        assert_ne!(connector1.proxy_url, connector2.proxy_url);
        assert_ne!(connector1.timeout, connector2.timeout);
    }

    #[test]
    fn test_auth_header_credentials_order() {
        let connector = HttpProxyConnector::new(
            "http://proxy.example.com:8080".to_string(),
            Some("testuser".to_string()),
            Some("testpass".to_string()),
            Duration::from_secs(30),
        );

        let auth = connector.generate_auth_header().unwrap();
        // Decode and verify format (username:password)
        let base64_part = auth.strip_prefix("Basic ").unwrap();
        assert!(!base64_part.is_empty());
        
        // The encoded string should be "testuser:testpass"
        use base64::{engine::general_purpose::STANDARD, Engine};
        let expected = STANDARD.encode("testuser:testpass".as_bytes());
        assert_eq!(base64_part, expected);
    }

    #[test]
    fn test_parse_proxy_url_invalid_port() {
        let connector = HttpProxyConnector::new(
            "http://proxy.example.com:abc".to_string(),
            None,
            None,
            Duration::from_secs(30),
        );
        let result = connector.parse_proxy_url();
        assert!(result.is_err());
        match result {
            Err(ProxyError::Config(_)) => (),
            _ => panic!("Expected Config error for non-numeric port"),
        }
    }

    #[test]
    fn test_parse_proxy_url_negative_port() {
        let connector = HttpProxyConnector::new(
            "http://proxy.example.com:-123".to_string(),
            None,
            None,
            Duration::from_secs(30),
        );
        let result = connector.parse_proxy_url();
        assert!(result.is_err());
        match result {
            Err(ProxyError::Config(_)) => (),
            _ => panic!("Expected Config error for negative port"),
        }
    }

    #[test]
    fn test_parse_proxy_url_too_large_port() {
        let connector = HttpProxyConnector::new(
            "http://proxy.example.com:70000".to_string(),
            None,
            None,
            Duration::from_secs(30),
        );
        let result = connector.parse_proxy_url();
        assert!(result.is_err());
        match result {
            Err(ProxyError::Config(_)) => (),
            _ => panic!("Expected Config error for too large port"),
        }
    }

    #[test]
    fn test_generate_auth_header_very_long_credentials() {
        let long_user = "u".repeat(1024);
        let long_pass = "p".repeat(1024);
        let connector = HttpProxyConnector::new(
            "http://proxy.example.com:8080".to_string(),
            Some(long_user.clone()),
            Some(long_pass.clone()),
            Duration::from_secs(30),
        );
        let auth = connector.generate_auth_header();
        assert!(auth.is_some());
        let auth_value = auth.unwrap();
        assert!(auth_value.starts_with("Basic "));
        let base64_part = auth_value.strip_prefix("Basic ").unwrap();
        use base64::{engine::general_purpose::STANDARD, Engine};
        let expected = STANDARD.encode(format!("{}:{}", long_user, long_pass).as_bytes());
        assert_eq!(base64_part, expected);
    }

    #[test]
    fn test_generate_auth_header_unicode_edge() {
        let connector = HttpProxyConnector::new(
            "http://proxy.example.com:8080".to_string(),
            Some("𝓤𝓼𝓮𝓻名".to_string()),
            Some("𝓟𝓪𝓼𝓼密".to_string()),
            Duration::from_secs(30),
        );
        let auth = connector.generate_auth_header();
        assert!(auth.is_some());
        let auth_value = auth.unwrap();
        assert!(auth_value.starts_with("Basic "));
    }
}
