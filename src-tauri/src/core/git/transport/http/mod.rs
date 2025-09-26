// Split from previous single-file http.rs into smaller modules without changing behavior.
// Public surface from transport remains:
// - struct CustomHttpsSubtransport (used by register.rs)
// - fn set_push_auth_header_value (re-exported to transport::)

use std::io::Write;
use std::net::TcpStream;
use std::sync::Arc;

use git2::Error;
use rustls::StreamOwned;
use rustls::{ClientConfig, ClientConnection, ServerName};
use url::Url;

use crate::core::config::model::AppConfig;
use crate::core::git::transport::metrics::{
    finish_and_store, tl_push_fallback_event, tl_reset, tl_set_cert_fp_changed,
    tl_set_fallback_stage, tl_set_used_fake, FallbackEventRecord,
};
use crate::core::git::transport::metrics_enabled;
use crate::core::git::transport::record_certificate;
use crate::core::git::transport::{
    is_fake_disabled, record_fake_attempt, AutoDisableConfig, AutoDisableEvent, DecisionCtx,
    FallbackDecision, FallbackReason, FallbackStage, TimingRecorder,
};
use crate::core::tls::util::{decide_sni_host_with_proxy, match_domain, proxy_present};
use crate::core::tls::verifier::{create_client_config, create_client_config_with_expected_name};

mod auth;
mod stream;
mod util;

pub use auth::set_push_auth_header_value;

// P3.3: Real-Host 验证失败触发的回退统计（Fake -> Real），按原因分类。
use std::sync::atomic::{AtomicU64, Ordering};
static FALLBACK_TLS_TOTAL: AtomicU64 = AtomicU64::new(0);
static FALLBACK_VERIFY_TOTAL: AtomicU64 = AtomicU64::new(0);

fn stage_label(stage: FallbackStage) -> &'static str {
    match stage {
        FallbackStage::Fake => "Fake",
        FallbackStage::Real => "Real",
        FallbackStage::Default => "Default",
        FallbackStage::None => "None",
    }
}

fn reason_label(reason: FallbackReason) -> &'static str {
    match reason {
        FallbackReason::EnterFake => "EnterFake",
        FallbackReason::FakeHandshakeError => "FakeHandshakeError",
        FallbackReason::SkipFakePolicy => "SkipFakePolicy",
        FallbackReason::RealFailed => "RealFailed",
    }
}

#[cfg(test)]
mod injection {
    use super::FallbackStage;
    use git2::Error;
    use std::collections::VecDeque;
    use std::sync::{Mutex, OnceLock};

    fn fake_queue() -> &'static Mutex<VecDeque<String>> {
        static Q: OnceLock<Mutex<VecDeque<String>>> = OnceLock::new();
        Q.get_or_init(|| Mutex::new(VecDeque::new()))
    }

    fn real_queue() -> &'static Mutex<VecDeque<String>> {
        static Q: OnceLock<Mutex<VecDeque<String>>> = OnceLock::new();
        Q.get_or_init(|| Mutex::new(VecDeque::new()))
    }

    pub fn inject(stage: FallbackStage, msg: String) {
        let queue = match stage {
            FallbackStage::Fake => fake_queue(),
            FallbackStage::Real => real_queue(),
            _ => return,
        };
        if let Ok(mut guard) = queue.lock() {
            guard.push_back(msg);
        }
    }

    pub fn take(stage: FallbackStage) -> Option<Error> {
        let queue = match stage {
            FallbackStage::Fake => fake_queue(),
            FallbackStage::Real => real_queue(),
            _ => return None,
        };
        let msg = queue.lock().ok().and_then(|mut g| g.pop_front());
        msg.map(|m| Error::from_str(&m))
    }

    pub fn reset() {
        if let Ok(mut g) = fake_queue().lock() {
            g.clear();
        }
        if let Ok(mut g) = real_queue().lock() {
            g.clear();
        }
    }
}

#[cfg(test)]
pub fn test_inject_fake_failure(msg: impl Into<String>) {
    injection::inject(FallbackStage::Fake, msg.into());
}

#[cfg(test)]
pub fn test_inject_real_failure(msg: impl Into<String>) {
    injection::inject(FallbackStage::Real, msg.into());
}

