use std::io::{Read, Write};
use std::net::TcpStream;

use git2::Error;
use rand::seq::SliceRandom;
use rustls::StreamOwned;
use rustls::{ClientConfig, ClientConnection, ServerName};

use crate::core::config::loader::load_or_init;
use crate::core::config::model::AppConfig;
use crate::core::tls::util::{proxy_present, set_last_good_sni};
use crate::core::tls::verifier::{create_client_config, create_client_config_with_expected_name};

use super::auth::get_push_auth_header;
use super::util::{
    find_crlf, find_double_crlf, log_body_preview, parse_http_header_first_line_and_host,
};
use super::{HttpOp, TransferKind};
use crate::core::git::transport::metrics::tl_mark_first_byte;

pub(super) struct SniffingStream {
    pub(super) inner: StreamOwned<ClientConnection, TcpStream>,
    pub(super) host: String,
    pub(super) port: u16,
    pub(super) used_fake_sni: bool,
    pub(super) current_sni: String,
    pub(super) path: String,
    pub(super) op: HttpOp,
    pub(super) cfg: AppConfig,
    // 嗅探缓冲（仅用于日志，最多 2KB）
    pub(super) wrote_buf: Vec<u8>,
    pub(super) wrote_logged: bool,
    pub(super) read_buf: Vec<u8>,
    pub(super) read_logged: bool,
    pub(super) read_body_logged: bool,
    // POST 缓冲
    pub(super) post_buf: Vec<u8>,
    pub(super) posted: bool,
    // 请求发送标记（GET）
    pub(super) requested: bool,
    // 响应解析状态
    pub(super) headers_parsed: bool,
    pub(super) header_buf: Vec<u8>,
    // 传输编码
    pub(super) transfer: Option<TransferKind>,
    // 输入原始缓冲（已去掉头部后剩余的 body 原始字节）
    pub(super) inbuf: Vec<u8>,
    // 解码后可供上层读取的字节
    pub(super) decoded: Vec<u8>,
    // 分块状态
    pub(super) chunk_remaining: usize,
    pub(super) reading_chunk_size: bool,
    pub(super) chunk_line: Vec<u8>,
    pub(super) trailer_mode: bool,
    // 内容长度剩余
    pub(super) content_remaining: usize,
    // EOF
    pub(super) eof: bool,
    // 403 轮换保护：仅允许轮换一次，避免无限循环
    pub(super) rotated_once: bool,
    // 若解析到致命 HTTP 状态（如 401 on receive-pack），优先通过 read() 返回该错误
    pub(super) fatal_error: Option<String>,
}

impl SniffingStream {
    pub(super) fn new(
        inner: StreamOwned<ClientConnection, TcpStream>,
        host: String,
        port: u16,
        used_fake_sni: bool,
        current_sni: String,
        path: String,
        op: HttpOp,
        cfg: AppConfig,
    ) -> Self {
        Self {
            inner,
            host,
            port,
            used_fake_sni,
            current_sni,
            path,
            op,
            cfg,
            wrote_buf: Vec::with_capacity(2048),
            wrote_logged: false,
            read_buf: Vec::with_capacity(2048),
            read_logged: false,
            read_body_logged: false,
            post_buf: Vec::new(),
            posted: false,
            requested: false,
            headers_parsed: false,
            header_buf: Vec::new(),
            transfer: None,
            inbuf: Vec::new(),
            decoded: Vec::new(),
            chunk_remaining: 0,
            reading_chunk_size: true,
            chunk_line: Vec::new(),
            trailer_mode: false,
            content_remaining: 0,
            eof: false,
            rotated_once: false,
            fatal_error: None,
        }
    }

