use std::collections::{HashMap, HashSet};
use std::fmt::Write;
use std::net::{SocketAddr, ToSocketAddrs};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use hyper::header::{HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use hyper::server::conn::AddrStream;
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Method, Request, Response, StatusCode, Uri};
use serde::Serialize;
use sha2::{Digest, Sha256};
use thiserror::Error;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};
use url::form_urlencoded;

use crate::core::config::model::ObservabilityExportConfig;

use super::aggregate::{CounterWindowSnapshot, HistogramWindowSnapshot, WindowRange};
use super::descriptors::{
    METRICS_EXPORT_RATE_LIMITED_TOTAL, METRICS_EXPORT_REQUESTS_TOTAL, METRICS_EXPORT_SERIES_TOTAL,
};
use super::registry::{
    CounterSeriesSnapshot, HistogramSeriesSnapshot, HistogramSnapshot, MetricDescriptor,
    MetricKind, MetricRegistry,
};
use super::{global_registry, MetricError};

const SNAPSHOT_DEFAULT_RANGE: WindowRange = WindowRange::LastFiveMinutes;

#[derive(Debug, Clone, Default)]
pub struct SnapshotQuery {
    pub names: Vec<String>,
    pub range: Option<WindowRange>,
    pub quantiles: Vec<f64>,
}

#[derive(Debug, Error)]
pub enum SnapshotQueryError {
    #[error("invalid range value '{0}'")]
    InvalidRange(String),
    #[error("invalid quantile value '{0}'")]
    InvalidQuantile(String),
}

#[derive(Debug, Error)]
pub enum ExportError {
    #[error("invalid bind address '{0}'")]
    InvalidBindAddress(String),
    #[error("failed to bind metrics exporter: {0}")]
    Bind(std::io::Error),
    #[error("failed to start metrics exporter: {0}")]
    Start(hyper::Error),
}

#[derive(Clone)]
struct ExportSettings {
    auth_token: Option<String>,
    rate_limit_qps: u32,
    max_series: usize,
    bind_address: String,
}

impl ExportSettings {
    fn from_config(cfg: &ObservabilityExportConfig) -> Self {
        Self {
            auth_token: cfg.auth_token.as_ref().and_then(|token| {
                if token.trim().is_empty() {
                    None
                } else {
                    Some(token.clone())
                }
            }),
            rate_limit_qps: cfg.rate_limit_qps,
            max_series: usize::try_from(cfg.max_series_per_snapshot).unwrap_or(usize::MAX),
            bind_address: cfg.bind_address.clone(),
        }
    }

    fn resolve_addr(&self) -> Result<SocketAddr, ExportError> {
        let mut candidates = self
            .bind_address
            .to_socket_addrs()
            .map_err(|_| ExportError::InvalidBindAddress(self.bind_address.clone()))?;
        candidates
            .next()
            .ok_or_else(|| ExportError::InvalidBindAddress(self.bind_address.clone()))
    }
}

#[derive(Debug)]
pub struct MetricsServerHandle {
    pub local_addr: SocketAddr,
    shutdown_tx: Mutex<Option<oneshot::Sender<()>>>,
    join_handle: JoinHandle<()>,
}

impl MetricsServerHandle {
    pub async fn shutdown(self) {
        if let Some(tx) = self
            .shutdown_tx
            .lock()
            .ok()
            .and_then(|mut guard| guard.take())
        {
            let _ = tx.send(());
        }
        if let Err(err) = self.join_handle.await {
            warn!(target = "metrics", error = %err, "metrics exporter task aborted");
        }
    }
}

#[derive(Clone)]
struct RateLimiter {
    state: Arc<Mutex<RateLimiterState>>,
}

#[derive(Debug)]
struct RateLimiterState {
    capacity: f64,
    tokens: f64,
    refill_per_sec: f64,
    last_refill: Instant,
}

impl RateLimiter {
    fn new(qps: u32) -> Option<Self> {
        if qps == 0 {
            return None;
        }
        let capacity = (qps.max(1) * 2) as f64;
        Some(Self {
            state: Arc::new(Mutex::new(RateLimiterState {
                capacity,
                tokens: capacity,
                refill_per_sec: qps as f64,
                last_refill: Instant::now(),
            })),
        })
    }

