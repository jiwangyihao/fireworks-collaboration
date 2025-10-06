use super::aggregate::HistogramWindowConfig;
use super::error::MetricError;
use super::registry::{MetricDescriptor, MetricKind, MetricRegistry};

pub const LATENCY_MS_BUCKETS: &[f64] = &[
    1.0, 5.0, 10.0, 25.0, 50.0, 75.0, 100.0, 150.0, 200.0, 300.0, 500.0, 750.0, 1_000.0, 1_500.0,
    2_000.0, 3_000.0, 5_000.0,
];

pub const GIT_TASKS_TOTAL: MetricDescriptor = MetricDescriptor::counter(
    "git_tasks_total",
    "Git task terminal states",
    &["kind", "state"],
);

pub const GIT_TASK_DURATION_MS: MetricDescriptor = MetricDescriptor::histogram(
    "git_task_duration_ms",
    "Git task durations in milliseconds",
    &["kind"],
    LATENCY_MS_BUCKETS,
);

pub const GIT_RETRY_TOTAL: MetricDescriptor = MetricDescriptor::counter(
    "git_retry_total",
    "Git retry attempts by category",
    &["kind", "category"],
);

pub const TLS_HANDSHAKE_MS: MetricDescriptor = MetricDescriptor::histogram(
    "tls_handshake_ms",
    "TLS handshake latency distribution",
    &["sni_strategy", "outcome"],
    LATENCY_MS_BUCKETS,
);

pub const IP_POOL_SELECTION_TOTAL: MetricDescriptor = MetricDescriptor::counter(
    "ip_pool_selection_total",
    "IP pool selection attempts",
    &["strategy", "outcome"],
);

pub const IP_POOL_REFRESH_TOTAL: MetricDescriptor = MetricDescriptor::counter(
    "ip_pool_refresh_total",
    "IP pool refresh outcomes",
    &["reason", "success"],
);

pub const IP_POOL_LATENCY_MS: MetricDescriptor = MetricDescriptor::histogram(
    "ip_pool_latency_ms",
    "IP candidate latency samples",
    &["source"],
    LATENCY_MS_BUCKETS,
);

pub const IP_POOL_AUTO_DISABLE_TOTAL: MetricDescriptor = MetricDescriptor::counter(
    "ip_pool_auto_disable_total",
    "IP pool auto disable triggers",
    &["reason"],
);

pub const CIRCUIT_BREAKER_TRIP_TOTAL: MetricDescriptor = MetricDescriptor::counter(
    "circuit_breaker_trip_total",
    "Circuit breaker trip events",
    &["reason"],
);

pub const CIRCUIT_BREAKER_RECOVER_TOTAL: MetricDescriptor = MetricDescriptor::counter(
    "circuit_breaker_recover_total",
    "Circuit breaker recover events",
    &[],
);

pub const PROXY_FALLBACK_TOTAL: MetricDescriptor = MetricDescriptor::counter(
    "proxy_fallback_total",
    "Proxy fallback occurrences",
    &["reason"],
);

pub const HTTP_STRATEGY_FALLBACK_TOTAL: MetricDescriptor = MetricDescriptor::counter(
    "http_strategy_fallback_total",
    "HTTP strategy fallback transitions",
    &["stage", "from"],
);

pub const SOAK_THRESHOLD_VIOLATION_TOTAL: MetricDescriptor = MetricDescriptor::counter(
    "soak_threshold_violation_total",
    "Soak threshold violations",
    &["name"],
);

pub const ALERTS_FIRED_TOTAL: MetricDescriptor =
    MetricDescriptor::counter("alerts_fired_total", "Alert firing events", &["severity"]);

pub const METRICS_EXPORT_REQUESTS_TOTAL: MetricDescriptor = MetricDescriptor::counter(
    "metrics_export_requests_total",
    "Metrics export request outcomes",
    &["status"],
);

pub const METRICS_EXPORT_SERIES_TOTAL: MetricDescriptor = MetricDescriptor::counter(
    "metrics_export_series_total",
    "Metrics export series returned",
    &["endpoint"],
);

pub const METRICS_EXPORT_RATE_LIMITED_TOTAL: MetricDescriptor = MetricDescriptor::counter(
    "metrics_export_rate_limited_total",
    "Metrics export rate limited responses",
    &[],
);

pub const METRIC_MEMORY_PRESSURE_TOTAL: MetricDescriptor = MetricDescriptor::counter(
    "metric_memory_pressure_total",
    "Metric runtime memory pressure events",
    &[],
);

pub const OBSERVABILITY_LAYER: MetricDescriptor = MetricDescriptor::gauge(
    "observability_layer",
    "Current observability layer status",
    &[],
);

pub const BASIC_METRICS: &[MetricDescriptor] = &[
    GIT_TASKS_TOTAL,
    GIT_TASK_DURATION_MS,
    GIT_RETRY_TOTAL,
    TLS_HANDSHAKE_MS,
    IP_POOL_SELECTION_TOTAL,
    IP_POOL_REFRESH_TOTAL,
    IP_POOL_LATENCY_MS,
    IP_POOL_AUTO_DISABLE_TOTAL,
    CIRCUIT_BREAKER_TRIP_TOTAL,
    CIRCUIT_BREAKER_RECOVER_TOTAL,
    PROXY_FALLBACK_TOTAL,
    HTTP_STRATEGY_FALLBACK_TOTAL,
    SOAK_THRESHOLD_VIOLATION_TOTAL,
    ALERTS_FIRED_TOTAL,
    METRICS_EXPORT_REQUESTS_TOTAL,
    METRICS_EXPORT_SERIES_TOTAL,
    METRICS_EXPORT_RATE_LIMITED_TOTAL,
    METRIC_MEMORY_PRESSURE_TOTAL,
    OBSERVABILITY_LAYER,
];

pub fn all_descriptors() -> &'static [MetricDescriptor] {
    BASIC_METRICS
}

pub fn find_descriptor(name: &str) -> Option<MetricDescriptor> {
    BASIC_METRICS.iter().copied().find(|desc| desc.name == name)
}

pub fn register_basic_metrics(registry: &MetricRegistry) -> Result<(), MetricError> {
    for desc in BASIC_METRICS {
        if let Err(err) = registry.register(*desc) {
            if !matches!(err, MetricError::AlreadyRegistered(_)) {
                return Err(err);
            }
        }
        match desc.kind {
            MetricKind::Counter => registry.enable_counter_window(*desc),
            MetricKind::Histogram => {
                registry.enable_histogram_window(*desc, HistogramWindowConfig::default());
            }
            MetricKind::Gauge => {}
        }
    }
    Ok(())
}
