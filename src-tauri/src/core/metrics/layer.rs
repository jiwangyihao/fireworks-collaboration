use once_cell::sync::OnceCell;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tracing::{info, warn};

use crate::core::config::model::{ObservabilityConfig, ObservabilityLayer};
use crate::events::structured::{publish_global, Event, StrategyEvent};

use super::descriptors::OBSERVABILITY_LAYER;
use super::{MetricError, MetricInitError, MetricRegistry};

#[derive(Debug, Clone)]
pub struct ResolvedLayerConfig {
    pub enabled: bool,
    pub basic_enabled: bool,
    pub aggregate_enabled: bool,
    pub export_enabled: bool,
    pub ui_enabled: bool,
    pub alerts_enabled: bool,
    pub optimize_enabled: bool,
    pub target_layer: ObservabilityLayer,
    pub effective_layer: ObservabilityLayer,
    pub max_allowed_layer: ObservabilityLayer,
}

impl ResolvedLayerConfig {
    pub fn allows(&self, layer: ObservabilityLayer) -> bool {
        self.effective_layer >= layer
    }
}

pub fn resolve_config(cfg: &ObservabilityConfig) -> ResolvedLayerConfig {
    let base_enabled = cfg.enabled && cfg.basic_enabled;
    let aggregate_flag = base_enabled && cfg.aggregate_enabled;
    let export_flag = aggregate_flag && cfg.export_enabled;
    let ui_flag = export_flag && cfg.ui_enabled;
    let alerts_flag = ui_flag && cfg.alerts_enabled;

    let highest_by_flags = if !base_enabled {
        ObservabilityLayer::Basic
    } else if !aggregate_flag {
        ObservabilityLayer::Basic
    } else if !export_flag {
        ObservabilityLayer::Aggregate
    } else if !ui_flag {
        ObservabilityLayer::Export
    } else if !alerts_flag {
        ObservabilityLayer::Ui
    } else {
        ObservabilityLayer::Optimize
    };

    let target = cfg.layer;
    let effective_layer = if base_enabled {
        target.min(highest_by_flags)
    } else {
        ObservabilityLayer::Basic
    };

    let aggregate_enabled = aggregate_flag && effective_layer >= ObservabilityLayer::Aggregate;
    let export_enabled = export_flag && effective_layer >= ObservabilityLayer::Export;
    let ui_enabled = ui_flag && effective_layer >= ObservabilityLayer::Ui;
    let alerts_enabled = alerts_flag && effective_layer >= ObservabilityLayer::Alerts;
    let optimize_enabled = alerts_flag && effective_layer == ObservabilityLayer::Optimize;

    ResolvedLayerConfig {
        enabled: cfg.enabled,
        basic_enabled: base_enabled,
        aggregate_enabled,
        export_enabled,
        ui_enabled,
        alerts_enabled,
        optimize_enabled,
        target_layer: target,
        effective_layer,
        max_allowed_layer: highest_by_flags,
    }
}

static LAYER_MANAGER: OnceCell<Arc<LayerManager>> = OnceCell::new();

pub fn initialize(
    cfg: &ObservabilityConfig,
    registry: Arc<MetricRegistry>,
) -> Result<ResolvedLayerConfig, MetricInitError> {
    let resolved = resolve_config(cfg);
    let manager = if let Some(existing) = LAYER_MANAGER.get() {
        existing.clone()
    } else {
        let manager = Arc::new(LayerManager::new(registry.clone(), &resolved, cfg)?);
        let _ = LAYER_MANAGER.set(manager.clone());
        manager
    };
    manager.update_from_resolved(&resolved, cfg);
    Ok(resolved)
}

pub fn current_layer() -> ObservabilityLayer {
    LAYER_MANAGER
        .get()
        .map(|manager| manager.current_layer())
        .unwrap_or(ObservabilityLayer::Basic)
}

pub fn is_active(layer: ObservabilityLayer) -> bool {
    LAYER_MANAGER
        .get()
        .map(|manager| manager.current_layer() >= layer)
        .unwrap_or(true)
}

