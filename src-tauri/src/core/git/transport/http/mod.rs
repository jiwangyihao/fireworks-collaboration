// Split from previous single-file http.rs into smaller modules without changing behavior.
// Public surface from transport remains:
// - struct CustomHttpsSubtransport (used by register.rs)
// - fn set_push_auth_header_value (re-exported to transport::)

use std::net::TcpStream;
use std::io::Write;
use std::sync::Arc;

use git2::Error;
use rustls::StreamOwned;
use rustls::{ClientConfig, ClientConnection, ServerName};
use url::Url;

use crate::core::config::model::AppConfig;
use crate::core::tls::util::{decide_sni_host_with_proxy, match_domain, proxy_present};
use crate::core::tls::verifier::{create_client_config, create_client_config_with_expected_name};

mod auth;
mod util;
mod stream;

pub use auth::set_push_auth_header_value;

/// 自定义 HTTPS 子传输：仅接管 TCP/TLS 建立与可选伪 SNI；HTTP 语义仍由 libgit2 智能传输处理。
pub(super) struct CustomHttpsSubtransport {
    pub(super) cfg: AppConfig,
    pub(super) tls: Arc<ClientConfig>,
}

/// HTTP 操作类型（smart 协议的四种阶段），仅限本模块及子模块使用。
pub(super) enum HttpOp {
    // GET /info/refs?service=git-upload-pack
    InfoRefsUpload,
    // POST /git-upload-pack
    UploadPack,
    // GET /info/refs?service=git-receive-pack
    InfoRefsReceive,
    // POST /git-receive-pack
    ReceivePack,
}

pub(super) enum TransferKind {
    Chunked,
    Length,
    Eof,
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
        let wrapped = stream::SniffingStream::new(
            stream,
            host.to_string(),
            port,
            used_fake_sni,
            sni_used,
            path,
            op,
            self.cfg.clone(),
        );
        tracing::debug!(target="git.transport", host=%host, port=%port, "sniffing stream created");
        Ok(Box::new(wrapped))
    }

    fn close(&self) -> Result<(), Error> { Ok(()) }
}

impl CustomHttpsSubtransport {
    pub(super) fn new(cfg: AppConfig) -> Self {
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

    /// 尝试建立 TLS 连接；若使用伪 SNI 的非证书类 I/O 失败，则回退到真实 SNI 再试一次。
    /// 返回值中的 bool 表示当前返回的连接是否仍在使用伪 SNI（用于后续 HTTP 层回退判断）。
    pub(super) fn connect_tls_with_fallback(
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
