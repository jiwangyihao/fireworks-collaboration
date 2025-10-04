//! SOCKS5 代理连接器实现
//!
//! 本模块实现了 SOCKS5 代理协议（RFC 1928），支持：
//! - No Auth (0x00) 认证方法
//! - Username/Password Auth (0x02) 认证方法
//! - IPv4、IPv6 和域名地址类型
//! - 超时控制和错误分类
//!
//! # SOCKS5 协议流程
//!
//! 1. 版本协商：客户端发送支持的认证方法列表，服务器选择一个
//! 2. 认证：根据服务器选择的方法进行认证（可选）
//! 3. 连接请求：客户端发送目标地址和端口
//! 4. 连接响应：服务器返回连接结果

use super::errors::ProxyError;
use super::ProxyConnector;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::time::Duration;

/// SOCKS5 协议版本号
pub const SOCKS5_VERSION: u8 = 0x05;

/// 认证方法：无需认证
pub const AUTH_NO_AUTH: u8 = 0x00;

/// 认证方法：用户名/密码认证
pub const AUTH_USERNAME_PASSWORD: u8 = 0x02;

/// 认证方法：无可接受的方法
pub const AUTH_NO_ACCEPTABLE: u8 = 0xFF;

/// 用户名/密码认证子协商版本
pub const AUTH_SUBNEG_VERSION: u8 = 0x01;

/// SOCKS5 命令：CONNECT
pub const CMD_CONNECT: u8 = 0x01;

/// 地址类型：IPv4
pub const ATYP_IPV4: u8 = 0x01;

/// 地址类型：域名
pub const ATYP_DOMAIN: u8 = 0x03;

/// 地址类型：IPv6
pub const ATYP_IPV6: u8 = 0x04;

/// SOCKS5 响应：成功
pub const REP_SUCCESS: u8 = 0x00;

/// SOCKS5 代理连接器
///
/// 实现 SOCKS5 代理协议，支持 No Auth 和 Username/Password 认证
pub struct Socks5ProxyConnector {
    /// 代理服务器 URL
    proxy_url: String,
    /// 代理服务器主机名
    pub proxy_host: String,
    /// 代理服务器端口
    pub proxy_port: u16,
    /// 可选的用户名
    pub username: Option<String>,
    /// 可选的密码
    pub password: Option<String>,
    /// 连接超时
    pub timeout: Duration,
}

impl Socks5ProxyConnector {
    /// 创建新的 SOCKS5 代理连接器
    ///
    /// # 参数
    ///
    /// * `proxy_url` - 代理服务器 URL (例如: "<socks5://proxy.example.com:1080>")
    /// * `username` - 可选的认证用户名
    /// * `password` - 可选的认证密码
    /// * `timeout` - 连接超时时间
    ///
    /// # 返回
    ///
    /// 成功时返回连接器实例，失败时返回 `ProxyError`
    pub fn new(
        proxy_url: String,
        username: Option<String>,
        password: Option<String>,
        timeout: Duration,
    ) -> Result<Self, ProxyError> {
        let (host, port) = Self::parse_proxy_url(&proxy_url)?;

        Ok(Self {
            proxy_url,
            proxy_host: host,
            proxy_port: port,
            username,
            password,
            timeout,
        })
    }

    /// 解析代理 URL
    ///
    /// 支持的格式:
    /// - socks5://host:port
    /// - socks://host:port (视为 SOCKS5)
    /// - host:port (默认为 SOCKS5)
    ///
    /// # 参数
    ///
    /// * `url` - 代理服务器 URL
    ///
    /// # 返回
    ///
    /// 成功时返回 (host, port) 元组，失败时返回 `ProxyError`
    pub fn parse_proxy_url(url: &str) -> Result<(String, u16), ProxyError> {
        // 移除 scheme 前缀
        let url = url
            .trim_start_matches("socks5://")
            .trim_start_matches("socks://");

        // 解析 host:port
        let parts: Vec<&str> = url.rsplitn(2, ':').collect();
        if parts.len() != 2 {
            return Err(ProxyError::config(format!(
                "Invalid SOCKS5 proxy URL format: {url}"
            )));
        }

        let port_str = parts[0];
        let host = parts[1];

        if host.is_empty() {
            return Err(ProxyError::config("Proxy host cannot be empty".to_string()));
        }

        let port = port_str
            .parse::<u16>()
            .map_err(|e| ProxyError::config(format!("Invalid proxy port '{port_str}': {e}")))?;

        if port == 0 {
            return Err(ProxyError::config("Proxy port cannot be 0".to_string()));
        }

        Ok((host.to_string(), port))
    }

