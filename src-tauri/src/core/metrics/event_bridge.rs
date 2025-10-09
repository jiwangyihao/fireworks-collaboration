use std::sync::Arc;
use std::time::Instant;

use dashmap::{mapref::entry::Entry, DashMap};

use crate::events::structured::{Event, EventBus, MetricAlertState, StrategyEvent, TaskEvent};

use super::descriptors::*;
use super::registry::MetricRegistry;
use super::runtime::{observe_histogram, record_counter, SampleKind};

const STATE_COMPLETED: &str = "completed";
const STATE_FAILED: &str = "failed";
const STATE_CANCELED: &str = "canceled";
const OUTCOME_OK: &str = "ok";
const OUTCOME_FAIL: &str = "fail";

pub struct EventMetricsBridge {
    _registry: Arc<MetricRegistry>,
    tasks: DashMap<String, TaskInfo>,
}

impl EventMetricsBridge {
    pub fn new(registry: Arc<MetricRegistry>) -> Self {
        Self {
            _registry: registry,
            tasks: DashMap::new(),
        }
    }

    fn handle_task_event(&self, event: TaskEvent) {
        match event {
            TaskEvent::Started { id, kind } => {
                let key = id;
                let is_git = kind.starts_with("Git");
                match self.tasks.entry(key.clone()) {
                    Entry::Occupied(mut occ) => {
                        let info = occ.get_mut();
                        info.kind = kind.clone();
                        info.is_git = is_git;
                        info.started_at = Some(Instant::now());
                        info.completed_recorded = false;
                        info.failed_recorded = false;
                        info.canceled_recorded = false;
                    }
                    Entry::Vacant(vacant) => {
                        vacant.insert(TaskInfo {
                            kind,
                            is_git,
                            started_at: Some(Instant::now()),
                            completed_recorded: false,
                            failed_recorded: false,
                            canceled_recorded: false,
                        });
                    }
                }
            }
            TaskEvent::Completed { id } => {
                if let Some(mut entry) = self.tasks.get_mut(&id) {
                    let info = entry.value_mut();
                    if info.is_git && !info.completed_recorded {
                        let kind = info.kind.clone();
                        let kind_label = kind.as_str();
                        self.record_git_task(kind_label, STATE_COMPLETED);
                        if let Some(duration) = info.started_at.map(|start| elapsed_ms(start)) {
                            self.observe_git_duration(kind_label, duration);
                        }
                        info.completed_recorded = true;
                    }
                }
            }
            TaskEvent::Failed { id, category, .. } => {
                if let Some(mut entry) = self.tasks.get_mut(&id) {
                    let info = entry.value_mut();
                    if info.is_git && !info.failed_recorded {
                        let kind = info.kind.clone();
                        let kind_label = kind.as_str();
                        self.record_git_task(kind_label, STATE_FAILED);
                        if let Some(duration) = info.started_at.map(|start| elapsed_ms(start)) {
                            self.observe_git_duration(kind_label, duration);
                        }
                        let cat_value = sanitize_label_value(&category);
                        self.record_retry(kind_label, &cat_value);
                        info.failed_recorded = true;
                    }
                }
            }
            TaskEvent::Canceled { id } => {
                if let Some(mut entry) = self.tasks.get_mut(&id) {
                    let info = entry.value_mut();
                    if info.is_git && !info.canceled_recorded {
                        let kind = info.kind.clone();
                        let kind_label = kind.as_str();
                        self.record_git_task(kind_label, STATE_CANCELED);
                        if let Some(duration) = info.started_at.map(|start| elapsed_ms(start)) {
                            self.observe_git_duration(kind_label, duration);
                        }
                        info.canceled_recorded = true;
                    }
                }
            }
        }
    }

    fn handle_strategy_event(&self, event: StrategyEvent) {
        match event {
            StrategyEvent::AdaptiveTlsTiming {
                used_fake_sni,
                tls_ms,
                total_ms,
                ip_source,
                ip_latency_ms,
                ..
            } => {
                if let Some(value) = tls_ms
                    .map(|v| v as f64)
                    .or_else(|| total_ms.map(|v| v as f64))
                {
                    let sni_strategy = if used_fake_sni { "fake" } else { "real" };
                    let outcome = if tls_ms.is_some() && total_ms.is_some() {
                        OUTCOME_OK
                    } else {
                        OUTCOME_FAIL
                    };
                    self.observe_tls_handshake(sni_strategy, outcome, value);
                }
                if let Some(latency) = ip_latency_ms {
                    let label = ip_source
                        .and_then(|s| s.split(',').next().map(|value| sanitize_label_value(value)))
                        .unwrap_or_else(|| "unknown".to_string());
                    self.observe_ip_latency(&label, latency as f64);
                }
            }
            StrategyEvent::AdaptiveTlsFallback { from, reason, .. } => {
                let stage = sanitize_label_value(&reason);
                let from_label = sanitize_label_value(&from);
                self.record_http_strategy_fallback(&stage, &from_label);
            }
            StrategyEvent::IpPoolSelection {
                strategy,
                source,
                latency_ms,
                ..
            } => {
                tracing::info!(
                    target = "metrics",
                    strategy = %strategy,
                    source = ?source,
                    latency_ms = ?latency_ms,
                    "processing IpPoolSelection event"
                );
                let strategy_label = sanitize_label_value(&strategy);
                let outcome_label = if source.as_ref().and_then(|s| s.split(',').next()).is_some() {
                    "success"
                } else {
                    "fail"
                };
                self.record_ip_selection(&strategy_label, outcome_label);
                if let Some(latency) = latency_ms {
                    if let Some(src) = source.as_ref().and_then(|s| s.split(',').next()) {
                        let src_label = sanitize_label_value(src);
                        self.observe_ip_latency(&src_label, latency as f64);
                    }
                }
            }
            StrategyEvent::IpPoolRefresh {
                success, reason, ..
            } => {
                let reason_label = sanitize_label_value(&reason);
                let success_label = if success { "true" } else { "false" };
                self.record_ip_refresh(&reason_label, success_label);
                // Log and expose current counter for quick verification
                if let Some(val) = self._registry.get_counter(
                    IP_POOL_REFRESH_TOTAL,
                    &[
                        ("reason", reason_label.as_str()),
                        ("success", success_label),
                    ],
                ) {
                    tracing::info!(target = "metrics", reason = %reason_label, success = %success_label, count = val, "ip_pool_refresh_total incremented");
                }
            }
            StrategyEvent::IpPoolAutoDisable { reason, .. } => {
                let reason_label = sanitize_label_value(&reason);
                self.record_ip_auto_disable(&reason_label);
            }
            StrategyEvent::IpPoolIpTripped { reason, .. } => {
                let reason_label = sanitize_label_value(&reason);
                self.record_circuit_trip(&reason_label);
            }
            StrategyEvent::IpPoolIpRecovered { .. } => {
                self.record_circuit_recover();
            }
            StrategyEvent::ProxyFallback { reason, .. } => {
                let reason_label = sanitize_label_value(&reason);
                self.record_proxy_fallback(&reason_label);
            }
            StrategyEvent::MetricAlert {
                severity, state, ..
            } => {
                if matches!(state, MetricAlertState::Firing | MetricAlertState::Active) {
                    let severity_label = sanitize_label_value(&severity);
                    self.record_alert(&severity_label);
                }
            }
            _ => {}
        }
    }

