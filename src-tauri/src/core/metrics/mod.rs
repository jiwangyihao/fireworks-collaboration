use once_cell::sync::OnceCell;
use std::sync::Arc;

use crate::core::config::model::ObservabilityConfig;

mod descriptors;
mod error;
mod event_bridge;
mod registry;

pub use descriptors::*;
pub use error::{MetricError, MetricInitError};
pub use registry::{HistogramSnapshot, MetricDescriptor, MetricKind, MetricRegistry};

static REGISTRY: OnceCell<Arc<MetricRegistry>> = OnceCell::new();
static BASIC_INIT: OnceCell<()> = OnceCell::new();
static BRIDGE: OnceCell<Arc<event_bridge::EventMetricsBridge>> = OnceCell::new();

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
        let fanout =
            crate::events::structured::ensure_fanout_bus().map_err(MetricInitError::EventBus)?;
        let bridge = Arc::new(event_bridge::EventMetricsBridge::new(registry));
        fanout.register(bridge.clone());
        let _ = BRIDGE.set(bridge);
        Ok(())
    })?;

    Ok(())
}
