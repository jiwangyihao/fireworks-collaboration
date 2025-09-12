use std::{collections::HashMap, sync::Arc, time::Instant};

use anyhow::{anyhow, Context, Result};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use hyper::{Body, Request, Response, Uri, Version};
use hyper::body::HttpBody as _;
use hyper::header::{HeaderMap, HeaderValue, HOST};
use rustls::ServerName;
use tokio::net::TcpStream;
use tokio::time::{timeout, Duration};
use tokio_rustls::TlsConnector;
use tokio_rustls::rustls::ClientConfig;

use crate::core::config::model::AppConfig;
use crate::core::tls::util::should_use_fake;
use crate::core::tls::verifier::create_client_config;

use super::types::{HttpRequestInput, HttpResponseOutput, TimingInfo};

/// 内部简单 HTTP 客户端：使用手动连接 + hyper client::conn，便于自定义 SNI
pub struct HttpClient {
  cfg: AppConfig,
  tls: Arc<ClientConfig>,
}

impl HttpClient {
  pub fn new(cfg: AppConfig) -> Self {
    let tls_cfg = Arc::new(create_client_config(&cfg.tls));
    Self { cfg, tls: tls_cfg }
  }

  /// 计算用于 TLS 握手的 SNI 主机名，并返回是否使用了伪 SNI
  pub fn compute_sni_host(&self, force_real_sni: bool, real_host: &str) -> (String, bool) {
    let fake = should_use_fake(&self.cfg, force_real_sni);
    if fake { (self.cfg.http.fake_sni_host.clone(), true) } else { (real_host.to_string(), false) }
  }

  /// Host 头写入或覆盖为真实域
  pub fn upsert_host_header(&self, headers: &mut HeaderMap, real_host: &str) {
    let val = HeaderValue::from_str(real_host).unwrap_or_else(|_| HeaderValue::from_static(""));
    headers.insert(HOST, val);
  }

  /// 是否需要大响应警告
  pub fn should_warn_large_body(&self, body_size: usize) -> bool {
    (body_size as u64) > self.cfg.http.large_body_warn_bytes
  }