    fn allow(&self) -> bool {
        let mut guard = match self.state.lock() {
            Ok(guard) => guard,
            Err(_poisoned) => {
                warn!(
                    target = "metrics",
                    "rate limiter mutex poisoned; allowing request"
                );
                return true;
            }
        };
        let now = Instant::now();
        let elapsed = now
            .saturating_duration_since(guard.last_refill)
            .as_secs_f64();
        if elapsed > 0.0 {
            guard.tokens = (guard.tokens + elapsed * guard.refill_per_sec).min(guard.capacity);
            guard.last_refill = now;
        }
        if guard.tokens >= 1.0 {
            guard.tokens -= 1.0;
            true
        } else {
            false
        }
    }
}

#[derive(Clone)]
struct MetricsExporter {
    registry: Arc<MetricRegistry>,
    settings: ExportSettings,
    limiter: Option<RateLimiter>,
}

struct ServeResponse {
    response: Response<Body>,
    series_count: usize,
    endpoint: Option<&'static str>,
}

impl ServeResponse {
    fn new(response: Response<Body>) -> Self {
        Self {
            response,
            series_count: 0,
            endpoint: None,
        }
    }

    fn with_series(response: Response<Body>, endpoint: &'static str, series_count: usize) -> Self {
        Self {
            response,
            series_count,
            endpoint: Some(endpoint),
        }
    }
}

impl MetricsExporter {
    fn new(registry: Arc<MetricRegistry>, settings: ExportSettings) -> Self {
        let limiter = RateLimiter::new(settings.rate_limit_qps);
        Self {
            registry,
            settings,
            limiter,
        }
    }

    fn check_auth(&self, req: &Request<Body>) -> bool {
        match &self.settings.auth_token {
            None => true,
            Some(expected) => req
                .headers()
                .get(AUTHORIZATION)
                .and_then(|value| value.to_str().ok())
                .and_then(|header| header.strip_prefix("Bearer "))
                .map(|provided| provided.trim() == expected)
                .unwrap_or(false),
        }
    }

    fn record_request(&self, status: &str) {
        let labels = [("status", status)];
        if let Err(err) = self
            .registry
            .incr_counter(METRICS_EXPORT_REQUESTS_TOTAL, &labels, 1)
        {
            debug!(target = "metrics", error = %err, "failed to record metrics_export_requests_total");
        }
    }

    fn record_series(&self, endpoint: &str, series: usize) {
        if series == 0 {
            return;
        }
        let labels = [("endpoint", endpoint)];
        if let Err(err) =
            self.registry
                .incr_counter(METRICS_EXPORT_SERIES_TOTAL, &labels, series as u64)
        {
            debug!(target = "metrics", error = %err, "failed to record metrics_export_series_total");
        }
    }

    fn record_rate_limited(&self) {
        if let Err(err) = self
            .registry
            .incr_counter(METRICS_EXPORT_RATE_LIMITED_TOTAL, &[], 1)
        {
            debug!(target = "metrics", error = %err, "failed to record metrics_export_rate_limited_total");
        }
    }

    fn enforce_rate_limit(&self) -> bool {
        match &self.limiter {
            Some(limiter) => limiter.allow(),
            None => true,
        }
    }

    fn handle_prometheus(&self) -> (String, usize) {
        let counters = self.registry.collect_counter_series();
        let histograms = self.registry.collect_histogram_series();
        let text = encode_prometheus_internal(&counters, &histograms);
        let total_series = counters.len() + histograms.len();
        (text, total_series)
    }

    fn handle_snapshot(&self, query: &SnapshotQuery) -> MetricsSnapshot {
        build_snapshot_internal(&self.registry, query, self.settings.max_series)
    }

