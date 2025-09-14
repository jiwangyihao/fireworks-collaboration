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
    // 若设置，则白名单匹配基于该主机名进行（用于伪 SNI 时按真实主机校验）
    override_host: Option<String>,
}

impl WhitelistCertVerifier {
    pub fn new(inner: Arc<dyn ServerCertVerifier>, whitelist: Vec<String>) -> Self {
        Self { inner, whitelist, override_host: None }
    }

    pub fn new_with_override(inner: Arc<dyn ServerCertVerifier>, whitelist: Vec<String>, override_host: Option<String>) -> Self {
        Self { inner, whitelist, override_host }
    }

    fn host_allowed_str(&self, host: &str) -> bool {
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

        // 再做域白名单判定（优先使用 override_host；否则使用 SNI 中的 DNS 名称）
        let host_to_check = if let Some(h) = &self.override_host {
            h.as_str()
        } else {
            match server_name { ServerName::DnsName(n) => n.as_ref(), _ => return Err(TlsError::General("non-dns server name".into())) }
        };
        if !self.host_allowed_str(host_to_check) {
            return Err(TlsError::General("SAN whitelist mismatch".into()));
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
        Self { whitelist, override_host }
    }
    fn host_allowed_str(&self, host: &str) -> bool {
        if self.whitelist.is_empty() { return false; }
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
            match server_name { ServerName::DnsName(n) => n.as_ref(), _ => return Err(TlsError::General("non-dns server name".into())) }
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
        OwnedTrustAnchor::from_subject_spki_name_constraints(ta.subject, ta.spki, ta.name_constraints)
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
            return Arc::new(WhitelistOnlyVerifier::new(tls.san_whitelist.clone(), override_host));
        }
        return Arc::new(InsecureCertVerifier);
    }
    let mut root_store = RootCertStore::empty();
    root_store.add_trust_anchors(webpki_roots::TLS_SERVER_ROOTS.iter().map(|ta| {
        OwnedTrustAnchor::from_subject_spki_name_constraints(ta.subject, ta.spki, ta.name_constraints)
    }));
    let inner = Arc::new(rustls::client::WebPkiVerifier::new(root_store, None));
    if tls.skip_san_whitelist {
        // 仅使用默认的链路与主机名校验
        return inner;
    }
    if let Some(h) = override_host {
        Arc::new(WhitelistCertVerifier::new_with_override(inner, tls.san_whitelist.clone(), Some(h)))
    } else {
        Arc::new(WhitelistCertVerifier::new(inner, tls.san_whitelist.clone()))
    }
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
        OwnedTrustAnchor::from_subject_spki_name_constraints(ta.subject, ta.spki, ta.name_constraints)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_host_allowed_logic() {
        let inner = Arc::new(rustls::client::WebPkiVerifier::new(RootCertStore::empty(), None));
        let v = WhitelistCertVerifier::new(inner, vec!["github.com".into(), "*.github.com".into()]);
        assert!(v.host_allowed_str("api.github.com"));
        assert!(!v.host_allowed_str("example.com"));
    }

    #[test]
    fn test_empty_whitelist_rejects_any() {
        let inner = Arc::new(rustls::client::WebPkiVerifier::new(RootCertStore::empty(), None));
        let v = WhitelistCertVerifier::new(inner, vec![]);
        assert!(!v.host_allowed_str("github.com"));
    }

    #[test]
    fn test_non_dns_server_name_rejected() {
        let inner = Arc::new(rustls::client::WebPkiVerifier::new(RootCertStore::empty(), None));
        let v = WhitelistCertVerifier::new(inner, vec!["github.com".into()]);
        // 非 DNS 名称时（例如 IP），若不提供 override_host，verify 阶段将返回错误；
        // 但 host_allowed_str 仍按字面匹配
        assert!(!v.host_allowed_str("127.0.0.1"));
        assert!(v.host_allowed_str("github.com"));
    }

    #[test]
    fn test_create_client_config() {
    let tls = TlsCfg { san_whitelist: vec!["github.com".into(), "*.github.com".into()], insecure_skip_verify: false, skip_san_whitelist: false };
        let _cfg = create_client_config(&tls);
    }

    #[test]
    fn test_create_client_config_insecure() {
    let tls = TlsCfg { san_whitelist: vec![], insecure_skip_verify: true, skip_san_whitelist: false };
        let _cfg = create_client_config(&tls);
    }
}