  /// 发送单个 HTTPS 请求，支持覆盖 SNI（伪 SNI）与时序统计
  pub async fn send(&self, input: HttpRequestInput) -> Result<HttpResponseOutput> {
    let url: Uri = input.url.parse::<Uri>().context("invalid URL")?;
    if url.scheme_str() != Some("https") { return Err(anyhow!("only https is supported in P0.4")); }

    // 先解码 body（若无效可在未触网前失败）
    let body_bytes: Vec<u8> = if let Some(b64) = &input.body_base64 { BASE64.decode(b64).context("decode bodyBase64")? } else { Vec::new() };

    let host = url.host().ok_or_else(|| anyhow!("url host missing"))?.to_string();
    let port = url.port_u16().unwrap_or(443);

    // 连接 TCP
    let start_total = Instant::now();
    let start_connect = Instant::now();
    let tcp = timeout(Duration::from_millis(input.timeout_ms), TcpStream::connect((host.as_str(), port)))
      .await.context("connect timeout")?
      .context("connect error")?;
    let connect_ms = start_connect.elapsed().as_millis() as u32;

  // TLS 握手，可能使用伪 SNI
  let (sni_host, fake) = self.compute_sni_host(input.force_real_sni, &host);
  let server_name = ServerName::try_from(sni_host.as_str()).map_err(|_| anyhow!("invalid dns name for sni"))?;
    let start_tls = Instant::now();
    let tls = TlsConnector::from(self.tls.clone());
    let stream = tls.connect(server_name, tcp).await.context("tls handshake")?;
    let tls_ms = start_tls.elapsed().as_millis() as u32;

    // 使用 hyper client::conn 手动发送请求
    let (mut sender, conn) = hyper::client::conn::handshake(stream)
      .await.context("http handshake")?;
    // 后台驱动连接
    tokio::spawn(async move {
      if let Err(e) = conn.await { tracing::debug!(target="http", "conn ended: {:?}", e); }
    });

    // 构造请求，Host 头保持真实域
    let mut req_builder = Request::builder()
      .method(input.method.as_str())
      .uri(url)
      .version(Version::HTTP_11);

    // 设置头
    let headers_map = req_builder.headers_mut().expect("headers");
    for (k, v) in &input.headers {
      if let Ok(name) = hyper::header::HeaderName::try_from(k) {
        if let Ok(val) = hyper::header::HeaderValue::try_from(v) { headers_map.insert(name, val); }
      }
    }
    // 覆盖/写入 Host 头为真实域
    self.upsert_host_header(headers_map, &host);

    let req = req_builder.body(Body::from(body_bytes)).expect("request body");

    let start_first = Instant::now();
  let mut resp: Response<Body> = sender.send_request(req).await.context("send request")?;
    let first_byte_ms = start_first.elapsed().as_millis() as u32;

    // 收集响应头
    let status = resp.status().as_u16();
    let mut headers: HashMap<String, String> = HashMap::new();
    for (k, v) in resp.headers().iter() {
      if let Ok(vs) = v.to_str() { headers.insert(k.to_string(), vs.to_string()); }
    }

    // 读取完整响应体
    let mut buf: Vec<u8> = Vec::new();
    while let Some(next) = resp.body_mut().data().await { let chunk = next.context("read body")?; buf.extend_from_slice(&chunk); }
    let total_ms = start_total.elapsed().as_millis() as u32;

    let body_size = buf.len();
    if self.should_warn_large_body(body_size) {
      tracing::warn!(target="http", size=body_size, "large body warning");
    }

    // 目前不做重定向跟随（由 P0.5 统一处理），返回基础数据
    let out = HttpResponseOutput {
      ok: (200..300).contains(&status),
      status,
      headers,
      body_base64: BASE64.encode(&buf),
      used_fake_sni: fake,
      ip: None,
      timing: TimingInfo { connect_ms, tls_ms, first_byte_ms, total_ms },
      redirects: vec![],
      body_size,
    };
    Ok(out)
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::core::config::model::AppConfig;

  #[tokio::test]
  async fn test_reject_non_https() {
    let client = HttpClient::new(AppConfig::default());
    let input = HttpRequestInput{
      url: "http://example.com/".into(),
      method: "GET".into(),
      headers: HashMap::new(),
      body_base64: None,
      timeout_ms: 100,
      force_real_sni: false,
      follow_redirects: false,
      max_redirects: 0,
    };
    let err = client.send(input).await.err().expect("should fail");
    let msg = format!("{}", err);
    assert!(msg.contains("only https"));
  }

  #[tokio::test]
  async fn test_invalid_base64_early() {
    let client = HttpClient::new(AppConfig::default());
    let input = HttpRequestInput{
      url: "https://example.com/".into(),
      method: "POST".into(),
      headers: HashMap::new(),
      body_base64: Some("***not-base64***".into()),
      timeout_ms: 100,
      force_real_sni: false,
      follow_redirects: false,
      max_redirects: 0,
    };
    let err = client.send(input).await.err().expect("should fail");
    let msg = format!("{}", err);
    assert!(msg.contains("decode bodyBase64"));
  }

  #[test]
  fn test_compute_sni_host_fake_and_real() {
    let mut cfg = AppConfig::default();
    cfg.http.fake_sni_enabled = true;
    cfg.http.fake_sni_host = "baidu.com".into();
    let client = HttpClient::new(cfg.clone());
    let (sni, used_fake) = client.compute_sni_host(false, "github.com");
    assert_eq!(sni, "baidu.com");
    assert!(used_fake);
    let (sni2, used_fake2) = client.compute_sni_host(true, "github.com");
    assert_eq!(sni2, "github.com");
    assert!(!used_fake2);
  }

  #[test]
  fn test_upsert_host_header_overrides() {
    let client = HttpClient::new(AppConfig::default());
    let mut h = HeaderMap::new();
    client.upsert_host_header(&mut h, "example.com");
    assert_eq!(h.get(HOST).unwrap(), "example.com");
    // override
    client.upsert_host_header(&mut h, "another.test");
    assert_eq!(h.get(HOST).unwrap(), "another.test");
  }

  #[test]
  fn test_should_warn_large_body_boundary() {
    let mut cfg = AppConfig::default();
    cfg.http.large_body_warn_bytes = 10;
    let client = HttpClient::new(cfg);
    assert!(!client.should_warn_large_body(10)); // equal -> no warn
    assert!(client.should_warn_large_body(11));  // greater -> warn
  }
}