    async fn serve(&self, req: Request<Body>) -> ServeResponse {
        if req.method() != Method::GET {
            self.record_request("method_not_allowed");
            return ServeResponse::new(build_response(
                StatusCode::METHOD_NOT_ALLOWED,
                "method not allowed",
            ));
        }

        if !self.check_auth(&req) {
            self.record_request("unauthorized");
            return ServeResponse::new(build_response(StatusCode::UNAUTHORIZED, "unauthorized"));
        }

        if !self.enforce_rate_limit() {
            self.record_request("rate_limited");
            self.record_rate_limited();
            return ServeResponse::new(build_response(
                StatusCode::TOO_MANY_REQUESTS,
                "too many requests",
            ));
        }

        match req.uri().path() {
            "/metrics" => {
                let (body, series) = self.handle_prometheus();
                self.record_request("success");
                self.record_series("prometheus", series);
                let response = Response::builder()
                    .status(StatusCode::OK)
                    .header(CONTENT_TYPE, "text/plain; version=0.0.4")
                    .body(Body::from(body))
                    .expect("prometheus response");
                ServeResponse::with_series(response, "prometheus", series)
            }
            "/metrics/snapshot" => match parse_snapshot_query(req.uri()) {
                Ok(query) => {
                    let snapshot = self.handle_snapshot(&query);
                    let series_len = snapshot.series.len();
                    match serde_json::to_vec(&snapshot) {
                        Ok(payload) => {
                            self.record_request("success");
                            self.record_series("snapshot", series_len);
                            let response = Response::builder()
                                .status(StatusCode::OK)
                                .header(CONTENT_TYPE, "application/json")
                                .body(Body::from(payload))
                                .expect("snapshot response");
                            ServeResponse::with_series(response, "snapshot", series_len)
                        }
                        Err(err) => {
                            error!(target = "metrics", error = %err, "failed to serialize snapshot");
                            self.record_request("error");
                            ServeResponse::new(build_response(
                                StatusCode::INTERNAL_SERVER_ERROR,
                                "serialization error",
                            ))
                        }
                    }
                }
                Err(err) => {
                    self.record_request("bad_request");
                    ServeResponse::new(build_response(StatusCode::BAD_REQUEST, &err.to_string()))
                }
            },
            _ => {
                self.record_request("not_found");
                ServeResponse::new(build_response(StatusCode::NOT_FOUND, "not found"))
            }
        }
    }
}

pub fn parse_snapshot_query(uri: &Uri) -> Result<SnapshotQuery, SnapshotQueryError> {
    let mut names = Vec::new();
    let mut range = None;
    let mut quantiles = Vec::new();

    if let Some(query) = uri.query() {
        for (key, value) in form_urlencoded::parse(query.as_bytes()) {
            match key.as_ref() {
                "names" => {
                    names.extend(
                        value
                            .split(',')
                            .filter(|s| !s.trim().is_empty())
                            .map(|s| s.trim().to_string()),
                    );
                }
                "range" => {
                    range = Some(parse_range(value.as_ref())?);
                }
                "quantiles" => {
                    quantiles.extend(
                        value
                            .split(',')
                            .filter(|s| !s.trim().is_empty())
                            .map(|token| parse_quantile(token.trim()))
                            .collect::<Result<Vec<f64>, _>>()?,
                    );
                }
                _ => {}
            }
        }
    }

    Ok(SnapshotQuery {
        names,
        range,
        quantiles,
    })
}

fn parse_range(value: &str) -> Result<WindowRange, SnapshotQueryError> {
    match value {
        "1m" | "1minute" => Ok(WindowRange::LastMinute),
        "5m" | "5minutes" => Ok(WindowRange::LastFiveMinutes),
        "1h" | "1hour" => Ok(WindowRange::LastHour),
        "24h" | "1d" | "1day" => Ok(WindowRange::LastDay),
        other => Err(SnapshotQueryError::InvalidRange(other.to_string())),
    }
}

