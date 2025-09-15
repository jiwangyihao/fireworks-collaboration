use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::{Arc, OnceLock};
use std::cell::RefCell;

use git2::{transport, Error, Remote};
use rustls::StreamOwned;
use rustls::{ClientConfig, ClientConnection, ServerName};
use rand::seq::SliceRandom;
use url::Url;
use crate::core::config::loader::load_or_init;

use crate::core::config::model::AppConfig;
use crate::core::tls::util::{decide_sni_host_with_proxy, match_domain, proxy_present, set_last_good_sni};
use crate::core::tls::verifier::{create_client_config, create_client_config_with_expected_name};

/// 自定义 HTTPS 子传输：仅接管 TCP/TLS 建立与可选伪 SNI；HTTP 语义仍由 libgit2 智能传输处理。
struct CustomHttpsSubtransport {
    cfg: AppConfig,
    tls: Arc<ClientConfig>,
}

/// 包装底层 TLS 流，负责：
/// - 仅进行“轻量嗅探日志”：记录首个请求/响应的起始行与少量头部；
/// - 不改写任何 HTTP 语义，彻底透传读写；
/// - 底层 TLS/SNI 已在连接阶段处理，HTTP 细节交由 libgit2 自身的 smart 协议实现处理。
enum HttpOp {
    // GET /info/refs?service=git-upload-pack
    InfoRefsUpload,
    // POST /git-upload-pack
    UploadPack,
    // GET /info/refs?service=git-receive-pack
    InfoRefsReceive,
    // POST /git-receive-pack
    ReceivePack,
}

struct SniffingStream {
    inner: StreamOwned<ClientConnection, TcpStream>,
    host: String,
    port: u16,
    used_fake_sni: bool,
    current_sni: String,
    path: String,
    op: HttpOp,
    cfg: AppConfig,
    // 嗅探缓冲（仅用于日志，最多 2KB）
    wrote_buf: Vec<u8>,
    wrote_logged: bool,
    read_buf: Vec<u8>,
    read_logged: bool,
    read_body_logged: bool,
    // POST 缓冲
    post_buf: Vec<u8>,
    posted: bool,
    // 请求发送标记（GET）
    requested: bool,
    // 响应解析状态
    headers_parsed: bool,
    header_buf: Vec<u8>,
    // 传输编码
    transfer: Option<TransferKind>,
    // 输入原始缓冲（已去掉头部后剩余的 body 原始字节）
    inbuf: Vec<u8>,
    // 解码后可供上层读取的字节
    decoded: Vec<u8>,
    // 分块状态
    chunk_remaining: usize,
    reading_chunk_size: bool,
    chunk_line: Vec<u8>,
    trailer_mode: bool,
    // 内容长度剩余
    content_remaining: usize,
    // EOF
    eof: bool,
    // 403 轮换保护：仅允许轮换一次，避免无限循环
    rotated_once: bool,
    // 若解析到致命 HTTP 状态（如 401 on receive-pack），优先通过 read() 返回该错误
    fatal_error: Option<String>,
}

enum TransferKind {
    Chunked,
    Length,
    Eof,
}

