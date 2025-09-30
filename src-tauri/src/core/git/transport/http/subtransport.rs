use std::io::Write;
use std::net::{SocketAddr, TcpStream};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use git2::Error;
use rustls::StreamOwned;
use rustls::{ClientConfig, ClientConnection, ServerName};
use url::Url;

use crate::core::config::model::AppConfig;
use crate::core::git::transport::metrics::{
    finish_and_store, tl_push_fallback_event, tl_reset, tl_set_cert_fp_changed,
    tl_set_fallback_stage, tl_set_ip_selection, tl_set_used_fake, FallbackEventRecord,
};
use crate::core::git::transport::metrics_enabled;
use crate::core::git::transport::record_certificate;
use crate::core::git::transport::{
    is_fake_disabled, record_fake_attempt, AutoDisableConfig, AutoDisableEvent, DecisionCtx,
    FallbackDecision, FallbackStage, TimingRecorder,
};
use crate::core::ip_pool::{self, IpOutcome, IpPool, IpSelectionStrategy, IpStat};
use crate::core::tls::util::{decide_sni_host_with_proxy, match_domain, proxy_present};
use crate::core::tls::verifier::{create_client_config, create_client_config_with_expected_name};

use super::fallback::{classify_and_count_fallback, reason_label, stage_label};
use super::util::format_ip_sources;
use super::{stream, HttpOp};

pub(in crate::core::git::transport) struct CustomHttpsSubtransport {
    pub(super) cfg: AppConfig,
    pub(super) tls: Arc<ClientConfig>,
    pub(super) pool: Arc<Mutex<IpPool>>,
}

#[cfg(not(feature = "tauri-app"))]
pub mod testing {
    //! Wrapper that exposes the custom HTTP subtransport to integration tests.
    use super::*;

    pub struct TestSubtransport {
        inner: CustomHttpsSubtransport,
    }

    impl TestSubtransport {
        pub fn new(cfg: AppConfig) -> Self {
            Self {
                inner: CustomHttpsSubtransport::new(cfg),
            }
        }

        pub fn connect_tls_with_fallback(
            &self,
            host: &str,
            port: u16,
        ) -> Result<(StreamOwned<ClientConnection, TcpStream>, bool, String), Error> {
            self.inner.connect_tls_with_fallback(host, port)
        }
    }
}