    fn try_log_request(&mut self) {
        if self.wrote_logged {
            return;
        }
        let cap = 2048usize;
        let upto = self.wrote_buf.len().min(cap);
        let slice = &self.wrote_buf[..upto];
        if let Some(pos) = find_double_crlf(slice) {
            let header = &slice[..pos];
            let (line1, host, has_auth) = parse_http_header_first_line_and_host(header);
            tracing::debug!(target="git.transport.http", host=%self.host, request_line=%line1, host_header=%host, auth_header_present=%has_auth, "http request");
            if let Ok(text) = std::str::from_utf8(header) {
                let trimmed = text.replace("\r\n", " | ");
                tracing::debug!(target="git.transport.http", host=%self.host, headers=%trimmed, "http request headers");
            }
            self.wrote_logged = true;
        } else if self.wrote_buf.len() >= cap {
            tracing::debug!(target="git.transport.http", host=%self.host, "http request headers not complete within sniff cap");
            self.wrote_logged = true;
        }
    }

    fn try_log_response(&mut self) {
        if self.read_logged {
            return;
        }
        let cap = 2048usize;
        let upto = self.read_buf.len().min(cap);
        let slice = &self.read_buf[..upto];
        if let Some(pos) = find_double_crlf(slice) {
            let header = &slice[..pos];
            let (status, _host, _auth) = parse_http_header_first_line_and_host(header);
            tracing::debug!(target="git.transport.http", host=%self.host, status_line=%status, "http response");
            if let Ok(text) = std::str::from_utf8(header) {
                let trimmed = text.replace("\r\n", " | ");
                tracing::debug!(target="git.transport.http", host=%self.host, headers=%trimmed, "http response headers");
            }
            self.read_logged = true;
        } else if self.read_buf.len() >= cap {
            tracing::debug!(target="git.transport.http", host=%self.host, "http response headers not complete within sniff cap");
            self.read_logged = true;
        }
    }

    fn ensure_request_sent(&mut self) -> std::io::Result<()> {
        match self.op {
            HttpOp::InfoRefsUpload | HttpOp::InfoRefsReceive => {
                if !self.requested {
                    let service = match self.op {
                        HttpOp::InfoRefsUpload => "git-upload-pack",
                        HttpOp::InfoRefsReceive => "git-receive-pack",
                        _ => unreachable!(),
                    };
                    let path = format!("{}/info/refs?service={}", self.path, service);
                    let host_hdr = if self.port == 443 {
                        self.host.clone()
                    } else {
                        format!("{}:{}", self.host, self.port)
                    };
                    // 仅在 receive-pack 的 info/refs 阶段尝试注入 Authorization
                    let auth_line = if matches!(self.op, HttpOp::InfoRefsReceive) {
                        get_push_auth_header().map(|v| format!("Authorization: {v}\r\n"))
                    } else {
                        None
                    };
                    let req = if let Some(al) = auth_line.as_deref() {
                        format!(
                            concat!(
                                "GET {} HTTP/1.1\r\n",
                                "Host: {}\r\n",
                                "User-Agent: git/2.46.0\r\n",
                                "Accept: */*\r\n",
                                "Accept-Encoding: identity\r\n",
                                "Pragma: no-cache\r\n",
                                "Cache-Control: no-cache\r\n",
                                "{}",
                                "Connection: close\r\n",
                                "\r\n"
                            ),
                            path, host_hdr, al
                        )
                    } else {
                        format!(
                            concat!(
                                "GET {} HTTP/1.1\r\n",
                                "Host: {}\r\n",
                                "User-Agent: git/2.46.0\r\n",
                                "Accept: */*\r\n",
                                "Accept-Encoding: identity\r\n",
                                "Pragma: no-cache\r\n",
                                "Cache-Control: no-cache\r\n",
                                "Connection: close\r\n",
                                "\r\n"
                            ),
                            path, host_hdr
                        )
                    };
                    tracing::debug!(target="git.transport.http", host=%self.host, request_line=%format!("GET {} HTTP/1.1", path), "send GET info/refs");
                    if auth_line.is_some() {
                        tracing::debug!(target="git.transport.http", host=%self.host, auth_injected=true, "authorization header injected for info/refs (receive-pack)");
                    }
                    self.wrote_buf.extend_from_slice(req.as_bytes());
                    self.inner.write_all(req.as_bytes())?;
                    self.inner.flush()?;
                    self.try_log_request();
                    self.requested = true;
                }
            }
            HttpOp::UploadPack | HttpOp::ReceivePack => {
                if !self.posted {
                    self.send_post()?;
                }
            }
        }
        Ok(())
    }

