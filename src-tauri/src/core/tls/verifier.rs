use std::sync::Arc;

use crate::core::config::model::TlsCfg;
use crate::core::tls::spki::{compute_spki_sha256_b64, SpkiSource};
use base64::Engine;
use rustls::client::{ServerCertVerified, ServerCertVerifier};
use rustls::ClientConfig;
use rustls::{Certificate, Error as TlsError, OwnedTrustAnchor, RootCertStore, ServerName};

pub struct RealHostCertVerifier {
    pub inner: Arc<dyn ServerCertVerifier>,
    pub override_host: Option<String>,
    pub real_host_verify_enabled: bool,
    pub spki_pins: Vec<String>,
}

impl RealHostCertVerifier {
    pub fn new(
        inner: Arc<dyn ServerCertVerifier>,
        override_host: Option<String>,
        real_host_verify_enabled: bool,
        spki_pins: Vec<String>,
    ) -> Self {
        Self {
            inner,
            override_host,
            real_host_verify_enabled,
            spki_pins,
        }
    }
}

impl ServerCertVerifier for RealHostCertVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &Certificate,
        intermediates: &[Certificate],
        server_name: &ServerName,
        scts: &mut dyn Iterator<Item = &[u8]>,
        ocsp_response: &[u8],
        now: std::time::SystemTime,
    ) -> Result<ServerCertVerified, TlsError> {
        // 先使用系统默认验证器验证链路与主机名：
        // - 若 real_host_verify_enabled 且 override_host 存在，则按真实域名构造 ServerName 传给 inner 验证器；
        // - 否则使用传入的 server_name（通常来源于 SNI）。
        if self.real_host_verify_enabled {
            if let Some(expected) = &self.override_host {
                match ServerName::try_from(expected.as_str()) {
                    Ok(exp_name) => {
                        self.inner.verify_server_cert(
                            end_entity,
                            intermediates,
                            &exp_name,
                            scts,
                            ocsp_response,
                            now,
                        )?;
                    }
                    Err(_) => {
                        self.inner.verify_server_cert(
                            end_entity,
                            intermediates,
                            server_name,
                            scts,
                            ocsp_response,
                            now,
                        )?;
                    }
                }
            } else {
                self.inner.verify_server_cert(
                    end_entity,
                    intermediates,
                    server_name,
                    scts,
                    ocsp_response,
                    now,
                )?;
            }
        } else {
            self.inner.verify_server_cert(
                end_entity,
                intermediates,
                server_name,
                scts,
                ocsp_response,
                now,
            )?;
        }

        // P3.4: SPKI Pin 强校验（若配置非空）。在链与主机名验证成功、白名单通过后执行。
        if !self.spki_pins.is_empty() {
            // 仅当 pin 列表全部合法时才执行；否则视为禁用（记录一次调试日志）。
            if let Some(valid_pins) = validate_pins(&self.spki_pins) {
                let (spki_b64, spki_source) = compute_spki_sha256_b64(end_entity);
                let pin_count = valid_pins.len() as u8;
                if !valid_pins.iter().any(|p| p == &spki_b64) {
                    let host_to_log = if let Some(h) = &self.override_host {
                        h.as_str()
                    } else {
                        match server_name {
                            ServerName::DnsName(n) => n.as_ref(),
                            _ => "",
                        }
                    };
                    // 发送结构化事件并返回 Verify 类错误；不触发 Fake->Real 回退。
                    tracing::warn!(target="git.transport", host=%host_to_log, pin_enforced="on", pin_count=%pin_count, cert_spki=%spki_b64, spki_source=%log_spki_source(spki_source), "pin_mismatch");
                    use crate::events::structured::{
                        publish_global, Event as StructuredEvent,
                        StrategyEvent as StructuredStrategyEvent,
                    };
                    publish_global(StructuredEvent::Strategy(
                        StructuredStrategyEvent::CertFpPinMismatch {
                            id: host_to_log.to_string(),
                            host: host_to_log.to_string(),
                            spki_sha256: spki_b64.clone(),
                            pin_count,
                        },
                    ));
                    return Err(TlsError::General("cert_fp_pin_mismatch".into()));
                } else {
                    let host_to_log = if let Some(h) = &self.override_host {
                        h.as_str()
                    } else {
                        match server_name {
                            ServerName::DnsName(n) => n.as_ref(),
                            _ => "",
                        }
                    };
                    tracing::debug!(target="git.transport", host=%host_to_log, pin_enforced="on", pin_count=%pin_count, spki_source=%log_spki_source(spki_source), "pin_match");
                }
            } else {
                let host_to_log = if let Some(h) = &self.override_host {
                    h.as_str()
                } else {
                    match server_name {
                        ServerName::DnsName(n) => n.as_ref(),
                        _ => "",
                    }
                };
                tracing::warn!(target="git.transport", host=%host_to_log, pin_enforced="off", reason="invalid_pins", "pin_disabled_this_conn");
            }
        }
        Ok(ServerCertVerified::assertion())
    }
}

