use std::sync::Arc;

use super::util::match_domain;
use crate::core::config::model::TlsCfg;
use crate::core::tls::spki::{compute_spki_sha256_b64, SpkiSource};
use base64::Engine;
use rustls::client::{ServerCertVerified, ServerCertVerifier};
use rustls::ClientConfig;
use rustls::{Certificate, Error as TlsError, OwnedTrustAnchor, RootCertStore, ServerName};

/// 包装 rustls 默认验证器，并在其基础上增加 SAN 白名单域校验。
pub struct WhitelistCertVerifier {
    inner: Arc<dyn ServerCertVerifier>,
    whitelist: Vec<String>,
    // 若设置，则白名单匹配基于该主机名进行（用于伪 SNI 时按真实主机校验）
    override_host: Option<String>,
    // P3.3: Real-Host 验证开关（默认开启）。开启时：即使握手使用了 Fake SNI，链与域名验证也按真实域执行；
    // 关闭时：回退到按 SNI 执行默认验证的旧逻辑（override_host 仅用于白名单匹配）。
    real_host_verify_enabled: bool,
    // P3.4: SPKI Pin 强校验（Base64URL，无填充，长度=43）。非空时启用。
    spki_pins: Vec<String>,
}

impl WhitelistCertVerifier {
    pub fn new(inner: Arc<dyn ServerCertVerifier>, whitelist: Vec<String>) -> Self {
        Self {
            inner,
            whitelist,
            override_host: None,
            real_host_verify_enabled: true,
            spki_pins: Vec::new(),
        }
    }

    pub fn new_with_override(
        inner: Arc<dyn ServerCertVerifier>,
        whitelist: Vec<String>,
        override_host: Option<String>,
        real_host_verify_enabled: bool,
        spki_pins: Vec<String>,
    ) -> Self {
        Self {
            inner,
            whitelist,
            override_host,
            real_host_verify_enabled,
            spki_pins,
        }
    }

    fn host_allowed_str(&self, host: &str) -> bool {
        if self.whitelist.is_empty() {
            return false;
        }
        self.whitelist.iter().any(|p| match_domain(p, host))
    }
}

impl ServerCertVerifier for WhitelistCertVerifier {
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
                // 尝试以真实域名执行验证（即使握手使用了 Fake SNI）
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
                        // 无法构造期望名称时，回退到原始 server_name
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
            // 关闭开关：按旧逻辑使用 SNI 对应的 server_name
            self.inner.verify_server_cert(
                end_entity,
                intermediates,
                server_name,
                scts,
                ocsp_response,
                now,
            )?;
        }

        // 再做域白名单判定（优先使用 override_host；否则使用 SNI 中的 DNS 名称）
        let host_to_check = if let Some(h) = &self.override_host {
            h.as_str()
        } else {
            match server_name {
                ServerName::DnsName(n) => n.as_ref(),
                _ => return Err(TlsError::General("non-dns server name".into())),
            }
        };
        if !self.host_allowed_str(host_to_check) {
            return Err(TlsError::General("SAN whitelist mismatch".into()));
        }

        // P3.4: SPKI Pin 强校验（若配置非空）。在链与主机名验证成功、白名单通过后执行。
        if !self.spki_pins.is_empty() {
            // 仅当 pin 列表全部合法时才执行；否则视为禁用（记录一次调试日志）。
            if let Some(valid_pins) = validate_pins(&self.spki_pins) {
                let (spki_b64, spki_source) = compute_spki_sha256_b64(end_entity);
                let pin_count = valid_pins.len() as u8;
                if !valid_pins.iter().any(|p| p == &spki_b64) {
                    // 发送结构化事件并返回 Verify 类错误；不触发 Fake->Real 回退。
                    tracing::warn!(target="git.transport", host=%host_to_check, pin_enforced="on", pin_count=%pin_count, cert_spki=%spki_b64, spki_source=%log_spki_source(spki_source), "pin_mismatch");
                    use crate::events::structured::{
                        publish_global, Event as StructuredEvent,
                        StrategyEvent as StructuredStrategyEvent,
                    };
                    publish_global(StructuredEvent::Strategy(
                        StructuredStrategyEvent::CertFpPinMismatch {
                            id: host_to_check.to_string(),
                            host: host_to_check.to_string(),
                            spki_sha256: spki_b64.clone(),
                            pin_count,
                        },
                    ));
                    return Err(TlsError::General("cert_fp_pin_mismatch".into()));
                } else {
                    tracing::debug!(target="git.transport", host=%host_to_check, pin_enforced="on", pin_count=%pin_count, spki_source=%log_spki_source(spki_source), "pin_match");
                }
            } else {
                tracing::warn!(target="git.transport", host=%host_to_check, pin_enforced="off", reason="invalid_pins", "pin_disabled_this_conn");
            }
        }
        Ok(ServerCertVerified::assertion())
    }
}

