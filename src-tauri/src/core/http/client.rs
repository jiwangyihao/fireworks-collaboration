use std::{collections::HashMap, sync::Arc, time::Instant};

use anyhow::{anyhow, Context, Result};
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use hyper::body::HttpBody as _;
use hyper::header::{HeaderMap, HeaderValue, HOST};
use hyper::{Body, Request, Response, Uri, Version};
use rustls::ServerName;
use tokio::net::TcpStream;
use tokio::time::{timeout, Duration};
use tokio_rustls::rustls::ClientConfig;
use tokio_rustls::TlsConnector;

use crate::core::config::model::AppConfig;
use crate::core::ip_pool::global::pick_best_async;
use crate::core::ip_pool::global::report_outcome_async;
use crate::core::ip_pool::IpOutcome;
use crate::core::ip_pool::IpSelectionStrategy;
// Metrics / events instrumentation
use crate::core::ip_pool::events::emit_ip_pool_selection;
use crate::events::structured::{
    publish_global, Event as StructuredEvent, StrategyEvent as StructuredStrategyEvent,
};
use uuid::Uuid;
// Reuse existing metrics enabled flag (resides in git transport metrics module) to keep gating consistent
use crate::core::git::transport::metrics::metrics_enabled;
use crate::core::tls::util::{decide_sni_host_with_proxy, proxy_present};
use crate::core::tls::verifier::create_client_config;

use super::types::{HttpRequestInput, HttpResponseOutput, TimingInfo};

