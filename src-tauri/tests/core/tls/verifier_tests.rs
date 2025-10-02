// 从 src/core/tls/verifier.rs 迁移的测试
use fireworks_collaboration_lib::core::{
    config::model::TlsCfg,
    tls::{
        spki::{compute_spki_sha256_b64, SpkiSource},
        verifier::{create_client_config, validate_pins, WhitelistCertVerifier},
    },
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use rcgen::generate_simple_self_signed;
use rustls::{
    client::{ServerCertVerified, ServerCertVerifier},
    Certificate, RootCertStore, ServerName,
};
use rustls::Error as TlsError;
use std::sync::Arc;

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
    let pins = vec![URL_SAFE_NO_PAD.encode([0u8; 32])];
    let v = WhitelistCertVerifier {
        inner: Arc::new(AlwaysOkVerifier),
        whitelist: vec!["example.com".into()],
        override_host: Some("example.com".into()),
        real_host_verify_enabled: true,
        spki_pins: pins,
    };
    let ee = Certificate(vec![]);
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
    let valid = vec![URL_SAFE_NO_PAD.encode([0u8; 32])];
    let out = validate_pins(&valid).unwrap();
    assert_eq!(out.len(), 1);
    let dup = vec![valid[0].clone(), valid[0].clone()];
    let out_dup = validate_pins(&dup).unwrap();
    assert_eq!(out_dup.len(), 1);
    assert!(validate_pins(&vec!["short".into()]).is_none());
    let many: Vec<String> = (0..11)
        .map(|_| "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA".to_string())
        .collect();
    assert!(validate_pins(&many).is_none());
}