/// 仅进行 SAN 白名单校验的验证器；不进行证书链与主机名的默认校验。
/// 用途：当用户选择“跳过默认证书验证，但仍希望保留自定义白名单校验”时。
pub struct WhitelistOnlyVerifier {
    whitelist: Vec<String>,
    override_host: Option<String>,
}

impl WhitelistOnlyVerifier {
    pub fn new(whitelist: Vec<String>, override_host: Option<String>) -> Self {
        Self {
            whitelist,
            override_host,
        }
    }
    fn host_allowed_str(&self, host: &str) -> bool {
        if self.whitelist.is_empty() {
            return false;
        }
        self.whitelist.iter().any(|p| match_domain(p, host))
    }
}

impl ServerCertVerifier for WhitelistOnlyVerifier {
    fn verify_server_cert(
        &self,
        _end_entity: &Certificate,
        _intermediates: &[Certificate],
        server_name: &ServerName,
        _scts: &mut dyn Iterator<Item = &[u8]>,
        _ocsp_response: &[u8],
        _now: std::time::SystemTime,
    ) -> Result<ServerCertVerified, TlsError> {
        // 仅做白名单域匹配
        let host_to_check = if let Some(h) = &self.override_host {
            h.as_str()
        } else {
            match server_name {
                ServerName::DnsName(n) => n.as_ref(),
                _ => return Err(TlsError::General("non-dns server name".into())),
            }
        };
        if !self.host_allowed_str(host_to_check) {
            return Err(TlsError::General("SAN whitelist mismatch".into()));
        }
        Ok(ServerCertVerified::assertion())
    }
}

/// 极不安全：完全跳过证书链与域名校验，仅用于原型阶段联调。
/// 当 `tls.insecure_skip_verify=true` 时启用。
pub struct InsecureCertVerifier;

impl ServerCertVerifier for InsecureCertVerifier {
    fn verify_server_cert(
        &self,
        _end_entity: &Certificate,
        _intermediates: &[Certificate],
        _server_name: &ServerName,
        _scts: &mut dyn Iterator<Item = &[u8]>,
        _ocsp_response: &[u8],
        _now: std::time::SystemTime,
    ) -> Result<ServerCertVerified, TlsError> {
        Ok(ServerCertVerified::assertion())
    }
}

/// 使用系统根证书构造 WhitelistCertVerifier 的便利函数
pub fn make_whitelist_verifier(tls: &TlsCfg) -> Arc<dyn ServerCertVerifier> {
    let mut root_store = RootCertStore::empty();
    root_store.add_trust_anchors(webpki_roots::TLS_SERVER_ROOTS.iter().map(|ta| {
        OwnedTrustAnchor::from_subject_spki_name_constraints(
            ta.subject,
            ta.spki,
            ta.name_constraints,
        )
    }));
    let inner = Arc::new(rustls::client::WebPkiVerifier::new(root_store, None));
    Arc::new(WhitelistCertVerifier::new(inner, tls.san_whitelist.clone()))
}

/// 根据 TLS 配置构造合适的证书验证器：
/// - insecure_skip_verify=true: 返回完全跳过验证的 InsecureCertVerifier
/// - skip_san_whitelist=true: 返回仅做默认链与主机名验证的 WebPkiVerifier
/// - 否则：返回默认链验证+SAN 白名单增强的 WhitelistCertVerifier
fn build_cert_verifier(tls: &TlsCfg, override_host: Option<String>) -> Arc<dyn ServerCertVerifier> {
    if tls.insecure_skip_verify {
        // 若用户仍希望保留白名单校验，则仅执行白名单匹配；否则完全跳过
        if !tls.skip_san_whitelist {
            return Arc::new(WhitelistOnlyVerifier::new(
                tls.san_whitelist.clone(),
                override_host,
            ));
        }
        return Arc::new(InsecureCertVerifier);
    }
    let mut root_store = RootCertStore::empty();
    root_store.add_trust_anchors(webpki_roots::TLS_SERVER_ROOTS.iter().map(|ta| {
        OwnedTrustAnchor::from_subject_spki_name_constraints(
            ta.subject,
            ta.spki,
            ta.name_constraints,
        )
    }));
    let inner = Arc::new(rustls::client::WebPkiVerifier::new(root_store, None));
    if tls.skip_san_whitelist {
        // 仅使用默认的链路与主机名校验
        return inner;
    }
    if let Some(h) = override_host {
        Arc::new(WhitelistCertVerifier::new_with_override(
            inner,
            tls.san_whitelist.clone(),
            Some(h),
            tls.real_host_verify_enabled,
            tls.spki_pins.clone(),
        ))
    } else {
        // 当未提供 override_host 时，real_host_verify_enabled 与旧逻辑等价（按 SNI 验证），此处保持默认开启值。
        let mut v = WhitelistCertVerifier::new(inner, tls.san_whitelist.clone());
        v.spki_pins = tls.spki_pins.clone();
        Arc::new(v)
    }
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

    // 构造验证器，若未跳过 SAN 白名单，将以 expected_host 作为白名单匹配的 override 主机名
    let verifier = build_cert_verifier(tls, Some(expected_host.to_string()));
    cfg.dangerous().set_certificate_verifier(verifier);
    cfg
}