    fn record_git_task(&self, kind: &str, state: &str) {
        let labels = [("kind", kind), ("state", state)];
        record_counter(GIT_TASKS_TOTAL, &labels, 1);
    }

    fn observe_git_duration(&self, kind: &str, value: f64) {
        let labels = [("kind", kind)];
        observe_histogram(GIT_TASK_DURATION_MS, &labels, value, SampleKind::None);
    }

    fn record_retry(&self, kind: &str, category: &str) {
        let labels = [("kind", kind), ("category", category)];
        record_counter(GIT_RETRY_TOTAL, &labels, 1);
    }

    fn observe_tls_handshake(&self, strategy: &str, outcome: &str, value: f64) {
        let labels = [("sni_strategy", strategy), ("outcome", outcome)];
        observe_histogram(TLS_HANDSHAKE_MS, &labels, value, SampleKind::TlsHandshake);
    }

    fn observe_ip_latency(&self, source: &str, value: f64) {
        let labels = [("source", source)];
        observe_histogram(IP_POOL_LATENCY_MS, &labels, value, SampleKind::None);
    }

    fn record_ip_selection(&self, strategy: &str, outcome: &str) {
        let labels = [("strategy", strategy), ("outcome", outcome)];
        record_counter(IP_POOL_SELECTION_TOTAL, &labels, 1);
    }

    fn record_ip_refresh(&self, reason: &str, success: &str) {
        let labels = [("reason", reason), ("success", success)];
        record_counter(IP_POOL_REFRESH_TOTAL, &labels, 1);
    }

    fn record_ip_auto_disable(&self, reason: &str) {
        let labels = [("reason", reason)];
        record_counter(IP_POOL_AUTO_DISABLE_TOTAL, &labels, 1);
    }

    fn record_circuit_trip(&self, reason: &str) {
        let labels = [("reason", reason)];
        record_counter(CIRCUIT_BREAKER_TRIP_TOTAL, &labels, 1);
    }

    fn record_circuit_recover(&self) {
        record_counter(CIRCUIT_BREAKER_RECOVER_TOTAL, &[], 1);
    }

    fn record_proxy_fallback(&self, reason: &str) {
        let labels = [("reason", reason)];
        record_counter(PROXY_FALLBACK_TOTAL, &labels, 1);
    }

    fn record_http_strategy_fallback(&self, stage: &str, from: &str) {
        let labels = [("stage", stage), ("from", from)];
        record_counter(HTTP_STRATEGY_FALLBACK_TOTAL, &labels, 1);
    }

    fn record_alert(&self, severity: &str) {
        let labels = [("severity", severity)];
        record_counter(ALERTS_FIRED_TOTAL, &labels, 1);
    }
}

impl EventBus for EventMetricsBridge {
    fn publish(&self, evt: Event) {
        match evt {
            Event::Task(task_evt) => self.handle_task_event(task_evt),
            Event::Strategy(strategy_evt) => self.handle_strategy_event(strategy_evt),
            _ => {}
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

struct TaskInfo {
    kind: String,
    is_git: bool,
    started_at: Option<Instant>,
    completed_recorded: bool,
    failed_recorded: bool,
    canceled_recorded: bool,
}

fn sanitize_label_value(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut last_was_sep = false;
    for ch in input.chars() {
        let mapped = if ch.is_ascii_alphanumeric() {
            last_was_sep = false;
            ch.to_ascii_lowercase()
        } else {
            if !last_was_sep {
                last_was_sep = true;
                '_'
            } else {
                continue;
            }
        };
        out.push(mapped);
    }
    if out.is_empty() {
        return "unknown".to_string();
    }
    let trimmed = out.trim_matches('_').to_string();
    if trimmed.is_empty() {
        "unknown".to_string()
    } else {
        trimmed
    }
}

fn elapsed_ms(start: Instant) -> f64 {
    start.elapsed().as_secs_f64() * 1_000.0
}