#[cfg(test)]
pub fn test_reset_injected_failures() {
    injection::reset();
}
pub fn test_reset_fallback_counters() {
    FALLBACK_TLS_TOTAL.store(0, Ordering::Relaxed);
    FALLBACK_VERIFY_TOTAL.store(0, Ordering::Relaxed);
}
#[cfg(test)]
pub fn test_snapshot_fallback_counters() -> (u64, u64) {
    (
        FALLBACK_TLS_TOTAL.load(Ordering::Relaxed),
        FALLBACK_VERIFY_TOTAL.load(Ordering::Relaxed),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::git::transport::{
        is_fake_disabled, test_auto_disable_guard, test_reset_auto_disable,
        tl_take_fallback_events, AutoDisableConfig, FallbackEventRecord,
    };
    use std::sync::{Mutex, OnceLock};

    fn counter_guard() -> &'static Mutex<()> {
        static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
        GUARD.get_or_init(|| Mutex::new(()))
    }

    #[test]
    fn classify_pin_error_as_verify() {
        let _lock = counter_guard().lock().unwrap();
        test_reset_fallback_counters();
        let category = classify_and_count_fallback("cert_fp_pin_mismatch");
        assert_eq!(category, "Verify");
        let (tls_total, verify_total) = test_snapshot_fallback_counters();
        assert_eq!(tls_total, 0);
        assert_eq!(verify_total, 1);
    }

    #[test]
    fn classify_tls_error_falls_back_to_tls() {
        let _lock = counter_guard().lock().unwrap();
        test_reset_fallback_counters();
        let category = classify_and_count_fallback("handshake failure");
        assert_eq!(category, "Tls");
        let (tls_total, verify_total) = test_snapshot_fallback_counters();
        assert_eq!(tls_total, 1);
        assert_eq!(verify_total, 0);
    }

    #[test]
    fn fallback_transition_emits_events_and_triggers_auto_disable() {
        let _auto_guard = test_auto_disable_guard().lock().unwrap();
        let _lock = counter_guard().lock().unwrap();
        test_reset_fallback_counters();
        test_reset_auto_disable();
        test_reset_injected_failures();
        let cfg = AppConfig::default();
        let sub = CustomHttpsSubtransport::new(cfg.clone());
        let mut auto_disable_seen = false;
        for i in 0..5 {
            test_inject_fake_failure(format!("fake-fail-{i}"));
            test_inject_real_failure(format!("real-fail-{i}"));
            let res = sub.connect_tls_with_fallback("github.com", 443);
            assert!(res.is_err(), "expected injected failure on attempt {i}");
            let events = tl_take_fallback_events();
            assert!(
                events.iter().any(|evt| matches!(
                    evt,
                    FallbackEventRecord::Transition { from, to, reason }
                        if *from == "Fake" && *to == "Real" && reason == "FakeHandshakeError"
                )),
                "missing Fake->Real transition on attempt {i}"
            );
            if events.iter().any(|evt| {
                matches!(
                    evt,
                    FallbackEventRecord::AutoDisable {
                        enabled: true,
                        threshold_pct,
                        cooldown_secs,
                    }
                    if *threshold_pct == cfg.http.auto_disable_fake_threshold_pct
                        && *cooldown_secs
                            == cfg.http.auto_disable_fake_cooldown_sec as u32
                )
            }) {
                auto_disable_seen = true;
            }
        }
        assert!(auto_disable_seen, "auto-disable event not observed");
        let auto_cfg = AutoDisableConfig::from_http_cfg(&cfg.http);
        assert!(
            is_fake_disabled(&auto_cfg),
            "fake SNI should be disabled after trigger"
        );
        test_reset_auto_disable();
    }
}

fn classify_and_count_fallback(err_msg: &str) -> &'static str {
    let em = err_msg.to_ascii_lowercase();
    // rustls 错误文本约定：General("SAN whitelist mismatch") 或域名不符等 -> Verify；其他握手/IO -> Tls
    if em.contains("whitelist")
        || em.contains("san")
        || em.contains("name")
        || em.contains("verify")
        || em.contains("pin")
    {
        FALLBACK_VERIFY_TOTAL.fetch_add(1, Ordering::Relaxed);
        "Verify"
    } else {
        FALLBACK_TLS_TOTAL.fetch_add(1, Ordering::Relaxed);
        "Tls"
    }
}