    /// 获取脱敏的代理 URL（隐藏凭证）
    pub fn sanitized_url(&self) -> String {
        if self.username.is_some() {
            format!("socks5://***:***@{}:{}", self.proxy_host, self.proxy_port)
        } else {
            format!("socks5://{}:{}", self.proxy_host, self.proxy_port)
        }
    }

    /// 版本协商
    ///
    /// 发送支持的认证方法列表，接收服务器选择的方法
    ///
    /// # 参数
    ///
    /// * `stream` - TCP 连接流
    ///
    /// # 返回
    ///
    /// 成功时返回服务器选择的认证方法，失败时返回 `ProxyError`
    fn negotiate_version(&self, stream: &mut TcpStream) -> Result<u8, ProxyError> {
        // 构造认证方法列表
        let mut methods = vec![AUTH_NO_AUTH];
        if self.username.is_some() && self.password.is_some() {
            methods.push(AUTH_USERNAME_PASSWORD);
        }

        // 发送版本协商请求: VER | NMETHODS | METHODS
        let mut request = vec![SOCKS5_VERSION, methods.len() as u8];
        request.extend_from_slice(&methods);

        stream
            .write_all(&request)
            .map_err(|e| ProxyError::network(format!("Failed to send version negotiation: {e}")))?;

        tracing::debug!(
            methods = ?methods,
            "Sent SOCKS5 version negotiation"
        );

        // 读取服务器响应: VER | METHOD
        let mut response = [0u8; 2];
        stream
            .read_exact(&mut response)
            .map_err(|e| ProxyError::network(format!("Failed to read version response: {e}")))?;

        let version = response[0];
        let chosen_method = response[1];

        // 验证版本号
        if version != SOCKS5_VERSION {
            return Err(ProxyError::proxy(format!(
                "Invalid SOCKS version from proxy: expected 0x05, got 0x{version:02x}"
            )));
        }

        // 检查是否无可接受的方法
        if chosen_method == AUTH_NO_ACCEPTABLE {
            return Err(ProxyError::proxy(
                "No acceptable authentication methods".to_string(),
            ));
        }

        tracing::debug!(
            method = chosen_method,
            "SOCKS5 server chose authentication method"
        );

        Ok(chosen_method)
    }

    /// 无认证
    ///
    /// 当服务器选择 No Auth 方法时，直接跳过认证步骤
    fn authenticate_none(&self) -> Result<(), ProxyError> {
        tracing::debug!("Using No Auth (0x00)");
        Ok(())
    }

    /// 用户名/密码认证
    ///
    /// 发送用户名和密码进行认证
    ///
    /// # 参数
    ///
    /// * `stream` - TCP 连接流
    ///
    /// # 返回
    ///
    /// 成功时返回 Ok(())，失败时返回 `ProxyError`
    fn authenticate_password(&self, stream: &mut TcpStream) -> Result<(), ProxyError> {
        let username = self.username.as_ref().ok_or_else(|| {
            ProxyError::config("Username required for password authentication".to_string())
        })?;
        let password = self.password.as_ref().ok_or_else(|| {
            ProxyError::config("Password required for password authentication".to_string())
        })?;

        // 构造认证请求: VER | ULEN | UNAME | PLEN | PASSWD
        let username_bytes = username.as_bytes();
        let password_bytes = password.as_bytes();

        if username_bytes.len() > 255 {
            return Err(ProxyError::config(
                "Username too long (max 255 bytes)".to_string(),
            ));
        }
        if password_bytes.len() > 255 {
            return Err(ProxyError::config(
                "Password too long (max 255 bytes)".to_string(),
            ));
        }

        let mut request = vec![AUTH_SUBNEG_VERSION, username_bytes.len() as u8];
        request.extend_from_slice(username_bytes);
        request.push(password_bytes.len() as u8);
        request.extend_from_slice(password_bytes);

        stream
            .write_all(&request)
            .map_err(|e| ProxyError::network(format!("Failed to send authentication: {e}")))?;

        tracing::debug!(
            username_len = username_bytes.len(),
            "Sent username/password authentication"
        );

        // 读取认证响应: VER | STATUS
        let mut response = [0u8; 2];
        stream
            .read_exact(&mut response)
            .map_err(|e| ProxyError::network(format!("Failed to read auth response: {e}")))?;

        let version = response[0];
        let status = response[1];

        if version != AUTH_SUBNEG_VERSION {
            return Err(ProxyError::proxy(format!(
                "Invalid auth subnegotiation version: expected 0x01, got 0x{version:02x}"
            )));
        }

        if status != 0x00 {
            return Err(ProxyError::auth(format!(
                "Authentication failed: status 0x{status:02x}"
            )));
        }

        tracing::debug!("Username/password authentication successful");
        Ok(())
    }

