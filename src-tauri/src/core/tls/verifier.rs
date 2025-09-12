use std::sync::Arc;

use rustls::client::{ServerCertVerified, ServerCertVerifier};
use rustls::{Certificate, Error as TlsError, ServerName, RootCertStore, OwnedTrustAnchor};
use rustls::ClientConfig;

use crate::core::config::model::TlsCfg;
use super::util::match_domain;

/// 包装 rustls 默认验证器，并在其基础上增加 SAN 白名单域校验。
pub struct WhitelistCertVerifier {
    inner: Arc<dyn ServerCertVerifier>,
    whitelist: Vec<String>,
}

impl WhitelistCertVerifier {
    pub fn new(inner: Arc<dyn ServerCertVerifier>, whitelist: Vec<String>) -> Self {
        Self { inner, whitelist }
    }

    fn host_allowed(&self, server_name: &ServerName) -> bool {
        let host = match server_name {
            ServerName::DnsName(n) => n.as_ref(),
            _ => return false,
        };
        // 空白名单直接拒绝（更安全）
        if self.whitelist.is_empty() { return false; }
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
        // 先使用系统默认验证器验证链路
        self.inner.verify_server_cert(end_entity, intermediates, server_name, scts, ocsp_response, now)?;

        // 再做域白名单判定（基于 SNI）
        if !self.host_allowed(server_name) {
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
        OwnedTrustAnchor::from_subject_spki_name_constraints(ta.subject, ta.spki, ta.name_constraints)
    }));
    let inner = Arc::new(rustls::client::WebPkiVerifier::new(root_store, None));
    Arc::new(WhitelistCertVerifier::new(inner, tls.san_whitelist.clone()))
}

/// 基于白名单验证器创建 rustls ClientConfig（无客户端证书）
pub fn create_client_config(tls: &TlsCfg) -> ClientConfig {
    // Root store 与 WebPkiVerifier 一致
    let mut root_store = RootCertStore::empty();
    root_store.add_trust_anchors(webpki_roots::TLS_SERVER_ROOTS.iter().map(|ta| {
        OwnedTrustAnchor::from_subject_spki_name_constraints(ta.subject, ta.spki, ta.name_constraints)
    }));

    let mut cfg = ClientConfig::builder()
        .with_safe_defaults()
        .with_root_certificates(root_store)
        .with_no_client_auth();

    if tls.insecure_skip_verify {
        // 直接跳过证书校验（仅原型测试，务必默认关闭）
        cfg.dangerous().set_certificate_verifier(Arc::new(InsecureCertVerifier));
    } else {
        let verifier = make_whitelist_verifier(tls);
        cfg.dangerous().set_certificate_verifier(verifier);
    }
    cfg
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_host_allowed_logic() {
    let inner = Arc::new(rustls::client::WebPkiVerifier::new(RootCertStore::empty(), None));
        let v = WhitelistCertVerifier::new(inner, vec!["github.com".into(), "*.github.com".into()]);
        let sni = ServerName::try_from("api.github.com").unwrap();
        assert!(v.host_allowed(&sni));
        let sni = ServerName::try_from("example.com").unwrap();
        assert!(!v.host_allowed(&sni));
    }

    #[test]
    fn test_empty_whitelist_rejects_any() {
        let inner = Arc::new(rustls::client::WebPkiVerifier::new(RootCertStore::empty(), None));
        let v = WhitelistCertVerifier::new(inner, vec![]);
        let sni = ServerName::try_from("github.com").unwrap();
        assert!(!v.host_allowed(&sni));
    }

    #[test]
    fn test_non_dns_server_name_rejected() {
        let inner = Arc::new(rustls::client::WebPkiVerifier::new(RootCertStore::empty(), None));
        let v = WhitelistCertVerifier::new(inner, vec!["github.com".into()]);
        // 构造一个非 DNS 的 ServerName：IP 会被解析为 IpAddress 变体
        let bad = ServerName::try_from("127.0.0.1").unwrap();
        assert!(!v.host_allowed(&bad));
        // 正向用例以使用 v，避免未使用告警
        let ok = ServerName::try_from("github.com").unwrap();
        assert!(v.host_allowed(&ok));
    }

    #[test]
    fn test_create_client_config() {
        let tls = TlsCfg { san_whitelist: vec!["github.com".into(), "*.github.com".into()], insecure_skip_verify: false };
        let _cfg = create_client_config(&tls);
    }

    #[test]
    fn test_create_client_config_insecure() {
        let tls = TlsCfg { san_whitelist: vec![], insecure_skip_verify: true };
        let _cfg = create_client_config(&tls);
    }
}
