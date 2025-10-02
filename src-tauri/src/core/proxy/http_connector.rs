//! HTTP/HTTPS proxy connector implementation
//!
//! Implements HTTP CONNECT tunnel protocol for proxying TCP connections.
//! Supports Basic authentication and timeout control.

use super::{ProxyConnector, ProxyError};
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
    pub proxy_url: String,
    
    /// Optional username for Basic authentication
    pub username: Option<String>,
    
    /// Optional password for Basic authentication
    pub password: Option<String>,
    
    /// Connection timeout in seconds
    pub timeout: Duration,
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
    pub fn parse_proxy_url(&self) -> Result<(String, u16), ProxyError> {
        let url = Url::parse(&self.proxy_url)
            .map_err(|e| ProxyError::config(format!("Invalid proxy URL: {e}")))?;
        
        let host = url.host_str()
            .ok_or_else(|| ProxyError::config("Proxy URL missing host"))?
            .to_string();
        
        let port = url.port().unwrap_or(8080); // Default HTTP proxy port
        
        Ok((host, port))
    }

    /// Generate Basic authentication header value
    pub fn generate_auth_header(&self) -> Option<String> {
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
    fn connect(&self, host: &str, port: u16) -> Result<TcpStream, ProxyError> {
        let start_time = Instant::now();
        
        // Parse proxy URL
        let (proxy_host, proxy_port) = self.parse_proxy_url()?;

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
            .map_err(|e| ProxyError::network(format!("Failed to resolve proxy address: {e}")))?
            .next()
            .ok_or_else(|| ProxyError::network("No addresses resolved for proxy".to_string()))?;

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
                    ProxyError::timeout(format!("Proxy connection timeout: {e}"))
                } else {
                    ProxyError::network(format!("Proxy connection failed: {e}"))
                }
            })?;

        tracing::debug!(
            elapsed_ms = start_time.elapsed().as_millis(),
            "TCP connection to proxy established"
        );

        // Set read/write timeouts
        stream.set_read_timeout(Some(self.timeout))
            .map_err(|e| ProxyError::network(format!("Failed to set read timeout: {e}")))?;
        stream.set_write_timeout(Some(self.timeout))
            .map_err(|e| ProxyError::network(format!("Failed to set write timeout: {e}")))?;

        // Send CONNECT request
        self.send_connect_request(&mut stream, host, port)
            .map_err(|e| {
                tracing::warn!(
                    error = %e,
                    error_category = e.category(),
                    elapsed_ms = start_time.elapsed().as_millis(),
                    "CONNECT request failed"
                );
                e
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