    fn send_post(&mut self) -> std::io::Result<()> {
        let (path, accept, content_type) = match self.op {
            HttpOp::UploadPack => (
                format!("{}/git-upload-pack", self.path),
                "application/x-git-upload-pack-result",
                "application/x-git-upload-pack-request",
            ),
            HttpOp::ReceivePack => (
                format!("{}/git-receive-pack", self.path),
                "application/x-git-receive-pack-result",
                "application/x-git-receive-pack-request",
            ),
            _ => unreachable!(),
        };
        let host_hdr = if self.port == 443 {
            self.host.clone()
        } else {
            format!("{}:{}", self.host, self.port)
        };
        let len = self.post_buf.len();
        let auth_line = if matches!(self.op, HttpOp::ReceivePack) {
            get_push_auth_header().map(|v| format!("Authorization: {v}\r\n"))
        } else {
            None
        };
        let headers = if let Some(al) = auth_line.as_deref() {
            format!(
                concat!(
                    "POST {} HTTP/1.1\r\n",
                    "Host: {}\r\n",
                    "User-Agent: git/2.46.0\r\n",
                    "Accept: {}\r\n",
                    "Content-Type: {}\r\n",
                    "Content-Length: {}\r\n",
                    "Accept-Encoding: identity\r\n",
                    "Pragma: no-cache\r\n",
                    "Cache-Control: no-cache\r\n",
                    "{}",
                    "Connection: close\r\n",
                    "\r\n"
                ),
                path, host_hdr, accept, content_type, len, al
            )
        } else {
            format!(
                concat!(
                    "POST {} HTTP/1.1\r\n",
                    "Host: {}\r\n",
                    "User-Agent: git/2.46.0\r\n",
                    "Accept: {}\r\n",
                    "Content-Type: {}\r\n",
                    "Content-Length: {}\r\n",
                    "Accept-Encoding: identity\r\n",
                    "Pragma: no-cache\r\n",
                    "Cache-Control: no-cache\r\n",
                    "Connection: close\r\n",
                    "\r\n"
                ),
                path, host_hdr, accept, content_type, len
            )
        };
        tracing::debug!(target="git.transport.http", host=%self.host, request_line=%format!("POST {} HTTP/1.1", path), content_length=%len, "send POST git-upload-pack");
        if auth_line.is_some() {
            tracing::debug!(target="git.transport.http", host=%self.host, auth_injected=true, "authorization header injected for receive-pack POST");
        }
        self.wrote_buf.extend_from_slice(headers.as_bytes());
        self.inner.write_all(headers.as_bytes())?;
        if len > 0 {
            self.inner.write_all(&self.post_buf)?;
        }
        self.inner.flush()?;
        self.posted = true;
        self.try_log_request();
        Ok(())
    }