// ===== Helpers: SPKI pin parsing & computing =====
fn validate_pins(pins: &[String]) -> Option<Vec<String>> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
    use rcgen::generate_simple_self_signed;
    use rustls::client::{ServerCertVerified, ServerCertVerifier};
    use rustls::{Certificate, ServerName};

    #[test]
    fn test_host_allowed_logic() {
        let inner = Arc::new(rustls::client::WebPkiVerifier::new(
            RootCertStore::empty(),
            None,
        ));
        let v = WhitelistCertVerifier::new(inner, vec!["github.com".into(), "*.github.com".into()]);
        assert!(v.host_allowed_str("api.github.com"));
        assert!(!v.host_allowed_str("example.com"));
    }

    #[test]
    fn test_empty_whitelist_rejects_any() {
        let inner = Arc::new(rustls::client::WebPkiVerifier::new(
            RootCertStore::empty(),
            None,
        ));
        let v = WhitelistCertVerifier::new(inner, vec![]);
        assert!(!v.host_allowed_str("github.com"));
    }

    #[test]
    fn test_non_dns_server_name_rejected() {
        let inner = Arc::new(rustls::client::WebPkiVerifier::new(
            RootCertStore::empty(),
            None,
        ));
        let v = WhitelistCertVerifier::new(inner, vec!["github.com".into()]);
        // 非 DNS 名称时（例如 IP），若不提供 override_host，verify 阶段将返回错误；
        // 但 host_allowed_str 仍按字面匹配
        assert!(!v.host_allowed_str("127.0.0.1"));
        assert!(v.host_allowed_str("github.com"));
    }

    #[test]
    fn test_create_client_config() {
        let tls = TlsCfg {
            san_whitelist: vec!["github.com".into(), "*.github.com".into()],
            insecure_skip_verify: false,
            skip_san_whitelist: false,
            spki_pins: Vec::new(),
            real_host_verify_enabled: true,
            metrics_enabled: true,
            cert_fp_log_enabled: true,
            cert_fp_max_bytes: 1024 * 1024,
        };
        let _cfg = create_client_config(&tls);
    }

    #[test]
    fn test_create_client_config_insecure() {
        let tls = TlsCfg {
            san_whitelist: vec![],
            insecure_skip_verify: true,
            skip_san_whitelist: false,
            spki_pins: Vec::new(),
            real_host_verify_enabled: true,
            metrics_enabled: true,
            cert_fp_log_enabled: true,
            cert_fp_max_bytes: 1024 * 1024,
        };
        let _cfg = create_client_config(&tls);
    }

    struct AlwaysOkVerifier;
    impl ServerCertVerifier for AlwaysOkVerifier {
        fn verify_server_cert(
            &self,
            _end_entity: &Certificate,
            _intermediates: &[Certificate],
            _server_name: &ServerName,
            _scts: &mut dyn Iterator<Item = &[u8]>,
            _ocsp_response: &[u8],
            _now: std::time::SystemTime,
        ) -> Result<ServerCertVerified, TlsError> {
            Ok(ServerCertVerified::assertion())
        }
    }

    #[test]
    fn test_pin_mismatch_returns_verify_error() {
        // Construct verifier with a valid-looking pin (base64url of 32 zero bytes) that won't match the empty-cert hash
        let pins = vec![URL_SAFE_NO_PAD.encode([0u8; 32])];
        let v = WhitelistCertVerifier {
            inner: Arc::new(AlwaysOkVerifier),
            whitelist: vec!["example.com".into()],
            override_host: Some("example.com".into()),
            real_host_verify_enabled: true,
            spki_pins: pins,
        };
        let ee = Certificate(vec![]); // empty cert -> hash won't be all 'A'
        let mut scts = std::iter::empty::<&[u8]>();
        let err = v
            .verify_server_cert(
                &ee,
                &[],
                &ServerName::try_from("fake.sni.com").unwrap(),
                &mut scts,
                &[],
                std::time::SystemTime::now(),
            )
            .unwrap_err();
        let msg = format!("{}", err);
        assert!(msg.to_ascii_lowercase().contains("pin"));
    }

    #[test]
    fn test_pin_match_allows_connection() {
        let cert = generate_simple_self_signed(vec!["pin.example".into()]).unwrap();
        let der = cert.serialize_der().unwrap();
        let rustls_cert = Certificate(der.clone());
        let (pin, source) = compute_spki_sha256_b64(&rustls_cert);
        assert_eq!(source, SpkiSource::Exact);

        let pins = vec![pin.clone()];
        let v = WhitelistCertVerifier {
            inner: Arc::new(AlwaysOkVerifier),
            whitelist: vec!["pin.example".into(), "*.example".into()],
            override_host: Some("pin.example".into()),
            real_host_verify_enabled: true,
            spki_pins: pins,
        };
        let mut scts = std::iter::empty::<&[u8]>();
        let result = v.verify_server_cert(
            &rustls_cert,
            &[],
            &ServerName::try_from("fake.sni.example").unwrap(),
            &mut scts,
            &[],
            std::time::SystemTime::now(),
        );
        assert!(result.is_ok());
    }

    struct CaptureVerifier(std::sync::Mutex<Option<String>>);
    impl ServerCertVerifier for CaptureVerifier {
        fn verify_server_cert(
            &self,
            _end_entity: &Certificate,
            _intermediates: &[Certificate],
            server_name: &ServerName,
            _scts: &mut dyn Iterator<Item = &[u8]>,
            _ocsp_response: &[u8],
            _now: std::time::SystemTime,
        ) -> Result<ServerCertVerified, TlsError> {
            let name = match server_name {
                ServerName::DnsName(n) => n.as_ref().to_string(),
                _ => "non-dns".to_string(),
            };
            *self.0.lock().unwrap() = Some(name);
            Ok(ServerCertVerified::assertion())
        }
    }

    #[test]
    fn test_real_host_verify_uses_override_when_enabled() {
        let captured = Arc::new(CaptureVerifier(std::sync::Mutex::new(None)));
        let whitelist = vec!["example.com".into()];
        let v = WhitelistCertVerifier {
            inner: captured.clone(),
            whitelist,
            override_host: Some("real.example.com".into()),
            real_host_verify_enabled: true,
            spki_pins: Vec::new(),
        };
        let ee = Certificate(vec![]);
        let mut scts = std::iter::empty::<&[u8]>();
        let _ = v.verify_server_cert(
            &ee,
            &[],
            &ServerName::try_from("fake.sni.com").unwrap(),
            &mut scts,
            &[],
            std::time::SystemTime::now(),
        );
        let got = captured.0.lock().unwrap().clone().unwrap();
        assert_eq!(got, "real.example.com");
    }

    #[test]
    fn test_real_host_verify_disabled_uses_sni() {
        let captured = Arc::new(CaptureVerifier(std::sync::Mutex::new(None)));
        let whitelist = vec!["example.com".into()];
        let v = WhitelistCertVerifier {
            inner: captured.clone(),
            whitelist,
            override_host: Some("real.example.com".into()),
            real_host_verify_enabled: false,
            spki_pins: Vec::new(),
        };
        let ee = Certificate(vec![]);
        let mut scts = std::iter::empty::<&[u8]>();
        let _ = v.verify_server_cert(
            &ee,
            &[],
            &ServerName::try_from("fake.sni.com").unwrap(),
            &mut scts,
            &[],
            std::time::SystemTime::now(),
        );
        let got = captured.0.lock().unwrap().clone().unwrap();
        assert_eq!(got, "fake.sni.com");
    }

    #[test]
    fn test_validate_pins_rules() {
        // valid length 43 and url-safe characters (encode 32 zero bytes)
        let valid = vec![URL_SAFE_NO_PAD.encode([0u8; 32])];
        let out = validate_pins(&valid).unwrap();
        assert_eq!(out.len(), 1);
        // duplicates are deduplicated but still considered valid
        let dup = vec![valid[0].clone(), valid[0].clone()];
        let out_dup = validate_pins(&dup).unwrap();
        assert_eq!(out_dup.len(), 1);
        // invalid length
        assert!(validate_pins(&vec!["short".into()]).is_none());
        // too many
        let many: Vec<String> = (0..11)
            .map(|_| "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA".to_string())
            .collect();
        assert!(validate_pins(&many).is_none());
    }
}