    /// 发送连接请求
    ///
    /// 支持 IPv4、IPv6 和域名三种地址类型
    ///
    /// # 参数
    ///
    /// * `stream` - TCP 连接流
    /// * `host` - 目标主机名或 IP 地址
    /// * `port` - 目标端口
    ///
    /// # 返回
    ///
    /// 成功时返回 Ok(())，失败时返回 `ProxyError`
    fn send_connect_request(
        &self,
        stream: &mut TcpStream,
        host: &str,
        port: u16,
    ) -> Result<(), ProxyError> {
        // 构造连接请求: VER | CMD | RSV | ATYP | DST.ADDR | DST.PORT
        let mut request = vec![SOCKS5_VERSION, CMD_CONNECT, 0x00]; // VER, CMD, RSV

        // 尝试解析为 IP 地址
        if let Ok(ip) = host.parse::<std::net::IpAddr>() {
            match ip {
                std::net::IpAddr::V4(ipv4) => {
                    request.push(ATYP_IPV4);
                    request.extend_from_slice(&ipv4.octets());
                    tracing::debug!(ip = %ipv4, "Using IPv4 address");
                }
                std::net::IpAddr::V6(ipv6) => {
                    request.push(ATYP_IPV6);
                    request.extend_from_slice(&ipv6.octets());
                    tracing::debug!(ip = %ipv6, "Using IPv6 address");
                }
            }
        } else {
            // 使用域名
            let host_bytes = host.as_bytes();
            if host_bytes.len() > 255 {
                return Err(ProxyError::config(
                    "Hostname too long (max 255 bytes)".to_string(),
                ));
            }
            request.push(ATYP_DOMAIN);
            request.push(host_bytes.len() as u8);
            request.extend_from_slice(host_bytes);
            tracing::debug!(domain = %host, "Using domain name");
        }

        // 添加端口（大端序）
        request.extend_from_slice(&port.to_be_bytes());

        stream
            .write_all(&request)
            .map_err(|e| ProxyError::network(format!("Failed to send connect request: {e}")))?;

        tracing::debug!(
            target.host = %host,
            target.port = %port,
            "Sent SOCKS5 connect request"
        );

        Ok(())
    }