pub fn set_layer(target: ObservabilityLayer, reason: Option<&str>) -> bool {
    LAYER_MANAGER
        .get()
        .map(|manager| manager.set_current(target, "manual", reason))
        .unwrap_or(false)
}

pub fn auto_downgrade(reason: &str) -> Option<ObservabilityLayer> {
    LAYER_MANAGER
        .get()
        .and_then(|manager| manager.auto_downgrade(reason))
}

pub fn handle_memory_pressure() -> Option<ObservabilityLayer> {
    auto_downgrade("memory_pressure")
}

pub fn override_layer_guards(min_residency: Duration, cooldown: Duration) {
    if let Some(manager) = LAYER_MANAGER.get() {
        manager.override_guards(min_residency, cooldown);
    }
}

pub fn resolved_state() -> Option<ResolvedLayerConfig> {
    LAYER_MANAGER.get().map(|manager| manager.snapshot())
}

struct LayerManager {
    registry: Arc<MetricRegistry>,
    current: AtomicU8,
    target: AtomicU8,
    max_allowed: AtomicU8,
    auto_downgrade: AtomicBool,
    min_residency_ms: AtomicU64,
    cooldown_ms: AtomicU64,
    last_transition: Mutex<Instant>,
    last_auto_downgrade: Mutex<Option<Instant>>,
}

impl LayerManager {
    fn new(
        registry: Arc<MetricRegistry>,
        resolved: &ResolvedLayerConfig,
        cfg: &ObservabilityConfig,
    ) -> Result<Self, MetricInitError> {
        let manager = Self {
            registry,
            current: AtomicU8::new(resolved.effective_layer.as_u8()),
            target: AtomicU8::new(resolved.target_layer.as_u8()),
            max_allowed: AtomicU8::new(resolved.max_allowed_layer.as_u8()),
            auto_downgrade: AtomicBool::new(cfg.auto_downgrade && resolved.basic_enabled),
            min_residency_ms: AtomicU64::new((cfg.min_layer_residency_secs as u64) * 1_000),
            cooldown_ms: AtomicU64::new((cfg.downgrade_cooldown_secs as u64) * 1_000),
            last_transition: Mutex::new(Instant::now()),
            last_auto_downgrade: Mutex::new(None),
        };
        manager.write_gauge(resolved.effective_layer)?;
        Ok(manager)
    }

    fn update_from_resolved(&self, resolved: &ResolvedLayerConfig, cfg: &ObservabilityConfig) {
        self.target
            .store(resolved.target_layer.as_u8(), Ordering::Relaxed);
        self.max_allowed
            .store(resolved.max_allowed_layer.as_u8(), Ordering::Relaxed);
        self.auto_downgrade.store(
            cfg.auto_downgrade && resolved.basic_enabled,
            Ordering::Relaxed,
        );
        self.min_residency_ms.store(
            (cfg.min_layer_residency_secs as u64) * 1_000,
            Ordering::Relaxed,
        );
        self.cooldown_ms.store(
            (cfg.downgrade_cooldown_secs as u64) * 1_000,
            Ordering::Relaxed,
        );

        let current = ObservabilityLayer::from_u8(self.current.load(Ordering::Relaxed));
        let clamped = self.clamp_layer(current);
        if clamped != current {
            let changed = self.set_current(clamped, "config", Some("constraint"));
            if changed {
                return;
            }
        }
        if resolved.effective_layer != current {
            self.set_current(resolved.effective_layer, "config", Some("target"));
        } else if let Err(err) = self.write_gauge(current) {
            warn!(target = "metrics", error = %err, "failed to update observability layer gauge");
        }
    }

    fn current_layer(&self) -> ObservabilityLayer {
        ObservabilityLayer::from_u8(self.current.load(Ordering::Relaxed))
    }

