use std::{collections::HashMap, sync::Arc, time::Instant};

use anyhow::{anyhow, Context, Result};
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use hyper::body::HttpBody as _;
use hyper::header::{HeaderMap, HeaderValue, HOST};
use hyper::{Body, Request, Uri, Version};
use rustls::ServerName;
use tokio::net::TcpStream;
use tokio::time::{timeout, Duration};
use tokio_rustls::rustls::ClientConfig;
use tokio_rustls::TlsConnector;

use crate::core::config::model::AppConfig;
use crate::core::ip_pool::global::pick_best_async;
use crate::core::ip_pool::global::report_outcome_async;
use crate::core::ip_pool::IpOutcome;
// Metrics / events instrumentation
// use crate::events::structured::{
//    publish_global, Event as StructuredEvent, StrategyEvent as StructuredStrategyEvent,
// };
// Reuse existing metrics enabled flag
// use crate::core::git::transport::metrics::metrics_enabled;
use crate::core::tls::util::{decide_sni_host_with_proxy, proxy_present};
use crate::core::tls::verifier::{create_client_config, create_client_config_with_expected_name};

use super::types::{HttpRequestInput, HttpResponseOutput, TimingInfo};

/// 内部简单 HTTP 客户端：使用手动连接 + hyper `client::conn，便于自定义` SNI
pub struct HttpClient {
    cfg: AppConfig,
    tls_default: Arc<ClientConfig>,
}

impl HttpClient {
    pub fn new(cfg: AppConfig) -> Self {
        let tls_cfg = Arc::new(create_client_config(&cfg.tls));
        Self {
            cfg,
            tls_default: tls_cfg,
        }
    }