impl SniffingStream {
    fn new(
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
            // 附带打印首个请求的完整请求头（裁剪至 cap）
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
            // 附带打印首个响应的完整响应头（裁剪至 cap）
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
                        get_push_auth_header().map(|v| format!("Authorization: {}\r\n", v))
                    } else { None };
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
                    // 记录请求首部用于嗅探日志
                    self.wrote_buf.extend_from_slice(req.as_bytes());
                    self.inner.write_all(req.as_bytes())?;
                    self.inner.flush()?;
                    // 触发一次尝试日志输出（仅记录首个请求）
                    self.try_log_request();
                    self.requested = true;
                }
            }
            HttpOp::UploadPack | HttpOp::ReceivePack => {
                if !self.posted {
                    // 若 flush 未触发，此处在读取前补发 POST
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
            get_push_auth_header().map(|v| format!("Authorization: {}\r\n", v))
        } else { None };
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
        // 记录请求首部用于嗅探日志
        self.wrote_buf.extend_from_slice(headers.as_bytes());
        self.inner.write_all(headers.as_bytes())?;
        if len > 0 {
            self.inner.write_all(&self.post_buf)?;
        }
        self.inner.flush()?;
        self.posted = true;
        // 触发一次尝试日志输出（仅记录首个请求）
        self.try_log_request();
        Ok(())
    }

    fn parse_headers_and_setup(&mut self) -> std::io::Result<()> {
        if self.headers_parsed {
            return Ok(());
        }
        // 读取直到 CRLFCRLF
        loop {
            if let Some(pos) = find_double_crlf(&self.header_buf) {
                let header = self.header_buf[..pos].to_vec();
                // 剩余视为 body 的起始
                let remaining = self.header_buf[pos..].to_vec();
                // remove leading CRLFCRLF from remaining for body start
                // find_double_crlf 返回的是头结束后下标，已经指向 body 开始，无需再跳
                self.inbuf.extend_from_slice(&remaining);
                // 解析响应状态与头
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
                            // 解析形如 HTTP/1.1 200 OK
                            let parts: Vec<&str> = line.split_whitespace().collect();
                            if parts.len() >= 2 {
                                if let Ok(code) = parts[1].parse::<u16>() { status_code = Some(code); }
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
                    // 若发生在 push(receive-pack)阶段，直接将其转换为显式错误，避免上层报 "bad packet length"
                    if matches!(self.op, HttpOp::InfoRefsReceive | HttpOp::ReceivePack) {
                        let realm = www_authenticate.as_deref().unwrap_or("");
                        let msg = if !realm.is_empty() {
                            format!("HTTP 401 Unauthorized ({}). Authentication required for git-receive-pack; please provide credentials.", realm)
                        } else {
                            "HTTP 401 Unauthorized. Authentication required for git-receive-pack; please provide credentials.".to_string()
                        };
                        self.fatal_error = Some(msg);
                    }
                }
                // 403 自动 SNI 轮换（仅限 InfoRefs 阶段；开启 sni_rotate_on_403；每个流仅尝试一次）。
                // 即便 TLS 层已经因失败回退到了真实 SNI，也允许再尝试一次“切换到另一个伪 SNI”（若存在）。
                if matches!(self.op, HttpOp::InfoRefsUpload | HttpOp::InfoRefsReceive) {
                    if let Some(403) = status_code {
                        if self.cfg.http.sni_rotate_on_403 && !self.rotated_once {
                            tracing::debug!(target="git.transport.http", host=%self.host, sni=%self.current_sni, "received 403, try rotate SNI and retry once");
                            // 发起一次新的 TLS 连接，选择不同的候选 SNI（尽力避免重复当前）
                            if let Ok((new_stream, new_used_fake, new_sni)) = Self::reconnect_with_rotated_sni(&self.cfg, &self.host, self.port, &self.current_sni) {
                                // 切换底层连接，并重发 GET
                                self.inner = new_stream;
                                self.used_fake_sni = new_used_fake;
                                self.current_sni = new_sni;
                                self.rotated_once = true;
                                // 清空已读缓冲，重新发请求
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
                                // 重新发送请求
                                self.ensure_request_sent()?;
                                // 再次循环读取新响应头
                                continue;
                            }
                        }
                    }
                    // 若 2xx 且 SNI 为伪值，记录最近一次成功的 SNI，便于后续优先使用
                    if let Some(code) = status_code { if (200..300).contains(&code) && self.used_fake_sni {
                        set_last_good_sni(&self.host, &self.current_sni);
                    }}
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
                // consume trailers until CRLFCRLF
                if let Some(pos) = find_double_crlf(&self.inbuf) {
                    // all trailers consumed
                    // drain up to pos
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
                // find CRLF
                if let Some(idx) = find_crlf(&self.inbuf) {
                    let line = self.inbuf.drain(..idx + 2).collect::<Vec<u8>>();
                    // line includes CRLF at end; exclude it
                    let line_no_crlf = &line[..line.len() - 2];
                    // strip any chunk extensions after ';'
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
            // read chunk data
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
                // expect CRLF after chunk
                if self.inbuf.len() < 2 {
                    let _ = self.read_more()?;
                }
                if self.inbuf.len() >= 2 {
                    if &self.inbuf[..2] == b"\r\n" {
                        self.inbuf.drain(..2);
                    }
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
}
// push 阶段的授权头（线程局部）。值应为完整的值，如 "Basic base64(user:pass)"。
thread_local! { static PUSH_AUTH: RefCell<Option<String>> = RefCell::new(None); }

pub fn set_push_auth_header_value(v: Option<String>) {
    PUSH_AUTH.with(|h| { *h.borrow_mut() = v; });
}

fn get_push_auth_header() -> Option<String> {
    PUSH_AUTH.with(|h| h.borrow().clone())
}

fn find_double_crlf(buf: &[u8]) -> Option<usize> {
    // 返回头部结束后紧随 body 的起始索引（含 CRLFCRLF 整个序列长度）
    buf.windows(4).position(|w| w == b"\r\n\r\n").map(|i| i + 4)
}

fn find_crlf(buf: &[u8]) -> Option<usize> {
    buf.windows(2).position(|w| w == b"\r\n")
}

fn parse_http_header_first_line_and_host(header: &[u8]) -> (String, String, bool) {
    let mut line1 = String::new();
    let mut host = String::new();
    let mut has_auth = false;
    let text = match std::str::from_utf8(header) {
        Ok(s) => s,
        Err(_) => return (line1, host, false),
    };
    for (i, line) in text.split("\r\n").enumerate() {
        if i == 0 {
            line1 = line.trim().to_string();
            continue;
        }
        let mut parts = line.splitn(2, ':');
        if let (Some(k), Some(v)) = (parts.next(), parts.next()) {
            if k.eq_ignore_ascii_case("host") {
                host = v.trim().to_string();
            }
            if k.eq_ignore_ascii_case("authorization") {
                has_auth = true;
            }
        }
    }
    (line1, host, has_auth)
}

impl Read for SniffingStream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        tracing::debug!(target="git.transport", host=%self.host, read_buf_len=%buf.len(), "stream read attempt");
        // 对应操作时机：InfoRefs 在第一次读取前先发 GET；UploadPack 在第一次读取前确保 POST 已发。
        self.ensure_request_sent()?;

        // 若还未解析响应头，先解析
        if !self.headers_parsed {
            self.parse_headers_and_setup()?;
        }

        // 若解析阶段已经判定为致命错误，则直接返回错误，避免后续协议层解析误报
        if let Some(msg) = self.fatal_error.clone() {
            return Err(std::io::Error::new(std::io::ErrorKind::PermissionDenied, msg));
        }

        // 若有已解码数据，先用之
        if !self.decoded.is_empty() {
            let n = self.decoded.len().min(buf.len());
            if !self.read_body_logged {
                log_body_preview(
                    &self.decoded[..n.min(32)],
                    &self.host,
                    "first decoded bytes",
                );
                self.read_body_logged = true;
            }
            buf[..n].copy_from_slice(&self.decoded[..n]);
            self.decoded.drain(..n);
            // 记录响应首部（从 header_buf）
            if !self.read_logged {
                let cap = 2048usize;
                let upto = self.header_buf.len().min(cap);
                self.read_buf.extend_from_slice(&self.header_buf[..upto]);
                self.try_log_response();
            }
            return Ok(n);
        }

        // 填充解码缓冲
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
        // 阻塞式等待：循环尝试读取并解码，直到获得至少 1 字节或 EOF
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
                }
                return Ok(n);
            }
            if self.eof {
                return Ok(0);
            }
            // 继续阻塞读取更多字节
            let _ = self.read_more()?;
        }
    }
}

impl Write for SniffingStream {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        tracing::debug!(target="git.transport", host=%self.host, write_len=%buf.len(), "stream write attempt");
        match self.op {
            HttpOp::InfoRefsUpload | HttpOp::InfoRefsReceive => {
                // GET 模式下，libgit2 不期望我们在 write() 时发送任何内容；直接忽略写入（返回长度）即可
                Ok(buf.len())
            }
            HttpOp::UploadPack | HttpOp::ReceivePack => {
                // 缓冲 POST body
                self.post_buf.extend_from_slice(buf);
                Ok(buf.len())
            }
        }
    }
    fn flush(&mut self) -> std::io::Result<()> {
        // UploadPack 在 flush 时发送 POST
        if matches!(self.op, HttpOp::UploadPack | HttpOp::ReceivePack) {
            if !self.posted {
                self.send_post()?;
            }
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

impl SniffingStream {}
impl SniffingStream {
    /// 尝试以不同的 SNI 重新建立 TLS 连接，优先随机选择不同于 current_sni 的伪 SNI 候选；若失败则返回错误。
    fn reconnect_with_rotated_sni(
        _cfg: &AppConfig,
        host: &str,
        port: u16,
        current_sni: &str,
    ) -> Result<(StreamOwned<ClientConnection, TcpStream>, bool, String), Error> {
        // 每次轮换加载“最新配置”
        let cfg_now = load_or_init().unwrap_or_else(|_| AppConfig::default());
        // 若代理存在或未启用 fake SNI：直接真实域
        let present = proxy_present();
        if present || !cfg_now.http.fake_sni_enabled {
            let server_name = ServerName::try_from(host).map_err(|_| Error::from_str("invalid real host"))?;
            let tls_cfg: Arc<ClientConfig> = Arc::new(create_client_config(&cfg_now.tls));
            let addr = format!("{host}:{port}");
            let tcp = TcpStream::connect(addr).map_err(|e| Error::from_str(&format!("tcp connect: {e}")))?;
            tcp.set_nodelay(true).ok();
            let mut conn = ClientConnection::new(tls_cfg, server_name).map_err(|e| Error::from_str(&format!("tls client: {e}")))?;
            conn.complete_io(&mut &tcp).map_err(|e| Error::from_str(&format!("tls handshake: {e}")))?;
            let mut stream = StreamOwned::new(conn, tcp);
            let _ = stream.flush();
            tracing::debug!(target="git.transport.http", host=%host, new_sni=%host, used_fake=false, "reconnect with real SNI due to proxy/disabled");
            return Ok((stream, false, host.to_string()));
        }

        // 构造候选（仅使用 fake_sni_hosts；严格遵守用户列表），排除当前 SNI
        let mut candidates: Vec<String> = Vec::new();
        for h in cfg_now.http.fake_sni_hosts.iter() {
            let h = h.trim();
            if !h.is_empty() && h != current_sni { candidates.push(h.to_string()); }
        }
        if candidates.is_empty() {
            // 无可替换候选，直接回退真实域
            let server_name = ServerName::try_from(host).map_err(|_| Error::from_str("invalid real host"))?;
            let tls_cfg: Arc<ClientConfig> = Arc::new(create_client_config(&cfg_now.tls));
            let addr = format!("{host}:{port}");
            let tcp = TcpStream::connect(addr).map_err(|e| Error::from_str(&format!("tcp connect: {e}")))?;
            tcp.set_nodelay(true).ok();
            let mut conn = ClientConnection::new(tls_cfg, server_name).map_err(|e| Error::from_str(&format!("tls client: {e}")))?;
            conn.complete_io(&mut &tcp).map_err(|e| Error::from_str(&format!("tls handshake: {e}")))?;
            let mut stream = StreamOwned::new(conn, tcp);
            let _ = stream.flush();
            tracing::debug!(target="git.transport.http", host=%host, new_sni=%host, used_fake=false, "no alternative fake SNI, fallback real");
            return Ok((stream, false, host.to_string()));
        }
        // 随机选择一个不同于当前的候选
        let mut rng = rand::thread_rng();
        let pick = candidates.choose(&mut rng).cloned().unwrap_or_else(|| candidates[0].clone());

        let server_name = ServerName::try_from(pick.as_str()).map_err(|_| Error::from_str("invalid sni host"))?;
        // 伪 SNI 时按真实 host 验证
        let tls_cfg: Arc<ClientConfig> = Arc::new(create_client_config_with_expected_name(&cfg_now.tls, host));
        let addr = format!("{host}:{port}");
        let tcp = TcpStream::connect(addr).map_err(|e| Error::from_str(&format!("tcp connect: {e}")))?;
        tcp.set_nodelay(true).ok();
        let mut conn = ClientConnection::new(tls_cfg, server_name).map_err(|e| Error::from_str(&format!("tls client: {e}")))?;
        conn.complete_io(&mut &tcp).map_err(|e| Error::from_str(&format!("tls handshake: {e}")))?;
        let mut stream = StreamOwned::new(conn, tcp);
        let _ = stream.flush();
        tracing::debug!(target="git.transport.http", host=%host, new_sni=%pick, used_fake=true, "reconnect with rotated SNI ok");
        return Ok((stream, true, pick));
    }
}
fn log_body_preview(body: &[u8], host: &str, msg: &str) {
    let n = body.len();
    let hex: String = body.iter().map(|b| format!("{:02x}", b)).collect();
    let ascii = body
        .iter()
        .map(|&b| {
            if b.is_ascii_graphic() || b == b' ' {
                b as char
            } else {
                '.'
            }
        })
        .collect::<String>();
    tracing::debug!(target="git.transport.http", host=%host, bytes=%n, hex=%hex, ascii=%ascii, "{}", msg);
}

impl CustomHttpsSubtransport {
    fn new(cfg: AppConfig) -> Self {
        let tls = Arc::new(create_client_config(&cfg.tls));
        Self { cfg, tls }
    }

    /// 按配置计算使用的 SNI 主机名（可能为伪 SNI），委托公共工具函数
    fn compute_sni(&self, real_host: &str) -> (String, bool) {
        let present = proxy_present();
        let (sni, used_fake) = decide_sni_host_with_proxy(&self.cfg, false, real_host, present);
        tracing::debug!(target="git.transport", real_host=%real_host, sni=%sni, used_fake=%used_fake, proxy_present=%present, "decided SNI host");
        (sni, used_fake)
    }

    // 早期策略：区分错误类型决定是否回退。实测中多数站点在伪 SNI 下会返回证书名不匹配，
    // 若不回退将直接失败。因此调整为：只要是伪 SNI 且首握手失败，无论错误类型，均尝试回退一次。

    /// 尝试建立 TLS 连接；若使用伪 SNI 的非证书类 I/O 失败，则回退到真实 SNI 再试一次。
    /// 返回值中的 bool 表示当前返回的连接是否仍在使用伪 SNI（用于后续 HTTP 层回退判断）。
    fn connect_tls_with_fallback(
        &self,
        host: &str,
        port: u16,
    ) -> Result<(StreamOwned<ClientConnection, TcpStream>, bool, String), Error> {
        tracing::debug!(target="git.transport", host=%host, port=%port, "begin tcp connect");
        // 先 TCP 直连
        let addr = format!("{host}:{port}");
        let tcp = TcpStream::connect(addr).map_err(|e| {
            tracing::debug!(target="git.transport", host=%host, port=%port, error=%e.to_string(), "tcp connect failed");
            Error::from_str(&format!("tcp connect: {e}"))
        })?;
        tcp.set_nodelay(true).ok();

        // 计算 SNI
        let (sni, used_fake) = self.compute_sni(host);
        let server_name =
            ServerName::try_from(sni.as_str()).map_err(|_| Error::from_str("invalid sni host"))?;

        // 选择证书验证配置：如果使用了伪 SNI，则在白名单验证阶段按真实主机名检查
        let tls_cfg: Arc<ClientConfig> = if used_fake {
            Arc::new(create_client_config_with_expected_name(&self.cfg.tls, host))
        } else {
            self.tls.clone()
        };

        // 先尝试以选定 SNI 完成握手
        tracing::debug!(target="git.transport", host=%host, port=%port, sni=%sni, used_fake=%used_fake, "start tls handshake");
        let mut conn = ClientConnection::new(tls_cfg.clone(), server_name)
            .map_err(|e| {
                tracing::debug!(target="git.transport", host=%host, port=%port, error=%e.to_string(), "tls client create failed");
                Error::from_str(&format!("tls client: {e}"))
            })?;
        // 进行一次握手驱动
        match conn.complete_io(&mut &tcp) {
            Ok(_) => {
                tracing::debug!(target="git.transport", host=%host, port=%port, used_fake=%used_fake, "tls handshake ok");
                let mut stream = StreamOwned::new(conn, tcp);
                let _ = stream.flush();
                return Ok((stream, used_fake, sni));
            }
            Err(err) => {
                tracing::debug!(target="git.transport", host=%host, port=%port, used_fake=%used_fake, error=%err.to_string(), "tls handshake failed");
                // 若是伪 SNI，则无论错误类型都回退一次
                if used_fake {
                    tracing::debug!(target="git.transport", host=%host, port=%port, "fake SNI failed, fallback to real SNI: {err}");
                    let addr2 = format!("{host}:{port}");
                    let tcp2 = TcpStream::connect(addr2).map_err(|e| {
                        tracing::debug!(target="git.transport", host=%host, port=%port, error=%e.to_string(), "tcp reconnect for real sni failed");
                        Error::from_str(&format!("tcp connect: {e}"))
                    })?;
                    tcp2.set_nodelay(true).ok();
                    let real_server = ServerName::try_from(host)
                        .map_err(|_| Error::from_str("invalid real host"))?;
                    let mut conn2 = ClientConnection::new(self.tls.clone(), real_server)
                        .map_err(|e| {
                            tracing::debug!(target="git.transport", host=%host, port=%port, error=%e.to_string(), "tls client create (real) failed");
                            Error::from_str(&format!("tls client: {e}"))
                        })?;
                    match conn2.complete_io(&mut &tcp2) {
                        Ok(_) => {
                            tracing::debug!(target="git.transport", host=%host, port=%port, "tls handshake (real sni) ok")
                        }
                        Err(e) => {
                            tracing::debug!(target="git.transport", host=%host, port=%port, error=%e.to_string(), "tls handshake (real sni) failed");
                            return Err(Error::from_str(&format!("tls handshake (real sni): {e}")));
                        }
                    }
                    let mut stream2 = StreamOwned::new(conn2, tcp2);
                    let _ = stream2.flush();
                    // TLS 层已回退为真实 SNI，后续无需再做 HTTP 层回退
                    return Ok((stream2, false, host.to_string()));
                } else {
                    return Err(Error::from_str(&format!("tls handshake: {err}")));
                }
            }
        }
    }
}

impl git2::transport::SmartSubtransport for CustomHttpsSubtransport {
    fn action(
        &self,
        url: &str,
        _action: git2::transport::Service,
    ) -> Result<Box<dyn git2::transport::SmartSubtransportStream>, Error> {
        // 解析自定义协议 URL：期望形如 https+custom://host/...
        tracing::debug!(target="git.transport", url=%url, "subtransport action");
        let parsed = Url::parse(url).map_err(|e| {
            tracing::debug!(target="git.transport", url=%url, error=%e.to_string(), "bad url");
            Error::from_str(&format!("bad url: {e}"))
        })?;
        let host = parsed
            .host_str()
            .ok_or_else(|| Error::from_str("missing host"))?;
        let port = parsed.port_or_known_default().unwrap_or(443);
        let path = parsed.path().to_string();

        // 白名单限制：host 必须命中 SAN 白名单之一
        let allowed = self
            .cfg
            .tls
            .san_whitelist
            .iter()
            .any(|p| match_domain(p, host));
        if !allowed {
            tracing::debug!(target="git.transport", host=%host, "host not allowed by SAN whitelist");
            return Err(Error::from_str("host not allowed by SAN whitelist"));
        }

        // 建立 TLS（带伪 SNI -> 真实 SNI 回退）
        tracing::debug!(target="git.transport", host=%host, port=%port, "connecting tls with fallback");
    let (stream, used_fake_sni, sni_used) = self.connect_tls_with_fallback(host, port)?;
        tracing::debug!(target="git.transport", host=%host, port=%port, used_fake_sni=%used_fake_sni, "connected and returning stream");
        // 确定操作类型：libgit2 会分两阶段调用（ls 与交互）；我们自行封装 HTTP smart 协议
        let op = match _action {
            git2::transport::Service::UploadPackLs => HttpOp::InfoRefsUpload,
            git2::transport::Service::UploadPack => HttpOp::UploadPack,
            git2::transport::Service::ReceivePackLs => HttpOp::InfoRefsReceive,
            git2::transport::Service::ReceivePack => HttpOp::ReceivePack,
        };
        // 包一层嗅探器：记录首部与状态
        let wrapped = SniffingStream::new(stream, host.to_string(), port, used_fake_sni, sni_used, path, op, self.cfg.clone());
        tracing::debug!(target="git.transport", host=%host, port=%port, "sniffing stream created");
        Ok(Box::new(wrapped))
    }

    fn close(&self) -> Result<(), Error> {
        Ok(())
    }
}

/// 仅注册一次自定义传输前缀 "https+custom"。注册后，所有该 scheme 的 URL 都会通过本实现建立连接。
static REGISTER_ONCE: OnceLock<()> = OnceLock::new();

pub fn ensure_registered(_cfg: &AppConfig) -> Result<(), Error> {
    let mut err: Option<Error> = None;
    REGISTER_ONCE.get_or_init(|| {
        // 安全：register 需外部同步；我们用 OnceLock 保证只注册一次。
        let r = unsafe {
            transport::register("https+custom", move |remote: &Remote| {
                // HTTP(s) 是无状态 smart 协议：需要启用 stateless-rpc 模式
                let rpc = true;
                // 每次创建传输时加载“最新配置”，避免保存后需重启
                let cfg_now = crate::core::config::loader::load_or_init()
                    .unwrap_or_else(|_| AppConfig::default());
                let sub = CustomHttpsSubtransport::new(cfg_now);
                transport::Transport::smart(remote, rpc, sub)
            })
        };
        if let Err(e) = r {
            err = Some(e);
        }
    });
    if let Some(e) = err {
        return Err(e);
    }
    Ok(())
}

/// 若启用灰度且命中白名单，将 https:// 重写为 https+custom://
pub fn maybe_rewrite_https_to_custom(cfg: &AppConfig, url: &str) -> Option<String> {
    maybe_rewrite_https_to_custom_inner(cfg, url, proxy_present())
}

/// 纯函数：根据是否存在代理(proxy_present)决定是否进行改写，便于测试中指定环境。
fn maybe_rewrite_https_to_custom_inner(
    cfg: &AppConfig,
    url: &str,
    proxy_present: bool,
) -> Option<String> {
    // 仅处理 https://
    if !cfg.http.fake_sni_enabled {
        return None;
    }
    if proxy_present {
        return None;
    }
    let parsed = Url::parse(url).ok()?;
    if parsed.scheme() != "https" {
        return None;
    }
    let host = parsed.host_str()?;
    // 命中白名单才改写
    if !cfg.tls.san_whitelist.iter().any(|p| match_domain(p, host)) {
        return None;
    }
    // 确保路径以 .git 结尾（Git 仓库标准）
    let mut path = parsed.path().to_string();
    if !path.ends_with(".git") {
        path.push_str(".git");
    }
    // 重构 URL 字符串：scheme 改为 https+custom，path 更新
    let query = parsed
        .query()
        .map(|q| format!("?{}", q))
        .unwrap_or_default();
    let fragment = parsed
        .fragment()
        .map(|f| format!("#{}", f))
        .unwrap_or_default();
    let authority = parsed.authority();
    Some(format!(
        "https+custom://{}{}{}{}",
        authority, path, query, fragment
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rewrite_only_when_enabled_and_whitelisted() {
        let mut cfg = AppConfig::default();
        cfg.http.fake_sni_enabled = true;
        cfg.tls.san_whitelist = vec!["github.com".into()];
        let url = "https://github.com/rust-lang/git2-rs";
        let out = maybe_rewrite_https_to_custom_inner(&cfg, url, false).expect("should rewrite");
        assert_eq!(out, "https+custom://github.com/rust-lang/git2-rs.git");

        // 非 https 不改写
        assert!(maybe_rewrite_https_to_custom_inner(&cfg, "http://github.com/", false).is_none());

        // 关闭开关不改写
        cfg.http.fake_sni_enabled = false;
        assert!(maybe_rewrite_https_to_custom_inner(&cfg, url, false).is_none());

        // 非白名单域不改写
        let mut cfg2 = AppConfig::default();
        cfg2.http.fake_sni_enabled = true;
        cfg2.tls.san_whitelist = vec!["example.com".into()];
        assert!(maybe_rewrite_https_to_custom_inner(&cfg2, url, false).is_none());
    }

    #[test]
    fn test_register_once_ok() {
        let cfg = AppConfig::default();
        // 多次调用不应 panic
        let _ = ensure_registered(&cfg);
        let _ = ensure_registered(&cfg);
    }

    #[test]
    fn test_rewrite_disabled_when_proxy_env_present() {
        let mut cfg = AppConfig::default();
        cfg.http.fake_sni_enabled = true;
        let url = "https://github.com/owner/repo";
        // 指定存在代理 -> 不改写
        assert!(maybe_rewrite_https_to_custom_inner(&cfg, url, true).is_none());
    }
}