impl git2::transport::SmartSubtransport for CustomHttpsSubtransport {
    fn action(
        &self,
        url: &str,
        action: git2::transport::Service,
    ) -> Result<Box<dyn git2::transport::SmartSubtransportStream>, Error> {
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

        tracing::debug!(target="git.transport", host=%host, port=%port, "connecting tls with fallback");
        let (stream, used_fake_sni, sni_used) = self.connect_tls_with_fallback(host, port)?;
        tracing::debug!(target="git.transport", host=%host, port=%port, used_fake_sni=%used_fake_sni, "connected and returning stream");

        let op = match action {
            git2::transport::Service::UploadPackLs => HttpOp::InfoRefsUpload,
            git2::transport::Service::UploadPack => HttpOp::UploadPack,
            git2::transport::Service::ReceivePackLs => HttpOp::InfoRefsReceive,
            git2::transport::Service::ReceivePack => HttpOp::ReceivePack,
        };

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
    pub(in crate::core::git::transport) fn new(cfg: AppConfig) -> Self {
        let tls = Arc::new(create_client_config(&cfg.tls));
        let pool = ip_pool::global::obtain_global_pool();
        Self { cfg, tls, pool }
    }

    fn compute_sni(&self, real_host: &str) -> (String, bool) {
        let present = proxy_present();
        let (sni, used_fake) = decide_sni_host_with_proxy(&self.cfg, false, real_host, present);
        tracing::debug!(target="git.transport", real_host=%real_host, sni=%sni, used_fake=%used_fake, proxy_present=%present, "decided SNI host");
        (sni, used_fake)
    }

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

        let selection = {
            let guard = self.pool.lock().expect("ip pool mutex poisoned");
            guard.pick_best_blocking(host, port)
        };
        let strategy_label = match selection.strategy() {
            IpSelectionStrategy::Cached => "Cached",
            IpSelectionStrategy::SystemDefault => "SystemDefault",
        };
        let candidates: Vec<IpStat> = selection.iter_candidates().cloned().collect();
        if !candidates.is_empty() {
            tracing::debug!(
                target="git.transport",
                host=%host,
                port=%port,
                strategy=strategy_label,
                candidates=candidates.len(),
                "ip pool provided candidates"
            );
        }

        let mut last_error: Option<Error> = None;
        let mut selection_success = false;
        let mut used_candidate_stat: Option<IpStat> = None;

        let record_candidate_outcome = |stat: &IpStat, outcome: IpOutcome| {
            if !matches!(selection.strategy(), IpSelectionStrategy::Cached) {
                return;
            }
            match self.pool.lock() {
                Ok(pool_guard) => pool_guard.report_candidate_outcome(host, port, stat, outcome),
                Err(_) => tracing::warn!(
                    target = "git.transport",
                    "ip pool mutex poisoned while reporting candidate outcome"
                ),
            }
        };

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

        enum ConnectTarget<'a> {
            System,
            Direct(&'a IpStat),
        }

        struct StageResult {
            stream: StreamOwned<ClientConnection, TcpStream>,
            used_fake: bool,
            sni: String,
            candidate: Option<IpStat>,
        }

        let mut attempt = |stage: FallbackStage,
                           host: &str,
                           port: u16,
                           target: ConnectTarget<'_>|
         -> Result<
            (StreamOwned<ClientConnection, TcpStream>, bool, String),
            Error,
        > {
            timing.mark_connect_start();
            #[cfg(any(test, not(feature = "tauri-app")))]
            {
                use super::fallback::injection;
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

            let (tcp, candidate_ref) = match target {
                ConnectTarget::System => {
                    let addr = format!("{host}:{port}");
                    (
                        TcpStream::connect(addr.as_str()).map_err(|e| {
                            tracing::debug!(
                                target="git.transport",
                                host=%host,
                                port=%port,
                                stage=?stage,
                                error=%e.to_string(),
                                "tcp connect failed"
                            );
                            Error::from_str(&format!("tcp connect: {e}"))
                        })?,
                        None,
                    )
                }
                ConnectTarget::Direct(stat) => {
                    let addr = SocketAddr::new(stat.candidate.address, stat.candidate.port);
                    tracing::debug!(
                        target="git.transport",
                        host=%host,
                        port=%port,
                        stage=?stage,
                        ip=%stat.candidate.address,
                        candidate_port=stat.candidate.port,
                        latency_ms=?stat.latency_ms,
                        sources=%format_ip_sources(&stat.sources),
                        "attempting ip pool candidate"
                    );
                    (
                        TcpStream::connect_timeout(&addr, Duration::from_millis(500)).map_err(
                            |e| {
                                tracing::debug!(
                                    target="git.transport",
                                    host=%host,
                                    port=%port,
                                    stage=?stage,
                                    ip=%stat.candidate.address,
                                    candidate_port=stat.candidate.port,
                                    error=%e.to_string(),
                                    "tcp connect failed for candidate"
                                );
                                Error::from_str(&format!("tcp connect: {e}"))
                            },
                        )?,
                        Some(stat),
                    )
                }
            };
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
            tracing::debug!(
                target="git.transport",
                host=%host,
                port=%port,
                sni=%sni,
                used_fake=%used_fake,
                stage=?stage,
                real_host_verify=%rhv,
                "start tls handshake"
            );
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
                    if matches!(stage, FallbackStage::Fake) {
                        let reason = classify_and_count_fallback(&em);
                        tracing::debug!(
                            target="git.transport",
                            host=%host,
                            port=%port,
                            used_fake=%used_fake,
                            stage=?stage,
                            reason=%reason,
                            "adaptive_tls_fallback: fake->real"
                        );
                    }
                    if let Some(stat) = candidate_ref {
                        tracing::debug!(
                            target="git.transport",
                            host=%host,
                            port=%port,
                            used_fake=%used_fake,
                            stage=?stage,
                            ip=%stat.candidate.address,
                            candidate_port=stat.candidate.port,
                            error=%em,
                            "tls handshake failed on ip pool candidate"
                        );
                    } else {
                        tracing::debug!(
                            target="git.transport",
                            host=%host,
                            port=%port,
                            used_fake=%used_fake,
                            stage=?stage,
                            error=%em,
                            "tls handshake failed"
                        );
                    }
                    Err(Error::from_str(&format!("tls handshake: {err}")))
                }
            }
        };