    /// 解析连接响应
    ///
    /// 读取并解析服务器的连接响应
    ///
    /// # 参数
    ///
    /// * `stream` - TCP 连接流
    ///
    /// # 返回
    ///
    /// 成功时返回 Ok(())，失败时返回 `ProxyError`
    fn parse_connect_response(&self, stream: &mut TcpStream) -> Result<(), ProxyError> {
        // 读取响应头: VER | REP | RSV | ATYP
        let mut header = [0u8; 4];
        stream
            .read_exact(&mut header)
            .map_err(|e| ProxyError::network(format!("Failed to read connect response: {e}")))?;

        let version = header[0];
        let rep = header[1];
        let _rsv = header[2];
        let atyp = header[3];

        // 验证版本号
        if version != SOCKS5_VERSION {
            return Err(ProxyError::proxy(format!(
                "Invalid SOCKS version in response: expected 0x05, got 0x{version:02x}"
            )));
        }

        // 检查响应状态
        if rep != REP_SUCCESS {
            let error_msg = match rep {
                0x01 => "General SOCKS server failure",
                0x02 => "Connection not allowed by ruleset",
                0x03 => "Network unreachable",
                0x04 => "Host unreachable",
                0x05 => "Connection refused",
                0x06 => "TTL expired",
                0x07 => "Command not supported",
                0x08 => "Address type not supported",
                _ => "Unknown SOCKS error",
            };
            return Err(ProxyError::proxy(format!("{error_msg} (code 0x{rep:02x})")));
        }

        // 读取绑定地址（我们不需要使用它，但必须读取以清空缓冲区）
        let addr_len = match atyp {
            ATYP_IPV4 => 4,
            ATYP_IPV6 => 16,
            ATYP_DOMAIN => {
                let mut len_byte = [0u8; 1];
                stream.read_exact(&mut len_byte).map_err(|e| {
                    ProxyError::network(format!("Failed to read domain length: {e}"))
                })?;
                len_byte[0] as usize
            }
            _ => {
                return Err(ProxyError::proxy(format!(
                    "Unknown address type in response: 0x{atyp:02x}"
                )));
            }
        };

        // 读取地址和端口（地址长度 + 2 字节端口）
        let mut addr_and_port = vec![0u8; addr_len + 2];
        stream
            .read_exact(&mut addr_and_port)
            .map_err(|e| ProxyError::network(format!("Failed to read bind address: {e}")))?;

        tracing::debug!(rep = rep, "SOCKS5 connect response: success");
        Ok(())
    }
}

impl ProxyConnector for Socks5ProxyConnector {
    fn connect(&self, host: &str, port: u16) -> Result<TcpStream, ProxyError> {
        let start = std::time::Instant::now();

        tracing::debug!(
            proxy.url = %self.sanitized_url(),
            target.host = %host,
            target.port = %port,
            timeout_secs = self.timeout.as_secs(),
            "Attempting SOCKS5 proxy connection"
        );

        // 1. 连接到代理服务器
        let proxy_addr = format!("{}:{}", self.proxy_host, self.proxy_port);
        let proxy_socket: SocketAddr = proxy_addr
            .to_socket_addrs()
            .map_err(|e| {
                ProxyError::network(format!(
                    "Failed to resolve proxy address '{proxy_addr}': {e}"
                ))
            })?
            .next()
            .ok_or_else(|| {
                ProxyError::network(format!("No addresses resolved for proxy '{proxy_addr}'"))
            })?;

        let mut stream = TcpStream::connect_timeout(&proxy_socket, self.timeout).map_err(|e| {
            if e.kind() == std::io::ErrorKind::TimedOut {
                ProxyError::timeout(format!(
                    "Proxy connection timed out after {:?}",
                    self.timeout
                ))
            } else {
                ProxyError::network(format!("Failed to connect to proxy: {e}"))
            }
        })?;

        // 设置读写超时
        stream
            .set_read_timeout(Some(self.timeout))
            .map_err(|e| ProxyError::network(format!("Failed to set read timeout: {e}")))?;
        stream
            .set_write_timeout(Some(self.timeout))
            .map_err(|e| ProxyError::network(format!("Failed to set write timeout: {e}")))?;

        tracing::debug!(
            elapsed_ms = start.elapsed().as_millis(),
            "Connected to SOCKS5 proxy"
        );

        // 2. 版本协商
        let chosen_method = self.negotiate_version(&mut stream)?;

        // 3. 认证
        match chosen_method {
            AUTH_NO_AUTH => self.authenticate_none()?,
            AUTH_USERNAME_PASSWORD => self.authenticate_password(&mut stream)?,
            _ => {
                return Err(ProxyError::proxy(format!(
                    "Unsupported authentication method: 0x{chosen_method:02x}"
                )));
            }
        }

        // 4. 发送连接请求
        self.send_connect_request(&mut stream, host, port)?;

        // 5. 解析连接响应
        self.parse_connect_response(&mut stream)?;

        let total_elapsed = start.elapsed();
        tracing::info!(
            proxy.type = "socks5",
            proxy.url = %self.sanitized_url(),
            target.host = %host,
            target.port = %port,
            elapsed_ms = total_elapsed.as_millis(),
            "SOCKS5 tunnel established successfully"
        );

        Ok(stream)
    }

    fn proxy_type(&self) -> &str {
        "socks5"
    }
}