#[cfg(test)]
pub fn test_classify_and_count_fallback(err_msg: &str) -> &'static str {
    classify_and_count_fallback(err_msg)
}

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

    fn close(&self) -> Result<(), Error> {
        Ok(())
    }
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
        let mut timing = TimingRecorder::new();
        tl_reset();
        tracing::debug!(target="git.transport", host=%host, port=%port, "begin tcp connect");
        let auto_cfg = AutoDisableConfig::from_http_cfg(&self.cfg.http);
        let runtime_fake_disabled = is_fake_disabled(&auto_cfg);
        if runtime_fake_disabled {
            tracing::warn!(
                target="git.transport",
                host=%host,
                port=%port,
                "adaptive_tls_fake temporarily disabled by runtime safeguard"
            );
        }
        let mut decision = FallbackDecision::initial(&DecisionCtx {
            policy_allows_fake: self.cfg.http.fake_sni_enabled,
            runtime_fake_disabled,
        });

        let mut last_error: Option<Error> = None;
        let emit_auto_disable_event = |evt: AutoDisableEvent| match evt {
            AutoDisableEvent::Triggered {
                threshold_pct,
                cooldown_secs,
            } => {
                tracing::warn!(
                    target="git.transport",
                    host=%host,
                    port=%port,
                    threshold_pct,
                    cooldown_secs,
                    "adaptive_tls_fake auto-disable triggered"
                );
                tl_push_fallback_event(FallbackEventRecord::AutoDisable {
                    enabled: true,
                    threshold_pct,
                    cooldown_secs,
                });
            }
            AutoDisableEvent::Recovered => {
                tracing::debug!(
                    target="git.transport",
                    host=%host,
                    port=%port,
                    "adaptive_tls_fake auto-disable recovered"
                );
                tl_push_fallback_event(FallbackEventRecord::AutoDisable {
                    enabled: false,
                    threshold_pct: 0,
                    cooldown_secs: 0,
                });
            }
        };

        // single attempt closure reused across Fake / Real
        let mut attempt = |stage: FallbackStage,
                           host: &str,
                           port: u16|
         -> Result<
            (StreamOwned<ClientConnection, TcpStream>, bool, String),
            Error,
        > {
            timing.mark_connect_start();
            #[cfg(test)]
            {
                if let Some(err) = injection::take(stage) {
                    tracing::debug!(
                        target="git.transport",
                        host=%host,
                        port=%port,
                        stage=?stage,
                        "tls handshake failure injected"
                    );
                    return Err(err);
                }
            }
            let addr = format!("{host}:{port}");
            let tcp = TcpStream::connect(addr).map_err(|e| {
                tracing::debug!(target="git.transport", host=%host, port=%port, error=%e.to_string(), stage=?stage, "tcp connect failed");
                Error::from_str(&format!("tcp connect: {e}"))
            })?;
            tcp.set_nodelay(true).ok();
            timing.mark_connect_end();

            let (sni, used_fake) = match stage {
                FallbackStage::Fake => self.compute_sni(host),
                FallbackStage::Real | FallbackStage::Default | FallbackStage::None => {
                    (host.to_string(), false)
                }
            };
            timing.mark_tls_start();
            let server_name = ServerName::try_from(sni.as_str())
                .map_err(|_| Error::from_str("invalid sni host"))?;
            let tls_cfg: Arc<ClientConfig> = if used_fake {
                Arc::new(create_client_config_with_expected_name(&self.cfg.tls, host))
            } else {
                self.tls.clone()
            };
            let rhv = self.cfg.tls.real_host_verify_enabled;
            tracing::debug!(target="git.transport", host=%host, port=%port, sni=%sni, used_fake=%used_fake, stage=?stage, real_host_verify=%rhv, "start tls handshake");
            let mut conn = ClientConnection::new(tls_cfg.clone(), server_name)
                .map_err(|e| Error::from_str(&format!("tls client: {e}")))?;
            match conn.complete_io(&mut &tcp) {
                Ok(_) => {
                    timing.mark_tls_end();
                    let mut stream = StreamOwned::new(conn, tcp);
                    let _ = stream.flush();
                    Ok((stream, used_fake, sni))
                }
                Err(err) => {
                    let em = err.to_string();
                    // 若当前是 Fake 阶段，记录一次回退统计并打印锚点日志
                    if matches!(stage, FallbackStage::Fake) {
                        let reason = classify_and_count_fallback(&em);
                        tracing::debug!(target="git.transport", host=%host, port=%port, used_fake=%used_fake, stage=?stage, reason=%reason, "adaptive_tls_fallback: fake->real");
                    }
                    tracing::debug!(target="git.transport", host=%host, port=%port, used_fake=%used_fake, stage=?stage, error=%em, "tls handshake failed");
                    Err(Error::from_str(&format!("tls handshake: {err}")))
                }
            }
        };

        // Drive attempts based on decision chain
        loop {
            let stage = decision.stage();
            if matches!(stage, FallbackStage::Default) && last_error.is_some() {
                tl_set_fallback_stage(stage_label(stage));
                if metrics_enabled() {
                    finish_and_store(&mut timing);
                }
                return Err(last_error.take().unwrap());
            }
            match attempt(stage, host, port) {
                Ok(ok) => {
                    if matches!(stage, FallbackStage::Fake) {
                        if let Some(evt) = record_fake_attempt(&auto_cfg, false) {
                            emit_auto_disable_event(evt);
                        }
                    }
                    if metrics_enabled() {
                        finish_and_store(&mut timing);
                    }
                    // record used fake & stage
                    tl_set_used_fake(ok.1);
                    tl_set_fallback_stage(stage_label(stage));
                    // fingerprint recording (best-effort)
                    if let Some(certs) = ok.0.conn.peer_certificates() {
                        if let Some((changed, _spki, _cert)) = record_certificate(host, &certs[..])
                        {
                            if changed {
                                tl_set_cert_fp_changed(true);
                            }
                        }
                    }
                    return Ok(ok);
                }
                Err(e) => {
                    if matches!(stage, FallbackStage::Fake) {
                        if let Some(evt) = record_fake_attempt(&auto_cfg, true) {
                            emit_auto_disable_event(evt);
                        }
                    }
                    if let Some(tr) = decision.advance_on_error() {
                        last_error = Some(e);
                        tl_push_fallback_event(FallbackEventRecord::Transition {
                            from: stage_label(tr.from),
                            to: stage_label(tr.to),
                            reason: reason_label(tr.reason).to_string(),
                        });
                        continue;
                    } else {
                        tl_set_fallback_stage(stage_label(stage));
                        if metrics_enabled() {
                            finish_and_store(&mut timing);
                        }
                        return Err(e);
                    }
                }
            }
        }
    }
}