    fn parse_headers_and_setup(&mut self) -> std::io::Result<()> {
        if self.headers_parsed {
            return Ok(());
        }
        loop {
            if let Some(pos) = find_double_crlf(&self.header_buf) {
                let header = self.header_buf[..pos].to_vec();
                let remaining = self.header_buf[pos..].to_vec();
                self.inbuf.extend_from_slice(&remaining);
                let mut status_line = String::new();
                let mut content_len: Option<usize> = None;
                let mut is_chunked = false;
                let mut content_type = String::new();
                let mut status_code: Option<u16> = None;
                let mut www_authenticate: Option<String> = None;
                if let Ok(text) = std::str::from_utf8(&header) {
                    for (i, line) in text.split("\r\n").enumerate() {
                        if i == 0 {
                            status_line = line.to_string();
                            let parts: Vec<&str> = line.split_whitespace().collect();
                            if parts.len() >= 2 {
                                if let Ok(code) = parts[1].parse::<u16>() {
                                    status_code = Some(code);
                                }
                            }
                            continue;
                        }
                        let mut parts = line.splitn(2, ':');
                        if let (Some(k), Some(v)) = (parts.next(), parts.next()) {
                            let k_l = k.trim().to_ascii_lowercase();
                            let v_t = v.trim();
                            if k_l == "content-length" {
                                if let Ok(n) = v_t.parse::<usize>() {
                                    content_len = Some(n);
                                }
                            }
                            if k_l == "transfer-encoding" && v_t.eq_ignore_ascii_case("chunked") {
                                is_chunked = true;
                            }
                            if k_l == "content-type" {
                                content_type = v_t.to_string();
                            }
                            if k_l == "www-authenticate" {
                                www_authenticate = Some(v_t.to_string());
                            }
                        }
                    }
                }
                tracing::debug!(target="git.transport.http", host=%self.host, status_line=%status_line, content_type=%content_type, chunked=%is_chunked, content_length=?content_len, "http response parsed");

                // 401 观测（通常表示需要认证）
                if let Some(401) = status_code {
                    let has_auth = get_push_auth_header().is_some();
                    tracing::debug!(target="git.transport.http", host=%self.host, auth_present_in_request=%has_auth, "401 Unauthorized encountered");
                    if matches!(self.op, HttpOp::InfoRefsReceive | HttpOp::ReceivePack) {
                        let realm = www_authenticate.as_deref().unwrap_or("");
                        let msg = if !realm.is_empty() {
                            format!("HTTP 401 Unauthorized ({realm}). Authentication required for git-receive-pack; please provide credentials.")
                        } else {
                            "HTTP 401 Unauthorized. Authentication required for git-receive-pack; please provide credentials.".to_string()
                        };
                        self.fatal_error = Some(msg);
                    }
                }
                // 403 自动 SNI 轮换（仅限 InfoRefs 阶段；开启 sni_rotate_on_403；每个流仅尝试一次）。
                if matches!(self.op, HttpOp::InfoRefsUpload | HttpOp::InfoRefsReceive) {
                    if let Some(403) = status_code {
                        if self.cfg.http.sni_rotate_on_403 && !self.rotated_once {
                            tracing::debug!(target="git.transport.http", host=%self.host, sni=%self.current_sni, "received 403, try rotate SNI and retry once");
                            if let Ok((new_stream, new_used_fake, new_sni)) =
                                Self::reconnect_with_rotated_sni(
                                    &self.cfg,
                                    &self.host,
                                    self.port,
                                    &self.current_sni,
                                )
                            {
                                self.inner = new_stream;
                                self.used_fake_sni = new_used_fake;
                                self.current_sni = new_sni;
                                self.rotated_once = true;
                                self.header_buf.clear();
                                self.inbuf.clear();
                                self.decoded.clear();
                                self.read_buf.clear();
                                self.read_logged = false;
                                self.wrote_buf.clear();
                                self.wrote_logged = false;
                                self.requested = false;
                                self.headers_parsed = false;
                                self.transfer = None;
                                self.chunk_remaining = 0;
                                self.reading_chunk_size = true;
                                self.trailer_mode = false;
                                self.content_remaining = 0;
                                self.eof = false;
                                self.ensure_request_sent()?;
                                continue;
                            }
                        }
                    }
                    if let Some(code) = status_code {
                        if (200..300).contains(&code) && self.used_fake_sni {
                            set_last_good_sni(&self.host, &self.current_sni);
                        }
                    }
                }
                if is_chunked {
                    self.transfer = Some(TransferKind::Chunked);
                    self.reading_chunk_size = true;
                    self.chunk_remaining = 0;
                } else if let Some(n) = content_len {
                    self.transfer = Some(TransferKind::Length);
                    self.content_remaining = n;
                } else {
                    self.transfer = Some(TransferKind::Eof);
                }
                self.headers_parsed = true;
                break;
            }
            let mut tmp = [0u8; 4096];
            let n = self.inner.read(&mut tmp)?;
            if n == 0 {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    "unexpected eof in headers",
                ));
            }
            self.header_buf.extend_from_slice(&tmp[..n]);
        }
        Ok(())
    }

    fn fill_decoded(&mut self) -> std::io::Result<()> {
        if self.eof {
            return Ok(());
        }
        if !self.headers_parsed {
            self.parse_headers_and_setup()?;
        }
        match self.transfer {
            Some(TransferKind::Chunked) => self.decode_chunked(),
            Some(TransferKind::Length) => self.decode_content_length(),
            Some(TransferKind::Eof) => self.decode_to_eof(),
            None => Ok(()),
        }
    }

    fn read_more(&mut self) -> std::io::Result<usize> {
        let mut tmp = [0u8; 8192];
        let n = self.inner.read(&mut tmp)?;
        if n > 0 {
            self.inbuf.extend_from_slice(&tmp[..n]);
        }
        Ok(n)
    }

    fn decode_chunked(&mut self) -> std::io::Result<()> {
        loop {
            if self.trailer_mode {
                if let Some(pos) = find_double_crlf(&self.inbuf) {
                    self.inbuf.drain(..pos);
                    self.eof = true;
                    return Ok(());
                }
                let n = self.read_more()?;
                if n == 0 {
                    self.eof = true;
                    return Ok(());
                }
                continue;
            }
            if self.reading_chunk_size {
                if let Some(idx) = find_crlf(&self.inbuf) {
                    let line = self.inbuf.drain(..idx + 2).collect::<Vec<u8>>();
                    let line_no_crlf = &line[..line.len() - 2];
                    let hex_part = match line_no_crlf.split(|&b| b == b';').next() {
                        Some(h) => h,
                        None => line_no_crlf,
                    };
                    let hex_str = std::str::from_utf8(hex_part).unwrap_or("");
                    let size = usize::from_str_radix(hex_str.trim(), 16).unwrap_or(0);
                    self.chunk_remaining = size;
                    self.reading_chunk_size = false;
                    if size == 0 {
                        self.trailer_mode = true;
                    }
                    continue;
                }
                let n = self.read_more()?;
                if n == 0 {
                    return Ok(());
                }
                continue;
            }
            if self.chunk_remaining > 0 {
                if self.inbuf.is_empty() {
                    let n = self.read_more()?;
                    if n == 0 {
                        return Ok(());
                    }
                }
                let take = self.chunk_remaining.min(self.inbuf.len());
                if take > 0 {
                    let data = self.inbuf.drain(..take).collect::<Vec<u8>>();
                    self.decoded.extend_from_slice(&data);
                    self.chunk_remaining -= take;
                }
                if self.chunk_remaining > 0 {
                    return Ok(());
                }
                if self.inbuf.len() < 2 {
                    let _ = self.read_more()?;
                }
                if self.inbuf.len() >= 2 && &self.inbuf[..2] == b"\r\n" {
                    self.inbuf.drain(..2);
                }
                self.reading_chunk_size = true;
                continue;
            }
        }
    }

    fn decode_content_length(&mut self) -> std::io::Result<()> {
        if self.content_remaining == 0 {
            self.eof = true;
            return Ok(());
        }
        if self.inbuf.is_empty() {
            let _ = self.read_more()?;
        }
        if self.inbuf.is_empty() {
            return Ok(());
        }
        let take = self.content_remaining.min(self.inbuf.len());
        let data = self.inbuf.drain(..take).collect::<Vec<u8>>();
        self.decoded.extend_from_slice(&data);
        self.content_remaining -= take;
        if self.content_remaining == 0 {
            self.eof = true;
        }
        Ok(())
    }

    fn decode_to_eof(&mut self) -> std::io::Result<()> {
        if self.inbuf.is_empty() {
            let n = self.read_more()?;
            if n == 0 {
                self.eof = true;
                return Ok(());
            }
        }
        if !self.inbuf.is_empty() {
            let data = self.inbuf.drain(..).collect::<Vec<u8>>();
            self.decoded.extend_from_slice(&data);
        }
        Ok(())
    }

    /// 尝试以不同的 SNI 重新建立 TLS 连接，优先随机选择不同于 `current_sni` 的伪 SNI 候选；若失败则返回错误。
    fn reconnect_with_rotated_sni(
        _cfg: &AppConfig,
        host: &str,
        port: u16,
        current_sni: &str,
    ) -> Result<(StreamOwned<ClientConnection, TcpStream>, bool, String), Error> {
        let cfg_now = load_or_init().unwrap_or_else(|_| AppConfig::default());
        let present = proxy_present();
        if present || !cfg_now.http.fake_sni_enabled {
            let server_name =
                ServerName::try_from(host).map_err(|_| Error::from_str("invalid real host"))?;
            let tls_cfg: std::sync::Arc<ClientConfig> =
                std::sync::Arc::new(create_client_config(&cfg_now.tls));
            let addr = format!("{host}:{port}");
            let tcp = TcpStream::connect(addr)
                .map_err(|e| Error::from_str(&format!("tcp connect: {e}")))?;
            tcp.set_nodelay(true).ok();
            let mut conn = ClientConnection::new(tls_cfg, server_name)
                .map_err(|e| Error::from_str(&format!("tls client: {e}")))?;
            conn.complete_io(&mut &tcp)
                .map_err(|e| Error::from_str(&format!("tls handshake: {e}")))?;
            let mut stream = StreamOwned::new(conn, tcp);
            let _ = stream.flush();
            tracing::debug!(target="git.transport.http", host=%host, new_sni=%host, used_fake=false, "reconnect with real SNI due to proxy/disabled");
            return Ok((stream, false, host.to_string()));
        }

        let mut candidates: Vec<String> = Vec::new();
        for h in cfg_now.http.fake_sni_hosts.iter() {
            let h = h.trim();
            if !h.is_empty() && h != current_sni {
                candidates.push(h.to_string());
            }
        }
        if candidates.is_empty() {
            let server_name =
                ServerName::try_from(host).map_err(|_| Error::from_str("invalid real host"))?;
            let tls_cfg: std::sync::Arc<ClientConfig> =
                std::sync::Arc::new(create_client_config(&cfg_now.tls));
            let addr = format!("{host}:{port}");
            let tcp = TcpStream::connect(addr)
                .map_err(|e| Error::from_str(&format!("tcp connect: {e}")))?;
            tcp.set_nodelay(true).ok();
            let mut conn = ClientConnection::new(tls_cfg, server_name)
                .map_err(|e| Error::from_str(&format!("tls client: {e}")))?;
            conn.complete_io(&mut &tcp)
                .map_err(|e| Error::from_str(&format!("tls handshake: {e}")))?;
            let mut stream = StreamOwned::new(conn, tcp);
            let _ = stream.flush();
            tracing::debug!(target="git.transport.http", host=%host, new_sni=%host, used_fake=false, "no alternative fake SNI, fallback real");
            return Ok((stream, false, host.to_string()));
        }
        let mut rng = rand::thread_rng();
        let pick = candidates
            .choose(&mut rng)
            .cloned()
            .unwrap_or_else(|| candidates[0].clone());

        let server_name =
            ServerName::try_from(pick.as_str()).map_err(|_| Error::from_str("invalid sni host"))?;
        let tls_cfg: std::sync::Arc<ClientConfig> =
            std::sync::Arc::new(create_client_config_with_expected_name(&cfg_now.tls, host));
        let addr = format!("{host}:{port}");
        let tcp =
            TcpStream::connect(addr).map_err(|e| Error::from_str(&format!("tcp connect: {e}")))?;
        tcp.set_nodelay(true).ok();
        let mut conn = ClientConnection::new(tls_cfg, server_name)
            .map_err(|e| Error::from_str(&format!("tls client: {e}")))?;
        conn.complete_io(&mut &tcp)
            .map_err(|e| Error::from_str(&format!("tls handshake: {e}")))?;
        let mut stream = StreamOwned::new(conn, tcp);
        let _ = stream.flush();
        tracing::debug!(target="git.transport.http", host=%host, new_sni=%pick, used_fake=true, "reconnect with rotated SNI ok");
        Ok((stream, true, pick))
    }
}

