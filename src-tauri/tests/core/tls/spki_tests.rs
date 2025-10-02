// 从 src/core/tls/spki.rs 迁移的测试
use fireworks_collaboration_lib::core::tls::spki::{
    compute_fingerprint_bundle, compute_spki_sha256_b64, SpkiSource,
};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use rcgen::generate_simple_self_signed;
use ring::digest::{digest, SHA256};
use rustls::Certificate;

#[test]
fn test_extract_spki_exact() {
    let cert = generate_simple_self_signed(vec!["example.com".into()]).unwrap();
    let der = cert.serialize_der().unwrap();
    let rustls_cert = Certificate(der.clone());

    let (spki, source) = compute_spki_sha256_b64(&rustls_cert);
    assert_eq!(source, SpkiSource::Exact);
    assert_eq!(spki.len(), 43);
}

#[test]
fn test_empty_cert_falls_back() {
    let cert = Certificate(Vec::new());
    let (spki, source) = compute_spki_sha256_b64(&cert);
    assert_eq!(source, SpkiSource::WholeCertFallback);
    assert_eq!(spki.len(), 43);
}

#[test]
fn test_fingerprint_bundle_contains_cert_hash() {
    let cert = generate_simple_self_signed(vec!["bundle.example".into()]).unwrap();
    let der = cert.serialize_der().unwrap();
    let rustls_cert = Certificate(der.clone());

    let bundle = compute_fingerprint_bundle(&rustls_cert);
    assert_eq!(bundle.spki_sha256.len(), 43);
    assert_eq!(bundle.cert_sha256.len(), 43);

    let expected_cert_sha = URL_SAFE_NO_PAD.encode(digest(&SHA256, &der).as_ref());
    assert_eq!(bundle.cert_sha256, expected_cert_sha);
    assert_eq!(bundle.spki_source, SpkiSource::Exact);
}