fn build_cert_verifier(tls: &TlsCfg, override_host: Option<String>) -> Arc<dyn ServerCertVerifier> {
    let mut root_store = RootCertStore::empty();
    root_store.add_trust_anchors(webpki_roots::TLS_SERVER_ROOTS.iter().map(|ta| {
        OwnedTrustAnchor::from_subject_spki_name_constraints(
            ta.subject,
            ta.spki,
            ta.name_constraints,
        )
    }));
    let inner = Arc::new(rustls::client::WebPkiVerifier::new(root_store, None));
    Arc::new(RealHostCertVerifier::new(
        inner,
        override_host,
        tls.real_host_verify_enabled,
        tls.spki_pins.clone(),
    ))
}

/// 基于白名单验证器创建 rustls ClientConfig（无客户端证书）
pub fn create_client_config(tls: &TlsCfg) -> ClientConfig {
    // Root store 与 WebPkiVerifier 一致
    let mut root_store = RootCertStore::empty();
    root_store.add_trust_anchors(webpki_roots::TLS_SERVER_ROOTS.iter().map(|ta| {
        OwnedTrustAnchor::from_subject_spki_name_constraints(
            ta.subject,
            ta.spki,
            ta.name_constraints,
        )
    }));

    let mut cfg = ClientConfig::builder()
        .with_safe_defaults()
        .with_root_certificates(root_store)
        .with_no_client_auth();

    // 基于 flags 构造相应验证器
    let verifier = build_cert_verifier(tls, None);
    cfg.dangerous().set_certificate_verifier(verifier);
    cfg
}

/// 基于白名单验证器创建 rustls ClientConfig，但强制基于“期望的真实主机名”做白名单校验。
/// 用途：当 TLS 握手使用“伪 SNI”时，仍希望后续证书白名单与域名一致性检验以真实主机名为准。
pub fn create_client_config_with_expected_name(tls: &TlsCfg, expected_host: &str) -> ClientConfig {
    // Root store 与 WebPkiVerifier 一致
    let mut root_store = RootCertStore::empty();
    root_store.add_trust_anchors(webpki_roots::TLS_SERVER_ROOTS.iter().map(|ta| {
        OwnedTrustAnchor::from_subject_spki_name_constraints(
            ta.subject,
            ta.spki,
            ta.name_constraints,
        )
    }));

    let mut cfg = ClientConfig::builder()
        .with_safe_defaults()
        .with_root_certificates(root_store)
        .with_no_client_auth();

    // 构造验证器，并将 expected_host 作为真实域名覆盖用于校验
    let verifier = build_cert_verifier(tls, Some(expected_host.to_string()));
    cfg.dangerous().set_certificate_verifier(verifier);
    cfg
}

// ===== Helpers: SPKI pin parsing & computing =====
pub fn validate_pins(pins: &[String]) -> Option<Vec<String>> {
    // 规则：Base64URL 无填充，长度=43，最多 10 个；非法或超限则返回 None（本次连接禁用 Pin）。
    if pins.is_empty() {
        return Some(Vec::new());
    }
    if pins.len() > 10 {
        return None;
    }
    let mut out: Vec<String> = Vec::new();
    for p in pins {
        let s = p.trim();
        if s.len() != 43 {
            return None;
        }
        // 尝试解码以校验合法性（但不使用解码结果）
        if base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(s)
            .is_err()
        {
            return None;
        }
        if !out.iter().any(|e| e == s) {
            out.push(s.to_string());
        }
    }
    Some(out)
}

fn log_spki_source(source: SpkiSource) -> &'static str {
    match source {
        SpkiSource::Exact => "exact",
        SpkiSource::WholeCertFallback => "fallback",
    }
}