/// 内部简单 HTTP 客户端：使用手动连接 + hyper `client::conn，便于自定义` SNI
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
        // 注意：不能在 await 期间持有互斥锁，否则命令 Future 不满足 Send 约束。
        let start_total = Instant::now();
        // 使用异步桥接进行选择，避免在当前 runtime 中阻塞
        let sel = pick_best_async(&host, port).await;

        // If we used ip pool selection, create a small guard that will report outcome back to the pool
        // via the async bridge. By default the guard reports Failure; call .report_success() to mark success.
        struct OutcomeReporter {
            sel: Option<crate::core::ip_pool::IpSelection>,
        }
        impl OutcomeReporter {
            fn report_success(mut self) {
                if let Some(selection) = self.sel.take() {
                    report_outcome_async(selection, IpOutcome::Success);
                }
            }
        }
        impl Drop for OutcomeReporter {
            fn drop(&mut self) {
                if let Some(selection) = self.sel.take() {
                    report_outcome_async(selection, IpOutcome::Failure);
                }
            }
        }

        // Only keep reporter when selection used cached ip
        let mut outcome_reporter = if sel.strategy() == IpSelectionStrategy::Cached {
            Some(OutcomeReporter {
                sel: Some(sel.clone()),
            })
        } else {
            None
        };

        // Prepare observability context (shared id for selection + timing events)
        let metrics_on = metrics_enabled();
        let request_id = Uuid::new_v4();
        if metrics_on {
            // Emit IP pool selection structured event (mirrors git transport emission)
            let candidates_count = sel.iter_candidates().count().min(255) as u8;
            tracing::info!(
                target = "http",
                host = %host,
                port = port,
                strategy = ?sel.strategy(),
                candidates = candidates_count,
                "emitting IpPoolSelection event"
            );
            emit_ip_pool_selection(
                request_id,
                &host,
                port,
                sel.strategy(),
                sel.selected(),
                candidates_count,
            );
        }

        let (connect_addr, sni_host, used_ip_pool, connect_ms) = match sel.strategy() {
            IpSelectionStrategy::SystemDefault => {
                // 走原逻辑
                let start_connect = Instant::now();
                let tcp = timeout(
                    Duration::from_millis(input.timeout_ms),
                    TcpStream::connect((host.as_str(), port)),
                )
                .await
                .context("connect timeout")?
                .context("connect error")?;
                (
                    tcp,
                    host.clone(),
                    false,
                    start_connect.elapsed().as_millis() as u32,
                )
            }
            IpSelectionStrategy::Cached => {
                // 用IP池选中的IP建立连接，SNI仍用原host
                let ip = sel
                    .selected()
                    .map(|s| s.candidate.address)
                    .ok_or_else(|| anyhow!("ip pool returned no candidate"))?;
                let start_connect = Instant::now();
                let tcp = timeout(
                    Duration::from_millis(input.timeout_ms),
                    TcpStream::connect((ip, port)),
                )
                .await
                .context("connect timeout (ip pool)")?
                .context("connect error (ip pool)")?;
                (
                    tcp,
                    host.clone(),
                    true,
                    start_connect.elapsed().as_millis() as u32,
                )
            }
        };

        // TLS 握手，可能使用伪 SNI
        let (sni_host_final, fake) = self.compute_sni_host(input.force_real_sni, &sni_host);
        let server_name = ServerName::try_from(sni_host_final.as_str())
            .map_err(|_| anyhow!("invalid dns name for sni"))?;
        let start_tls = Instant::now();
        let tls = TlsConnector::from(self.tls.clone());
        let stream = tls
            .connect(server_name, connect_addr)
            .await
            .context("tls handshake")?;
        let tls_ms = start_tls.elapsed().as_millis() as u32;

        // 使用 hyper client::conn 手动发送请求
        let (mut sender, conn) = hyper::client::conn::handshake(stream)
            .await
            .context("http handshake")?;
        // 后台驱动连接
        tokio::spawn(async move {
            if let Err(e) = conn.await {
                tracing::debug!(target = "http", "conn ended: {:?}", e);
            }
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
                if let Ok(val) = hyper::header::HeaderValue::try_from(v) {
                    headers_map.insert(name, val);
                }
            }
        }
        // 覆盖/写入 Host 头为真实域
        self.upsert_host_header(headers_map, &host);

        let req = req_builder
            .body(Body::from(body_bytes))
            .expect("request body");

        let start_first = Instant::now();
        let mut resp: Response<Body> = sender.send_request(req).await.context("send request")?;
        let first_byte_ms = start_first.elapsed().as_millis() as u32;

        // 收集响应头
        let status = resp.status().as_u16();
        let mut headers: HashMap<String, String> = HashMap::new();
        for (k, v) in resp.headers().iter() {
            if let Ok(vs) = v.to_str() {
                headers.insert(k.to_string(), vs.to_string());
            }
        }

        // 读取完整响应体
        let mut buf: Vec<u8> = Vec::new();
        while let Some(next) = resp.body_mut().data().await {
            let chunk = next.context("read body")?;
            buf.extend_from_slice(&chunk);
        }
        let total_ms = start_total.elapsed().as_millis() as u32;

        let body_size = buf.len();
        if self.should_warn_large_body(body_size) {
            tracing::warn!(target = "http", size = body_size, "large body warning");
        }

        // 目前不做重定向跟随（由 P0.5 统一处理），返回基础数据
        let out = HttpResponseOutput {
            ok: (200..300).contains(&status),
            status,
            headers,
            body_base64: BASE64.encode(&buf),
            used_fake_sni: fake,
            ip: if used_ip_pool {
                sel.selected().map(|s| s.candidate.address.to_string())
            } else {
                None
            },
            timing: TimingInfo {
                connect_ms,
                tls_ms,
                first_byte_ms,
                total_ms,
            },
            redirects: vec![],
            body_size,
        };

        if metrics_on {
            // Map strategy enum to label (consistent with IpPoolSelection emission helper)
            let strategy_label = match sel.strategy() {
                IpSelectionStrategy::Cached => "Cached",
                IpSelectionStrategy::SystemDefault => "SystemDefault",
            };
            let ip_source = if used_ip_pool {
                sel.selected().map(|stat| {
                    stat.sources
                        .iter()
                        .map(|s| format!("{:?}", s))
                        .collect::<Vec<_>>()
                        .join(",")
                })
            } else {
                None
            };
            let ip_latency_ms = if used_ip_pool {
                sel.selected().and_then(|s| s.latency_ms)
            } else {
                None
            };
            publish_global(StructuredEvent::Strategy(
                StructuredStrategyEvent::AdaptiveTlsTiming {
                    id: request_id.to_string(),
                    kind: "HttpSingle".to_string(),
                    used_fake_sni: fake,
                    fallback_stage: "Direct".to_string(),
                    connect_ms: Some(connect_ms),
                    tls_ms: Some(tls_ms),
                    first_byte_ms: Some(first_byte_ms),
                    total_ms: Some(total_ms),
                    cert_fp_changed: false,
                    ip_source,
                    ip_latency_ms,
                    ip_selection_stage: Some(strategy_label.to_string()),
                },
            ));
        }
        // Mark success if we used ip pool and reached here
        if let Some(r) = outcome_reporter.take() {
            r.report_success();
        }
        Ok(out)
    }
}