impl Read for SniffingStream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        tracing::debug!(target="git.transport", host=%self.host, read_buf_len=%buf.len(), "stream read attempt");
        self.ensure_request_sent()?;
        if !self.headers_parsed {
            self.parse_headers_and_setup()?;
        }
        if let Some(msg) = self.fatal_error.clone() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                msg,
            ));
        }
        if !self.decoded.is_empty() {
            let n = self.decoded.len().min(buf.len());
            if !self.read_body_logged {
                log_body_preview(
                    &self.decoded[..n.min(32)],
                    &self.host,
                    "first decoded bytes",
                );
                self.read_body_logged = true;
                tl_mark_first_byte();
            }
            buf[..n].copy_from_slice(&self.decoded[..n]);
            self.decoded.drain(..n);
            if !self.read_logged {
                let cap = 2048usize;
                let upto = self.header_buf.len().min(cap);
                self.read_buf.extend_from_slice(&self.header_buf[..upto]);
                self.try_log_response();
            }
            return Ok(n);
        }
        self.fill_decoded()?;
        if !self.decoded.is_empty() {
            let n = self.decoded.len().min(buf.len());
            if !self.read_body_logged {
                log_body_preview(
                    &self.decoded[..n.min(32)],
                    &self.host,
                    "first decoded bytes",
                );
                self.read_body_logged = true;
                tl_mark_first_byte();
            }
            buf[..n].copy_from_slice(&self.decoded[..n]);
            self.decoded.drain(..n);
            if !self.read_logged {
                let cap = 2048usize;
                let upto = self.header_buf.len().min(cap);
                self.read_buf.extend_from_slice(&self.header_buf[..upto]);
                self.try_log_response();
            }
            return Ok(n);
        }
        if self.eof {
            return Ok(0);
        }
        loop {
            self.fill_decoded()?;
            if !self.decoded.is_empty() {
                let n = self.decoded.len().min(buf.len());
                buf[..n].copy_from_slice(&self.decoded[..n]);
                self.decoded.drain(..n);
                if !self.read_logged {
                    let cap = 2048usize;
                    let upto = self.header_buf.len().min(cap);
                    self.read_buf.extend_from_slice(&self.header_buf[..upto]);
                    self.try_log_response();
                    tl_mark_first_byte();
                }
                return Ok(n);
            }
            if self.eof {
                return Ok(0);
            }
            let _ = self.read_more()?;
        }
    }
}

impl Write for SniffingStream {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        tracing::debug!(target="git.transport", host=%self.host, write_len=%buf.len(), "stream write attempt");
        match self.op {
            HttpOp::InfoRefsUpload | HttpOp::InfoRefsReceive => Ok(buf.len()),
            HttpOp::UploadPack | HttpOp::ReceivePack => {
                self.post_buf.extend_from_slice(buf);
                Ok(buf.len())
            }
        }
    }
    fn flush(&mut self) -> std::io::Result<()> {
        if matches!(self.op, HttpOp::UploadPack | HttpOp::ReceivePack) && !self.posted {
            self.send_post()?;
        }
        Ok(())
    }
}

impl std::io::Seek for SniffingStream {
    fn seek(&mut self, _pos: std::io::SeekFrom) -> std::io::Result<u64> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "seek is not supported on a network stream",
        ))
    }
}