    /// 计算用于 TLS 握手的 SNI 主机名，并返回是否使用了伪 SNI
    pub fn compute_sni_host(&self, force_real_sni: bool, real_host: &str) -> (String, bool) {
        let proxy = proxy_present();
        decide_sni_host_with_proxy(&self.cfg, force_real_sni, real_host, proxy)
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
        if url.scheme_str() != Some("https") {
            return Err(anyhow!("only https is supported in P0.4"));
        }

        // 先解码 body（若无效可在未触网前失败）
        let body_bytes: Vec<u8> = if let Some(b64) = &input.body_base64 {
            BASE64.decode(b64).context("decode bodyBase64")?
        } else {
            Vec::new()
        };

        let host = url
            .host()
            .ok_or_else(|| anyhow!("url host missing"))?
            .to_string();
        let port = url.port_u16().unwrap_or(443);

        // 集成全局IP池，优先用 pick_best 选出的 IP 建立连接。
        let start_total = Instant::now();
        let sel = pick_best_async(&host, port).await;

        let mut errors: Vec<String> = Vec::new();
        let mut success_out: Option<HttpResponseOutput> = None;
        let mut used_ip_pool = false;
        let mut used_candidate_stat: Option<crate::core::ip_pool::IpStat> = None;

        // Candidate targets: from IP pool, then try System if pool fails or is empty
        let candidates: Vec<crate::core::ip_pool::IpStat> = sel.iter_candidates().cloned().collect();
        enum Target {
            Candidate(crate::core::ip_pool::IpStat),
            System,
        }
        let mut targets: Vec<Target> = candidates
            .iter()
            .map(|c| Target::Candidate(c.clone()))
            .collect();
        
        // If system strategy or fallback needed, append System
        // Note: subtransport tries system after candidates. We do same.
        targets.push(Target::System);

        for target in targets {
            // Check if we should skip system if we are strictly cached?
            // subtransport falls back to system even if Cached strategy, IF candidates fail.
            // But if pick_best returns SystemDefault, candidates is empty.
            if let Target::System = target {
                 if !candidates.is_empty() && success_out.is_some() {
                     break; // Should not happen if success, but logical check
                 }
            }

            let (connect_addr, is_pool_candidate, current_stat) = match &target {
                Target::Candidate(stat) => {
                     (stat.candidate.address.to_string(), true, Some(stat.clone()))
                }
                Target::System => {
                    (host.clone(), false, None)
                }
            };

            let start_connect = Instant::now();
            let tcp_res = timeout(
                Duration::from_millis(input.timeout_ms),
                TcpStream::connect((connect_addr.as_str(), port)),
            ).await;

            let tcp = match tcp_res {
                Ok(Ok(stream)) => stream,
                Ok(Err(e)) => {
                    errors.push(format!("connect error to {}: {}", connect_addr, e));
                    if is_pool_candidate {
                        if let Some(c) = &current_stat {
                           report_outcome_async(crate::core::ip_pool::IpSelection::from_cached(&host, port, c.clone()), IpOutcome::Failure);
                        }
                    }
                    continue;
                }
                Err(_) => {
                    errors.push(format!("connect timeout to {}", connect_addr));
                    if is_pool_candidate {
                         if let Some(c) = &current_stat {
                           report_outcome_async(crate::core::ip_pool::IpSelection::from_cached(&host, port, c.clone()), IpOutcome::Failure);
                        }
                    }
                    continue;
                }
            };
            let connect_ms = start_connect.elapsed().as_millis() as u32;

            // TLS Handshake
            let (sni_host_final, fake) = self.compute_sni_host(input.force_real_sni, &host); // Use host for SNI calculation
            let server_name = match ServerName::try_from(sni_host_final.as_str()) {
                Ok(sn) => sn,
                Err(e) => {
                    errors.push(format!("invalid sni {}: {}", sni_host_final, e));
                    continue;
                }
            };

            let start_tls = Instant::now();
            let tls_config: Arc<ClientConfig> = if fake {
                Arc::new(create_client_config_with_expected_name(
                    &self.cfg.tls,
                    &host,
                ))
            } else {
                self.tls_default.clone()
            };
            let tls = TlsConnector::from(tls_config);
            let stream = match tls.connect(server_name, tcp).await {
                Ok(s) => s,
                Err(e) => {
                     errors.push(format!("tls handshake error with {}: {}", connect_addr, e));
                     if is_pool_candidate {
                        if let Some(c) = &current_stat {
                           report_outcome_async(crate::core::ip_pool::IpSelection::from_cached(&host, port, c.clone()), IpOutcome::Failure);
                        }
                     }
                     continue;
                }
            };
            let tls_ms = start_tls.elapsed().as_millis() as u32;

             // HTTP Handshake
            let (mut sender, conn) = match hyper::client::conn::handshake(stream).await {
                Ok(res) => res,
                Err(e) => {
                     errors.push(format!("http handshake error with {}: {}", connect_addr, e));
                     // Handshake failure is also a connection/protocol failure
                     if is_pool_candidate {
                         if let Some(c) = &current_stat {
                           report_outcome_async(crate::core::ip_pool::IpSelection::from_cached(&host, port, c.clone()), IpOutcome::Failure);
                        }
                     }
                     continue;
                }
            };

            tokio::spawn(async move {
                if let Err(e) = conn.await {
                    tracing::debug!(target = "http", "conn ended: {:?}", e);
                }
            });

            // Send Request
            let mut req_builder = Request::builder()
                .method(input.method.as_str())
                .uri(url.clone()) // Re-use parsed URL
                .version(Version::HTTP_11);
            
            let headers_map = req_builder.headers_mut().expect("headers");
            for (k, v) in &input.headers {
                if let Ok(name) = hyper::header::HeaderName::try_from(k) {
                    if let Ok(val) = hyper::header::HeaderValue::try_from(v) {
                        headers_map.insert(name, val);
                    }
                }
            }
            self.upsert_host_header(headers_map, &host);

            let req = req_builder
                .body(Body::from(body_bytes.clone()))
                .expect("request body");

            let start_first = Instant::now();
            let resp_res = sender.send_request(req).await;
            
            match resp_res {
                Ok(mut resp) => {
                    let first_byte_ms = start_first.elapsed().as_millis() as u32;
                    let status = resp.status().as_u16();
                    let mut headers: HashMap<String, String> = HashMap::new();
                    for (k, v) in resp.headers().iter() {
                         if let Ok(vs) = v.to_str() {
                             headers.insert(k.to_string(), vs.to_string());
                         }
                    }

                    let mut buf: Vec<u8> = Vec::new();
                    let mut read_failed = false;
                    while let Some(next) = resp.body_mut().data().await {
                        match next {
                            Ok(chunk) => buf.extend_from_slice(&chunk),
                            Err(e) => {
                                errors.push(format!("read body error from {}: {}", connect_addr, e));
                                read_failed = true;
                                break;
                            }
                        }
                    }
                    if read_failed {
                         if is_pool_candidate {
                             if let Some(c) = &current_stat {
                               report_outcome_async(crate::core::ip_pool::IpSelection::from_cached(&host, port, c.clone()), IpOutcome::Failure);
                             }
                         }
                        continue;
                    }

                    // Success!
                    if is_pool_candidate {
                         if let Some(c) = &current_stat {
                           report_outcome_async(crate::core::ip_pool::IpSelection::from_cached(&host, port, c.clone()), IpOutcome::Success);
                         }
                    }
                    
                    let total_ms = start_total.elapsed().as_millis() as u32;
                    let body_size = buf.len();
                     if self.should_warn_large_body(body_size) {
                        tracing::warn!(target = "http", size = body_size, "large body warning");
                    }

                    success_out = Some(HttpResponseOutput {
                        ok: (200..300).contains(&status),
                        status,
                        headers,
                        body_base64: BASE64.encode(&buf),
                        used_fake_sni: fake,
                        ip: Some(connect_addr), // This might be hostname for system, or IP for candidate
                        timing: TimingInfo {
                            connect_ms,
                            tls_ms,
                            first_byte_ms,
                            total_ms,
                        },
                        redirects: vec![],
                        body_size,
                    });
                    used_ip_pool = is_pool_candidate;
                    used_candidate_stat = current_stat;
                    break;
                }
                Err(e) => {
                     errors.push(format!("send request error to {}: {}", connect_addr, e));
                     if is_pool_candidate {
                         if let Some(c) = &current_stat {
                           report_outcome_async(crate::core::ip_pool::IpSelection::from_cached(&host, port, c.clone()), IpOutcome::Failure);
                         }
                     }
                     continue;
                }
            }
        }

        let out = match success_out {
            Some(o) => o,
            None => return Err(anyhow!("All connection attempts failed: {:?}", errors)),
        };


        Ok(out)
    }
}