        let mut run_stage = |stage: FallbackStage| -> Result<StageResult, Error> {
            let mut last_candidate_err: Option<Error> = None;
            for stat in &candidates {
                match attempt(stage, host, port, ConnectTarget::Direct(stat)) {
                    Ok((stream, used_fake, sni)) => {
                        record_candidate_outcome(stat, IpOutcome::Success);
                        tracing::debug!(
                            target="git.transport",
                            host=%host,
                            port=%port,
                            stage=?stage,
                            ip=%stat.candidate.address,
                            candidate_port=stat.candidate.port,
                            latency_ms=?stat.latency_ms,
                            sources=%format_ip_sources(&stat.sources),
                            "ip pool candidate succeeded"
                        );
                        return Ok(StageResult {
                            stream,
                            used_fake,
                            sni,
                            candidate: Some(stat.clone()),
                        });
                    }
                    Err(err) => {
                        record_candidate_outcome(stat, IpOutcome::Failure);
                        last_candidate_err = Some(err);
                        continue;
                    }
                }
            }
            match attempt(stage, host, port, ConnectTarget::System) {
                Ok((stream, used_fake, sni)) => {
                    if !candidates.is_empty() {
                        tracing::debug!(
                            target="git.transport",
                            host=%host,
                            port=%port,
                            stage=?stage,
                            "ip pool candidates exhausted; using system dns"
                        );
                    }
                    Ok(StageResult {
                        stream,
                        used_fake,
                        sni,
                        candidate: None,
                    })
                }
                Err(err) => {
                    if let Some(prev) = last_candidate_err.as_ref() {
                        tracing::debug!(
                            target="git.transport",
                            host=%host,
                            port=%port,
                            stage=?stage,
                            error=%prev,
                            "system dns attempt failed after ip pool candidates"
                        );
                    }
                    Err(err)
                }
            }
        };

        let final_result: Result<StageResult, Error> = loop {
            let stage = decision.stage();
            if matches!(stage, FallbackStage::Default) && last_error.is_some() {
                tl_set_fallback_stage(stage_label(stage));
                if metrics_enabled() {
                    finish_and_store(&mut timing);
                }
                break Err(last_error.take().unwrap());
            }
            match run_stage(stage) {
                Ok(stage_ok) => {
                    if matches!(stage, FallbackStage::Fake) {
                        if let Some(evt) = record_fake_attempt(&auto_cfg, false) {
                            emit_auto_disable_event(evt);
                        }
                    }
                    if metrics_enabled() {
                        finish_and_store(&mut timing);
                    }
                    tl_set_used_fake(stage_ok.used_fake);
                    tl_set_fallback_stage(stage_label(stage));
                    used_candidate_stat = stage_ok.candidate.clone();
                    selection_success = stage_ok.candidate.is_some();
                    break Ok(stage_ok);
                }
                Err(err) => {
                    if matches!(stage, FallbackStage::Fake) {
                        if let Some(evt) = record_fake_attempt(&auto_cfg, true) {
                            emit_auto_disable_event(evt);
                        }
                    }
                    if let Some(tr) = decision.advance_on_error() {
                        tl_push_fallback_event(FallbackEventRecord::Transition {
                            from: stage_label(tr.from),
                            to: stage_label(tr.to),
                            reason: reason_label(tr.reason).to_string(),
                        });
                        last_error = Some(err);
                        continue;
                    } else {
                        tl_set_fallback_stage(stage_label(stage));
                        if metrics_enabled() {
                            finish_and_store(&mut timing);
                        }
                        break Err(err);
                    }
                }
            }
        };

        let ip_source_label = used_candidate_stat
            .as_ref()
            .map(|stat| format_ip_sources(&stat.sources));
        let ip_latency = used_candidate_stat
            .as_ref()
            .and_then(|stat| stat.latency_ms);
        tl_set_ip_selection(Some(strategy_label), ip_source_label.clone(), ip_latency);

        if matches!(selection.strategy(), IpSelectionStrategy::Cached) {
            let outcome = match &final_result {
                Ok(_) if selection_success => IpOutcome::Success,
                _ => IpOutcome::Failure,
            };
            match self.pool.lock() {
                Ok(pool_guard) => pool_guard.report_outcome(&selection, outcome),
                Err(_) => tracing::warn!(
                    target = "git.transport",
                    "ip pool mutex poisoned while reporting outcome"
                ),
            }
        }

        match final_result {
            Ok(stage_ok) => {
                if let Some(certs) = stage_ok.stream.conn.peer_certificates() {
                    if let Some((changed, _spki, _cert)) = record_certificate(host, &certs[..]) {
                        if changed {
                            tl_set_cert_fp_changed(true);
                        }
                    }
                }
                if let Some(ref stat) = stage_ok.candidate {
                    tracing::debug!(
                        target="git.transport",
                        host=%host,
                        port=%port,
                        ip=%stat.candidate.address,
                        candidate_port=stat.candidate.port,
                        latency_ms=?stat.latency_ms,
                        sources=%format_ip_sources(&stat.sources),
                        "ip pool candidate selected"
                    );
                } else if matches!(selection.strategy(), IpSelectionStrategy::Cached) {
                    tracing::debug!(
                        target="git.transport",
                        host=%host,
                        port=%port,
                        "ip pool selection fell back to system dns"
                    );
                }
                Ok((stage_ok.stream, stage_ok.used_fake, stage_ok.sni))
            }
            Err(err) => Err(err),
        }
    }
}