fn parse_quantile(token: &str) -> Result<f64, SnapshotQueryError> {
    if token.is_empty() {
        return Err(SnapshotQueryError::InvalidQuantile(token.to_string()));
    }
    if let Some(number) = token.strip_prefix('p').or_else(|| token.strip_prefix('P')) {
        if number.is_empty() {
            return Err(SnapshotQueryError::InvalidQuantile(token.to_string()));
        }
        let parsed: f64 = number
            .parse::<f64>()
            .map_err(|_| SnapshotQueryError::InvalidQuantile(token.to_string()))?;
        if !(0.0..=100.0).contains(&parsed) {
            return Err(SnapshotQueryError::InvalidQuantile(token.to_string()));
        }
        return Ok(parsed / 100.0);
    }
    let value: f64 = token
        .parse()
        .map_err(|_| SnapshotQueryError::InvalidQuantile(token.to_string()))?;
    if !(0.0..=1.0).contains(&value) {
        return Err(SnapshotQueryError::InvalidQuantile(token.to_string()));
    }
    Ok(value)
}

fn build_response(status: StatusCode, message: &str) -> Response<Body> {
    Response::builder()
        .status(status)
        .header(
            CONTENT_TYPE,
            HeaderValue::from_static("text/plain; charset=utf-8"),
        )
        .body(Body::from(message.as_bytes().to_vec()))
        .expect("build response")
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MetricsSnapshot {
    pub generated_at_ms: u64,
    pub series: Vec<MetricsSnapshotSeries>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MetricsSnapshotSeries {
    pub name: String,
    #[serde(rename = "type")]
    pub series_type: &'static str,
    pub labels: HashMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sum: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub count: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub buckets: Option<Vec<HistogramBucketDto>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quantiles: Option<HashMap<String, f64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub points: Option<Vec<CounterPointDto>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub histogram_points: Option<Vec<HistogramPointDto>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub range: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_samples: Option<Vec<HistogramSampleDto>>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct HistogramBucketDto {
    pub le: String,
    pub c: u64,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CounterPointDto {
    pub offset_seconds: u64,
    pub value: u64,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct HistogramPointDto {
    pub offset_seconds: u64,
    pub count: u64,
    pub sum: f64,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct HistogramSampleDto {
    pub offset_seconds: u64,
    pub value: f64,
}

pub fn encode_prometheus(registry: &MetricRegistry) -> String {
    let counters = registry.collect_counter_series();
    let histograms = registry.collect_histogram_series();
    encode_prometheus_internal(&counters, &histograms)
}

pub fn build_snapshot(
    registry: &MetricRegistry,
    query: &SnapshotQuery,
    max_series: usize,
) -> MetricsSnapshot {
    build_snapshot_internal(registry, query, max_series)
}

pub fn start_http_server(
    cfg: &ObservabilityExportConfig,
) -> Result<MetricsServerHandle, ExportError> {
    let settings = ExportSettings::from_config(cfg);
    let registry = global_registry();
    let exporter = MetricsExporter::new(registry, settings.clone());
    let addr = settings.resolve_addr()?;
    let listener = std::net::TcpListener::bind(addr).map_err(ExportError::Bind)?;
    listener.set_nonblocking(true).map_err(ExportError::Bind)?;
    let local_addr = listener.local_addr().map_err(ExportError::Bind)?;

    let exporter = exporter;
    let make_svc = make_service_fn(move |conn: &AddrStream| {
        let exporter = exporter.clone();
        let remote_addr = conn.remote_addr();
        async move {
            Ok::<_, hyper::Error>(service_fn(move |req| {
                let exporter = exporter.clone();
                let path = req.uri().path().to_string();
                let remote_addr = remote_addr;
                async move {
                    let start = Instant::now();
                    let result = exporter.serve(req).await;
                    let duration = start.elapsed();
                    let ServeResponse {
                        response,
                        series_count,
                        endpoint,
                    } = result;
                    let status = response.status();
                    log_access(remote_addr, &path, status, endpoint, series_count, duration);
                    Ok::<_, hyper::Error>(response)
                }
            }))
        }
    });

    let server = hyper::Server::from_tcp(listener)
        .map_err(ExportError::Start)?
        .serve(make_svc);

    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
    let graceful = server.with_graceful_shutdown(async move {
        let _ = shutdown_rx.await;
    });
    let join_handle = tokio::spawn(async move {
        if let Err(err) = graceful.await {
            error!(target = "metrics", error = %err, "metrics exporter stopped unexpectedly");
        }
    });

    info!(target = "metrics", address = %local_addr, "metrics exporter listening");

    Ok(MetricsServerHandle {
        local_addr,
        shutdown_tx: Mutex::new(Some(shutdown_tx)),
        join_handle,
    })
}

fn encode_prometheus_internal(
    counters: &[CounterSeriesSnapshot],
    histograms: &[HistogramSeriesSnapshot],
) -> String {
    let mut out = String::new();
    let mut emitted = HashSet::new();

    for counter in counters {
        emit_descriptor(&mut out, &mut emitted, counter.descriptor);
        let labels = format_labels(counter.descriptor, &counter.label_values);
        out.push_str(&format!(
            "{}{} {}\n",
            counter.descriptor.name, labels, counter.value
        ));
    }

    for histogram in histograms {
        emit_descriptor(&mut out, &mut emitted, histogram.descriptor);
        let base_labels = format_labels(histogram.descriptor, &histogram.label_values);
        let mut cumulative = 0u64;
        for (boundary, count) in histogram.snapshot.buckets.iter() {
            cumulative = cumulative.saturating_add(*count);
            let bucket_labels =
                format_bucket_labels(histogram.descriptor, &histogram.label_values, *boundary);
            out.push_str(&format!(
                "{}_bucket{} {}\n",
                histogram.descriptor.name, bucket_labels, cumulative
            ));
        }
        out.push_str(&format!(
            "{}_sum{} {}\n",
            histogram.descriptor.name,
            base_labels,
            format_float(histogram.snapshot.sum)
        ));
        out.push_str(&format!(
            "{}_count{} {}\n",
            histogram.descriptor.name, base_labels, histogram.snapshot.count
        ));
    }

    out
}

fn build_snapshot_internal(
    registry: &MetricRegistry,
    query: &SnapshotQuery,
    max_series: usize,
) -> MetricsSnapshot {
    let now = now_ms();
    let range = query.range.unwrap_or(SNAPSHOT_DEFAULT_RANGE);
    let quantiles = if query.quantiles.is_empty() {
        vec![0.5, 0.9, 0.95]
    } else {
        query.quantiles.clone()
    };
    let filter: Option<HashSet<&str>> = if query.names.is_empty() {
        None
    } else {
        Some(query.names.iter().map(|s| s.as_str()).collect())
    };
    let range_str = range_to_string(range);
    let mut output = Vec::new();
    let mut remaining = if max_series == 0 {
        usize::MAX
    } else {
        max_series
    };

    for counter in registry.collect_counter_series() {
        if !include_metric(&filter, counter.descriptor.name) {
            continue;
        }
        let labels = labels_map(counter.descriptor, &counter.label_values);
        let label_refs = build_label_refs(counter.descriptor, &counter.label_values);
        let window_snapshot = match registry.snapshot_counter_window(
            counter.descriptor,
            &label_refs,
            range,
        ) {
            Ok(snapshot) => Some(snapshot),
            Err(MetricError::AggregatorDisabled) | Err(MetricError::SeriesNotFound { .. }) => None,
            Err(err) => {
                debug!(target = "metrics", error = %err, metric = counter.descriptor.name, "counter window snapshot failed");
                None
            }
        };
        let points = window_snapshot
            .as_ref()
            .map(|snapshot| convert_counter_points(snapshot));
        let series = MetricsSnapshotSeries {
            name: counter.descriptor.name.to_string(),
            series_type: "counter",
            labels,
            value: Some(counter.value),
            sum: None,
            count: None,
            buckets: None,
            quantiles: None,
            points,
            histogram_points: None,
            range: window_snapshot.as_ref().map(|_| range_str.clone()),
            raw_samples: None,
        };
        output.push(series);
        if remaining != usize::MAX {
            remaining = remaining.saturating_sub(1);
            if remaining == 0 {
                return MetricsSnapshot {
                    generated_at_ms: now,
                    series: output,
                };
            }
        }
    }

    for histogram in registry.collect_histogram_series() {
        if remaining == 0 {
            break;
        }
        if !include_metric(&filter, histogram.descriptor.name) {
            continue;
        }
        let labels = labels_map(histogram.descriptor, &histogram.label_values);
        let label_refs = build_label_refs(histogram.descriptor, &histogram.label_values);
        let window_snapshot = match registry.snapshot_histogram_window(
            histogram.descriptor,
            &label_refs,
            range,
            &quantiles,
        ) {
            Ok(snapshot) => Some(snapshot),
            Err(MetricError::AggregatorDisabled) | Err(MetricError::SeriesNotFound { .. }) => None,
            Err(err) => {
                debug!(target = "metrics", error = %err, metric = histogram.descriptor.name, "histogram window snapshot failed");
                None
            }
        };
        let buckets = Some(convert_histogram_buckets(&histogram.snapshot));
        let quantile_map = window_snapshot
            .as_ref()
            .and_then(|snapshot| convert_quantiles(snapshot));
        let histogram_points = window_snapshot
            .as_ref()
            .map(|snapshot| convert_histogram_points(snapshot));
        let raw_samples = window_snapshot
            .as_ref()
            .map(|snapshot| convert_raw_samples(snapshot));
        let series = MetricsSnapshotSeries {
            name: histogram.descriptor.name.to_string(),
            series_type: "histogram",
            labels,
            value: None,
            sum: Some(histogram.snapshot.sum),
            count: Some(histogram.snapshot.count),
            buckets,
            quantiles: quantile_map,
            points: None,
            histogram_points,
            range: window_snapshot.as_ref().map(|_| range_str.clone()),
            raw_samples,
        };
        output.push(series);
        if remaining != usize::MAX {
            remaining = remaining.saturating_sub(1);
        }
    }

    MetricsSnapshot {
        generated_at_ms: now,
        series: output,
    }
}

fn emit_descriptor(out: &mut String, emitted: &mut HashSet<&'static str>, desc: MetricDescriptor) {
    if !emitted.insert(desc.name) {
        return;
    }
    out.push_str(&format!("# HELP {} {}\n", desc.name, desc.help));
    let metric_type = match desc.kind {
        MetricKind::Counter => "counter",
        MetricKind::Histogram => "histogram",
        MetricKind::Gauge => "gauge",
    };
    out.push_str(&format!("# TYPE {} {}\n", desc.name, metric_type));
}

fn format_labels(desc: MetricDescriptor, values: &[String]) -> String {
    if desc.labels.is_empty() {
        return String::new();
    }
    let pairs: Vec<String> = desc
        .labels
        .iter()
        .enumerate()
        .map(|(idx, label)| {
            let value = values.get(idx).map(|s| s.as_str()).unwrap_or("unknown");
            format!("{}=\"{}\"", label, escape_label(value))
        })
        .collect();
    format!("{{{}}}", pairs.join(","))
}

fn format_bucket_labels(desc: MetricDescriptor, values: &[String], boundary: f64) -> String {
    let mut parts: Vec<String> = desc
        .labels
        .iter()
        .enumerate()
        .map(|(idx, label)| {
            let value = values.get(idx).map(|s| s.as_str()).unwrap_or("unknown");
            format!("{}=\"{}\"", label, escape_label(value))
        })
        .collect();
    parts.push(format!("le=\"{}\"", format_bucket_value(boundary)));
    format!("{{{}}}", parts.join(","))
}

fn format_bucket_value(boundary: f64) -> String {
    if boundary.is_infinite() {
        "+Inf".to_string()
    } else {
        format_float(boundary)
    }
}

fn format_float(value: f64) -> String {
    if value.is_nan() {
        return "NaN".to_string();
    }
    if value.is_infinite() {
        return if value.is_sign_negative() {
            "-Inf".to_string()
        } else {
            "Inf".to_string()
        };
    }
    let mut s = format!("{:.6}", value);
    while s.contains('.') && s.ends_with('0') {
        s.pop();
    }
    if s.ends_with('.') {
        s.pop();
    }
    if s.is_empty() {
        s.push('0');
    }
    s
}

fn escape_label(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}

fn include_metric(filter: &Option<HashSet<&str>>, name: &str) -> bool {
    filter
        .as_ref()
        .map(|set| set.contains(name))
        .unwrap_or(true)
}

fn labels_map(desc: MetricDescriptor, values: &[String]) -> HashMap<String, String> {
    desc.labels
        .iter()
        .enumerate()
        .map(|(idx, label)| {
            let value = values
                .get(idx)
                .cloned()
                .unwrap_or_else(|| "unknown".to_string());
            ((*label).to_string(), value)
        })
        .collect()
}

fn build_label_refs<'a>(
    desc: MetricDescriptor,
    values: &'a [String],
) -> Vec<(&'static str, &'a str)> {
    desc.labels
        .iter()
        .enumerate()
        .filter_map(|(idx, label)| values.get(idx).map(|value| (*label, value.as_str())))
        .collect()
}

fn convert_counter_points(snapshot: &CounterWindowSnapshot) -> Vec<CounterPointDto> {
    snapshot
        .points
        .iter()
        .map(|point| CounterPointDto {
            offset_seconds: point.offset_seconds,
            value: point.value,
        })
        .collect()
}

fn convert_histogram_buckets(snapshot: &HistogramSnapshot) -> Vec<HistogramBucketDto> {
    snapshot
        .buckets
        .iter()
        .map(|(boundary, count)| HistogramBucketDto {
            le: format_bucket_value(*boundary),
            c: *count,
        })
        .collect()
}

fn convert_histogram_points(snapshot: &HistogramWindowSnapshot) -> Vec<HistogramPointDto> {
    snapshot
        .points
        .iter()
        .map(|point| HistogramPointDto {
            offset_seconds: point.offset_seconds,
            count: point.count,
            sum: point.sum,
        })
        .collect()
}

fn convert_raw_samples(snapshot: &HistogramWindowSnapshot) -> Vec<HistogramSampleDto> {
    snapshot
        .raw_samples
        .iter()
        .map(|sample| HistogramSampleDto {
            offset_seconds: sample.offset_seconds,
            value: sample.value,
        })
        .collect()
}

fn convert_quantiles(snapshot: &HistogramWindowSnapshot) -> Option<HashMap<String, f64>> {
    if snapshot.quantiles.is_empty() {
        return None;
    }
    let map = snapshot
        .quantiles
        .iter()
        .map(|(quantile, value)| (format_quantile_key(*quantile), *value))
        .collect();
    Some(map)
}

fn format_quantile_key(q: f64) -> String {
    let pct = q * 100.0;
    let mut s = format!("{:.3}", pct);
    while s.contains('.') && s.ends_with('0') {
        s.pop();
    }
    if s.ends_with('.') {
        s.pop();
    }
    format!("p{}", s)
}

fn range_to_string(range: WindowRange) -> String {
    match range {
        WindowRange::LastMinute => "1m".to_string(),
        WindowRange::LastFiveMinutes => "5m".to_string(),
        WindowRange::LastHour => "1h".to_string(),
        WindowRange::LastDay => "24h".to_string(),
    }
}

fn log_access(
    remote: SocketAddr,
    path: &str,
    status: StatusCode,
    endpoint: Option<&'static str>,
    series: usize,
    duration: Duration,
) {
    let remote_hash = hash_socket_addr(remote);
    let duration_ms = duration.as_secs_f64() * 1_000.0;
    info!(
        target = "metrics",
        timestamp_ms = now_ms(),
        remote = %remote_hash,
        path = path,
        status = status.as_u16(),
        endpoint = endpoint.unwrap_or("n/a"),
        series = series,
        duration_ms = duration_ms,
        "metrics exporter request"
    );
}

fn hash_socket_addr(addr: SocketAddr) -> String {
    let mut hasher = Sha256::new();
    hasher.update(addr.to_string().as_bytes());
    let digest = hasher.finalize();
    let mut out = String::with_capacity(12);
    for byte in digest.iter().take(6) {
        let _ = write!(&mut out, "{:02x}", byte);
    }
    out
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_secs(0))
        .as_millis() as u64
}