    fn set_current(
        &self,
        requested: ObservabilityLayer,
        initiator: &str,
        reason: Option<&str>,
    ) -> bool {
        let target = self.clamp_layer(requested);
        let current_raw = self.current.load(Ordering::Relaxed);
        if current_raw == target.as_u8() {
            return false;
        }
        self.current.store(target.as_u8(), Ordering::Relaxed);
        if let Err(err) = self.write_gauge(target) {
            warn!(
                target = "metrics",
                error = %err,
                "failed to write observability layer gauge during transition"
            );
        }
        if let Ok(mut guard) = self.last_transition.lock() {
            *guard = Instant::now();
        }
        let from = ObservabilityLayer::from_u8(current_raw);
        self.publish_change(from, target, initiator, reason);
        true
    }

    fn auto_downgrade(&self, reason: &str) -> Option<ObservabilityLayer> {
        if !self.auto_downgrade.load(Ordering::Relaxed) {
            return None;
        }
        let now = Instant::now();
        if let Ok(last_transition) = self.last_transition.lock() {
            let min_residency =
                Duration::from_millis(self.min_residency_ms.load(Ordering::Relaxed));
            if now.saturating_duration_since(*last_transition) < min_residency {
                return None;
            }
        }
        if let Ok(mut guard) = self.last_auto_downgrade.lock() {
            let cooldown = Duration::from_millis(self.cooldown_ms.load(Ordering::Relaxed));
            if let Some(last) = *guard {
                if now.saturating_duration_since(last) < cooldown {
                    return None;
                }
            }
            let current = self.current_layer();
            let Some(next) = current.next_lower() else {
                return None;
            };
            let changed = self.set_current(next, "auto-downgrade", Some(reason));
            if changed {
                *guard = Some(now);
                info!(
                    target = "metrics",
                    from = current.as_str(),
                    to = next.as_str(),
                    %reason,
                    "observability layer auto downgraded"
                );
                return Some(next);
            }
        }
        None
    }

    fn override_guards(&self, min_residency: Duration, cooldown: Duration) {
        self.min_residency_ms
            .store(min_residency.as_millis() as u64, Ordering::Relaxed);
        self.cooldown_ms
            .store(cooldown.as_millis() as u64, Ordering::Relaxed);
    }

    fn snapshot(&self) -> ResolvedLayerConfig {
        let current = self.current_layer();
        let target = ObservabilityLayer::from_u8(self.target.load(Ordering::Relaxed));
        let max_allowed = ObservabilityLayer::from_u8(self.max_allowed.load(Ordering::Relaxed));
        ResolvedLayerConfig {
            enabled: true,
            basic_enabled: current >= ObservabilityLayer::Basic,
            aggregate_enabled: current >= ObservabilityLayer::Aggregate,
            export_enabled: current >= ObservabilityLayer::Export,
            ui_enabled: current >= ObservabilityLayer::Ui,
            alerts_enabled: current >= ObservabilityLayer::Alerts,
            optimize_enabled: current == ObservabilityLayer::Optimize,
            target_layer: target,
            effective_layer: current,
            max_allowed_layer: max_allowed,
        }
    }

    fn clamp_layer(&self, requested: ObservabilityLayer) -> ObservabilityLayer {
        let target = ObservabilityLayer::from_u8(self.target.load(Ordering::Relaxed));
        let max_allowed = ObservabilityLayer::from_u8(self.max_allowed.load(Ordering::Relaxed));
        let mut layer = requested;
        if layer > target {
            layer = target;
        }
        if layer > max_allowed {
            layer = max_allowed;
        }
        layer
    }

    fn publish_change(
        &self,
        from: ObservabilityLayer,
        to: ObservabilityLayer,
        initiator: &str,
        reason: Option<&str>,
    ) {
        publish_global(Event::Strategy(StrategyEvent::ObservabilityLayerChanged {
            from: from.as_str().to_string(),
            to: to.as_str().to_string(),
            initiator: initiator.to_string(),
            reason: reason.map(|r| r.to_string()),
        }));
    }

    fn write_gauge(&self, layer: ObservabilityLayer) -> Result<(), MetricError> {
        self.registry
            .set_gauge(OBSERVABILITY_LAYER, &[], layer.as_u8() as u64)
    }
}
