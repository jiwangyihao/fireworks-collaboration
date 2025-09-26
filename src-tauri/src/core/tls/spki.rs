use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use ring::digest::{digest, SHA256};
use rustls::Certificate;
use thiserror::Error;
use x509_parser::prelude::*;

/// 标记 SPKI 指纹来源：是精确 ASN.1 提取还是退化为整张证书哈希。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpkiSource {
    Exact,
    WholeCertFallback,
}

/// 证书指纹组合（SPKI + 整证书）
#[derive(Debug, Clone)]
pub struct FingerprintBundle {
    pub spki_sha256: String,
    pub cert_sha256: String,
    pub spki_source: SpkiSource,
}

/// 精确提取 SPKI 失败时的错误类型。
#[derive(Debug, Error)]
pub enum SpkiError {
    #[error("certificate parse error: {0}")]
    Parse(String),
}

fn try_extract_spki_der<'a>(cert_der: &'a [u8]) -> Result<&'a [u8], SpkiError> {
    let (_, cert) =
        X509Certificate::from_der(cert_der).map_err(|e| SpkiError::Parse(e.to_string()))?;
    let spki = cert.tbs_certificate.subject_pki;
    // subject_pki.raw 包含完整 DER 片段
    Ok(spki.raw)
}

/// 计算 leaf 证书的 SPKI SHA256（Base64URL 无填充编码）。
pub fn compute_spki_sha256_b64(cert: &Certificate) -> (String, SpkiSource) {
    match try_extract_spki_der(&cert.0) {
        Ok(spki_der) => {
            let sha = digest(&SHA256, spki_der);
            (URL_SAFE_NO_PAD.encode(sha.as_ref()), SpkiSource::Exact)
        }
        Err(_) => {
            let sha = digest(&SHA256, &cert.0);
            (
                URL_SAFE_NO_PAD.encode(sha.as_ref()),
                SpkiSource::WholeCertFallback,
            )
        }
    }
}

/// 同时计算 SPKI 与整张证书的 SHA256 指纹。
pub fn compute_fingerprint_bundle(cert: &Certificate) -> FingerprintBundle {
    let (spki_sha256, spki_source) = compute_spki_sha256_b64(cert);
    let cert_sha = digest(&SHA256, &cert.0);
    let cert_sha256 = URL_SAFE_NO_PAD.encode(cert_sha.as_ref());
    FingerprintBundle {
        spki_sha256,
        cert_sha256,
        spki_source,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use rcgen::generate_simple_self_signed;
    use ring::digest::SHA256;

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
}
