use once_cell::sync::OnceCell;
use std::sync::Arc;

use crate::core::config::model::ObservabilityConfig;

mod aggregate;
mod alerts;
mod descriptors;
mod error;
mod event_bridge;
mod export;
mod registry;
mod runtime;

pub use aggregate::{
    CounterWindowSnapshot, HistogramRawSample, HistogramWindowConfig, HistogramWindowSnapshot,
    ManualTimeProvider, SystemTimeProvider, TimeProvider, WindowPoint, WindowRange,
    WindowResolution, WindowSeriesDescriptor,
};
pub use descriptors::*;
pub use error::{MetricError, MetricInitError};
pub use export::{
    build_snapshot, encode_prometheus, parse_snapshot_query, start_http_server, MetricsSnapshot,
    MetricsSnapshotSeries, SnapshotQuery, SnapshotQueryError,
};
pub use registry::{HistogramSnapshot, MetricDescriptor, MetricKind, MetricRegistry};
pub use runtime::SampleKind;

static REGISTRY: OnceCell<Arc<MetricRegistry>> = OnceCell::new();
static BASIC_INIT: OnceCell<()> = OnceCell::new();
static BRIDGE: OnceCell<Arc<event_bridge::EventMetricsBridge>> = OnceCell::new();
static AGGREGATE_INIT: OnceCell<()> = OnceCell::new();
static AGGREGATOR: OnceCell<Arc<aggregate::WindowAggregator>> = OnceCell::new();
static EXPORT_HANDLE: OnceCell<export::MetricsServerHandle> = OnceCell::new();
static ALERTS_INIT: OnceCell<()> = OnceCell::new();
static ALERT_ENGINE: OnceCell<Arc<alerts::AlertEngine>> = OnceCell::new();

pub fn global_registry() -> Arc<MetricRegistry> {
    REGISTRY
        .get_or_init(|| Arc::new(MetricRegistry::new()))
        .clone()
}

pub fn init_basic_observability(cfg: &ObservabilityConfig) -> Result<(), MetricInitError> {
    if !cfg.enabled || !cfg.basic_enabled {
        return Ok(());
    }

    BASIC_INIT.get_or_try_init(|| -> Result<(), MetricInitError> {
        let registry = global_registry();
        descriptors::register_basic_metrics(&registry)?;
        runtime::init(cfg, registry.clone())?;
        let fanout =
            crate::events::structured::ensure_fanout_bus().map_err(MetricInitError::EventBus)?;
        let bridge = Arc::new(event_bridge::EventMetricsBridge::new(registry));
        fanout.register(bridge.clone());
        let _ = BRIDGE.set(bridge);
        Ok(())
    })?;

    Ok(())
}

pub fn init_aggregate_observability(cfg: &ObservabilityConfig) -> Result<(), MetricInitError> {
    let provider: Arc<dyn TimeProvider> = Arc::new(SystemTimeProvider::default());
    init_aggregate_observability_with_provider(cfg, provider)
}

pub fn init_aggregate_observability_with_provider(
    cfg: &ObservabilityConfig,
    provider: Arc<dyn TimeProvider>,
) -> Result<(), MetricInitError> {
    if !cfg.enabled || !cfg.basic_enabled || !cfg.aggregate_enabled {
        return Ok(());
    }

    init_basic_observability(cfg)?;

    if AGGREGATOR.get().is_some() {
        return Ok(());
    }

    let registry = global_registry();
    let aggregator = Arc::new(aggregate::WindowAggregator::new(provider));
    registry.attach_aggregator(aggregator.clone());
    let _ = AGGREGATOR.set(aggregator);
    let _ = AGGREGATE_INIT.set(());
    Ok(())
}

pub fn aggregate_enabled() -> bool {
    AGGREGATOR.get().is_some()
}

pub fn init_export_observability(cfg: &ObservabilityConfig) -> Result<(), MetricInitError> {
    if !cfg.enabled || !cfg.basic_enabled || !cfg.export_enabled {
        return Ok(());
    }

    init_aggregate_observability(cfg)?;

    if EXPORT_HANDLE.get().is_some() {
        return Ok(());
    }

    let handle = export::start_http_server(&cfg.export)
        .map_err(|err| MetricInitError::Export(err.to_string()))?;
    let _ = EXPORT_HANDLE.set(handle);
    Ok(())
}

pub fn init_alerts_observability(cfg: &ObservabilityConfig) -> Result<(), MetricInitError> {
    if !cfg.enabled || !cfg.basic_enabled || !cfg.alerts_enabled {
        return Ok(());
    }

    init_aggregate_observability(cfg)?;

    ALERTS_INIT.get_or_try_init(|| -> Result<(), MetricInitError> {
        let registry = global_registry();
        let engine = Arc::new(
            alerts::AlertEngine::new(registry, cfg.alerts.clone())
                .map_err(|err| MetricInitError::Alerts(err.to_string()))?,
        );
        engine.evaluate();
        engine.spawn();
        let _ = ALERT_ENGINE.set(engine);
        Ok(())
    })?;

    Ok(())
}

pub fn evaluate_alerts_now() {
    if let Some(engine) = ALERT_ENGINE.get() {
        engine.evaluate();
    }
}

pub fn configure_tls_sampling(rate: u32) {
    runtime::configure_tls_sample_rate(rate);
}

pub fn set_runtime_debug_mode(enabled: bool) {
    runtime::configure_debug_mode(enabled);
}

pub fn set_runtime_ip_mode(mode: crate::core::config::model::ObservabilityRedactIpMode) {
    runtime::configure_ip_mode(mode);
}

pub fn set_runtime_memory_limit(bytes: u64) {
    runtime::configure_memory_limit(bytes);
}

pub fn force_memory_pressure_check() {
    runtime::force_memory_pressure_check();
}

pub fn record_counter_metric(desc: MetricDescriptor, labels: &[(&'static str, &str)], value: u64) {
    runtime::record_counter(desc, labels, value);
}

pub fn observe_histogram_metric(
    desc: MetricDescriptor,
    labels: &[(&'static str, &str)],
    value: f64,
) {
    runtime::observe_histogram(desc, labels, value, SampleKind::None);
}

pub fn observe_histogram_with_kind(
    desc: MetricDescriptor,
    labels: &[(&'static str, &str)],
    value: f64,
    kind: SampleKind,
) {
    runtime::observe_histogram(desc, labels, value, kind);
}
