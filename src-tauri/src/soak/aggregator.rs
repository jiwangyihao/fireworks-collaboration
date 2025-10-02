use crate::core::tasks::model::TaskState;
use crate::events::structured::{Event, StrategyEvent, TaskEvent};
use std::collections::HashMap;

use super::models::*;
use super::utils::compute_field_stats;

#[derive(Default)]
pub(crate) struct OperationStats {
    pub total: u64,
    pub completed: u64,
    pub failed: u64,
    pub canceled: u64,
}

impl OperationStats {
    pub fn record(&mut self, state: &TaskState) {
        self.total += 1;
        match state {
            TaskState::Completed => self.completed += 1,
            TaskState::Failed => self.failed += 1,
            TaskState::Canceled => self.canceled += 1,
            _ => {}
        }
    }
}

#[derive(Default)]
pub(crate) struct TimingSample {
    pub connect_ms: Option<u32>,
    pub tls_ms: Option<u32>,
    pub first_byte_ms: Option<u32>,
    pub total_ms: Option<u32>,
    pub used_fake: bool,
    pub fallback_stage: String,
    pub cert_fp_changed: bool,
}

#[derive(Default)]
pub(crate) struct TimingData {
    pub samples: Vec<TimingSample>,
    pub final_stage_counts: HashMap<String, u64>,
    pub used_fake_count: u64,
}

#[derive(Default)]
pub(crate) struct FallbackStats {
    pub counts: HashMap<String, u64>,
    pub fake_to_real: u64,
    pub real_to_default: u64,
}

#[derive(Default)]
pub(crate) struct AutoDisableStats {
    pub triggered: u64,
    pub recovered: u64,
}

#[derive(Default)]
pub struct IpPoolStats {
    pub selection_total: u64,
    pub selection_by_strategy: HashMap<String, u64>,
    pub refresh_total: u64,
    pub refresh_success: u64,
    pub refresh_failure: u64,
}

#[derive(Default)]
pub struct ProxyStats {
    pub fallback_count: u64,
    pub recovered_count: u64,
    pub health_check_latencies: Vec<u32>,
    pub health_check_success: u64,
    pub health_check_failure: u64,
    pub system_proxy_detect_total: u64,
    pub system_proxy_detect_success: u64,
}

/// Aggregates soak test data from task execution and structured events.
#[derive(Default)]
pub struct SoakAggregator {
    operations: HashMap<String, OperationStats>,
    timing: HashMap<String, TimingData>,
    fallback: FallbackStats,
    auto_disable: AutoDisableStats,
    cert_fp_events: u64,
    pub ip_pool: IpPoolStats,
    pub proxy: ProxyStats,
    pub expected_iterations: u32,
}

impl SoakAggregator {
    pub fn new(expected_iterations: u32) -> Self {
        Self {
            expected_iterations,
            ..Default::default()
        }
    }

    pub fn record_task(&mut self, kind: &str, state: TaskState) {
        self.operations
            .entry(kind.to_string())
            .or_default()
            .record(&state);
    }

    pub fn process_events(&mut self, events: Vec<Event>) {
        for evt in events {
            match evt {
                Event::Task(TaskEvent::Completed { .. })
                | Event::Task(TaskEvent::Failed { .. }) => {
                    // Already tracked via record_task
                }
                Event::Strategy(StrategyEvent::AdaptiveTlsTiming {
                    kind,
                    used_fake_sni,
                    fallback_stage,
                    connect_ms,
                    tls_ms,
                    first_byte_ms,
                    total_ms,
                    cert_fp_changed,
                    ..
                }) => {
                    self.process_timing_event(
                        kind,
                        used_fake_sni,
                        fallback_stage,
                        connect_ms,
                        tls_ms,
                        first_byte_ms,
                        total_ms,
                        cert_fp_changed,
                    );
                }
                Event::Strategy(StrategyEvent::AdaptiveTlsFallback {
                    kind,
                    from,
                    to,
                    reason,
                    ..
                }) => {
                    self.process_fallback_event(kind, from, to, reason);
                }
                Event::Strategy(StrategyEvent::AdaptiveTlsAutoDisable { enabled, .. }) => {
                    self.process_auto_disable_event(enabled);
                }
                Event::Strategy(StrategyEvent::CertFingerprintChanged { .. }) => {
                    self.cert_fp_events += 1;
                }
                Event::Strategy(StrategyEvent::IpPoolSelection { strategy, .. }) => {
                    self.process_ip_pool_selection(strategy);
                }
                Event::Strategy(StrategyEvent::IpPoolRefresh { success, .. }) => {
                    self.process_ip_pool_refresh(success);
                }
                Event::Strategy(StrategyEvent::ProxyFallback { .. }) => {
                    self.proxy.fallback_count += 1;
                }
                Event::Strategy(StrategyEvent::ProxyRecovered { .. }) => {
                    self.proxy.recovered_count += 1;
                }
                Event::Strategy(StrategyEvent::ProxyHealthCheck {
                    success,
                    latency_ms,
                    ..
                }) => {
                    self.process_proxy_health_check(success, latency_ms);
                }
                Event::Strategy(StrategyEvent::SystemProxyDetected { success, .. }) => {
                    self.process_system_proxy_detect(success);
                }
                _ => {}
            }
        }
    }

    fn process_timing_event(
        &mut self,
        kind: String,
        used_fake_sni: bool,
        fallback_stage: String,
        connect_ms: Option<u32>,
        tls_ms: Option<u32>,
        first_byte_ms: Option<u32>,
        total_ms: Option<u32>,
        cert_fp_changed: bool,
    ) {
        let entry = self.timing.entry(kind).or_default();
        entry.used_fake_count += used_fake_sni as u64;
        *entry
            .final_stage_counts
            .entry(fallback_stage.clone())
            .or_default() += 1;
        entry.samples.push(TimingSample {
            connect_ms,
            tls_ms,
            first_byte_ms,
            total_ms,
            used_fake: used_fake_sni,
            fallback_stage,
            cert_fp_changed,
        });
    }

    fn process_fallback_event(&mut self, kind: String, from: String, to: String, reason: String) {
        let key = format!("{}:{}->{}:{}", kind, from, to, reason);
        *self.fallback.counts.entry(key).or_default() += 1;
        if from == "Fake" && to == "Real" {
            self.fallback.fake_to_real += 1;
        }
        if from == "Real" && to == "Default" {
            self.fallback.real_to_default += 1;
        }
    }

    fn process_auto_disable_event(&mut self, enabled: bool) {
        if enabled {
            self.auto_disable.triggered += 1;
        } else {
            self.auto_disable.recovered += 1;
        }
    }

    fn process_ip_pool_selection(&mut self, strategy: String) {
        self.ip_pool.selection_total += 1;
        *self
            .ip_pool
            .selection_by_strategy
            .entry(strategy)
            .or_default() += 1;
    }

    fn process_ip_pool_refresh(&mut self, success: bool) {
        self.ip_pool.refresh_total += 1;
        if success {
            self.ip_pool.refresh_success += 1;
        } else {
            self.ip_pool.refresh_failure += 1;
        }
    }

    fn process_proxy_health_check(&mut self, success: bool, latency_ms: Option<u32>) {
        if success {
            self.proxy.health_check_success += 1;
        } else {
            self.proxy.health_check_failure += 1;
        }
        if let Some(latency) = latency_ms {
            self.proxy.health_check_latencies.push(latency);
        }
    }

    fn process_system_proxy_detect(&mut self, success: bool) {
        self.proxy.system_proxy_detect_total += 1;
        if success {
            self.proxy.system_proxy_detect_success += 1;
        }
    }

    pub fn into_report(
        self,
        started_unix: u64,
        finished_unix: u64,
        duration_secs: u64,
        options: SoakOptionsSnapshot,
    ) -> SoakReport {
        let (operations_summary, totals) = self.build_operations_summary();
        let (timing_summary, total_fake_attempts, cert_fp_changed_samples) =
            self.build_timing_summary();

        let success_rate = if totals.total_operations > 0 {
            totals.completed as f64 / totals.total_operations as f64
        } else {
            1.0
        };

        let fallback_ratio = if total_fake_attempts > 0 {
            self.fallback.fake_to_real as f64 / total_fake_attempts as f64
        } else {
            0.0
        };

        let ip_pool_summary = self.build_ip_pool_summary();
        let proxy_summary = self.build_proxy_summary();
        let threshold_summary = self.build_threshold_summary(
            &options.thresholds,
            success_rate,
            fallback_ratio,
            &ip_pool_summary,
        );

        SoakReport {
            started_unix,
            finished_unix,
            duration_secs,
            options,
            iterations: self.expected_iterations,
            operations: operations_summary,
            timing: timing_summary,
            fallback: FallbackSummary {
                counts: self.fallback.counts,
                fake_to_real: self.fallback.fake_to_real,
                real_to_default: self.fallback.real_to_default,
            },
            auto_disable: AutoDisableSummary {
                triggered: self.auto_disable.triggered,
                recovered: self.auto_disable.recovered,
            },
            cert_fp_events: self.cert_fp_events + cert_fp_changed_samples,
            ip_pool: ip_pool_summary,
            proxy: proxy_summary,
            totals,
            thresholds: threshold_summary,
            comparison: None,
        }
    }

    fn build_operations_summary(&self) -> (HashMap<String, OperationSummary>, TotalsSummary) {
        let mut operations_summary = HashMap::new();
        let mut total_operations = 0u64;
        let mut total_completed = 0u64;
        let mut total_failed = 0u64;
        let mut total_canceled = 0u64;

        for (kind, stats) in self.operations.iter() {
            let success_rate = if stats.total > 0 {
                stats.completed as f64 / stats.total as f64
            } else {
                1.0
            };
            total_operations += stats.total;
            total_completed += stats.completed;
            total_failed += stats.failed;
            total_canceled += stats.canceled;
            operations_summary.insert(
                kind.clone(),
                OperationSummary {
                    total: stats.total,
                    completed: stats.completed,
                    failed: stats.failed,
                    canceled: stats.canceled,
                    success_rate,
                },
            );
        }

        let totals = TotalsSummary {
            total_operations,
            completed: total_completed,
            failed: total_failed,
            canceled: total_canceled,
        };

        (operations_summary, totals)
    }

    fn build_timing_summary(&self) -> (HashMap<String, TimingSummary>, u64, u64) {
        let mut timing_summary = HashMap::new();
        let mut total_fake_attempts = 0u64;
        let mut cert_fp_changed_samples = 0u64;

        for (kind, data) in self.timing.iter() {
            let connect_vals: Vec<u32> =
                data.samples.iter().filter_map(|s| s.connect_ms).collect();
            let tls_vals: Vec<u32> = data.samples.iter().filter_map(|s| s.tls_ms).collect();
            let first_byte_vals: Vec<u32> = data
                .samples
                .iter()
                .filter_map(|s| s.first_byte_ms)
                .collect();
            let total_vals: Vec<u32> = data.samples.iter().filter_map(|s| s.total_ms).collect();
            let changed_count = data.samples.iter().filter(|s| s.cert_fp_changed).count() as u64;

            cert_fp_changed_samples += changed_count;
            total_fake_attempts += data.used_fake_count;

            timing_summary.insert(
                kind.clone(),
                TimingSummary {
                    samples: data.samples.len(),
                    used_fake: data.used_fake_count as usize,
                    cert_fp_changed_samples: changed_count as usize,
                    final_stage_counts: data.final_stage_counts.clone(),
                    connect_ms: compute_field_stats(&connect_vals),
                    tls_ms: compute_field_stats(&tls_vals),
                    first_byte_ms: compute_field_stats(&first_byte_vals),
                    total_ms: compute_field_stats(&total_vals),
                },
            );
        }

        (timing_summary, total_fake_attempts, cert_fp_changed_samples)
    }

    fn build_ip_pool_summary(&self) -> IpPoolSummary {
        let refresh_success_rate = if self.ip_pool.refresh_total > 0 {
            self.ip_pool.refresh_success as f64 / self.ip_pool.refresh_total as f64
        } else {
            1.0
        };

        IpPoolSummary {
            selection_total: self.ip_pool.selection_total,
            selection_by_strategy: self.ip_pool.selection_by_strategy.clone(),
            refresh_total: self.ip_pool.refresh_total,
            refresh_success: self.ip_pool.refresh_success,
            refresh_failure: self.ip_pool.refresh_failure,
            refresh_success_rate,
        }
    }

    fn build_proxy_summary(&self) -> ProxySummary {
        let health_check_total = self.proxy.health_check_success + self.proxy.health_check_failure;
        let avg_health_check_latency_ms = if !self.proxy.health_check_latencies.is_empty() {
            let sum: u64 = self
                .proxy
                .health_check_latencies
                .iter()
                .map(|&v| v as u64)
                .sum();
            Some(sum as f64 / self.proxy.health_check_latencies.len() as f64)
        } else {
            None
        };
        let system_proxy_detect_success_rate = if self.proxy.system_proxy_detect_total > 0 {
            self.proxy.system_proxy_detect_success as f64
                / self.proxy.system_proxy_detect_total as f64
        } else {
            0.0
        };

        ProxySummary {
            fallback_count: self.proxy.fallback_count,
            recovered_count: self.proxy.recovered_count,
            health_check_total,
            health_check_success: self.proxy.health_check_success,
            avg_health_check_latency_ms,
            system_proxy_detect_total: self.proxy.system_proxy_detect_total,
            system_proxy_detect_success: self.proxy.system_proxy_detect_success,
            system_proxy_detect_success_rate,
        }
    }

    fn build_threshold_summary(
        &self,
        thresholds_cfg: &SoakThresholds,
        success_rate: f64,
        fallback_ratio: f64,
        ip_pool_summary: &IpPoolSummary,
    ) -> ThresholdSummary {
        let ip_pool_threshold = if self.ip_pool.refresh_total > 0 {
            Some(ThresholdCheck::at_least(
                ip_pool_summary.refresh_success_rate,
                thresholds_cfg.min_ip_pool_refresh_success_rate,
            ))
        } else {
            None
        };

        let auto_disable_threshold = Some(ThresholdCheck::at_most(
            self.auto_disable.triggered as f64,
            thresholds_cfg.max_auto_disable_triggered as f64,
        ));

        ThresholdSummary::new(
            ThresholdCheck::at_least(success_rate, thresholds_cfg.min_success_rate),
            ThresholdCheck::at_most(fallback_ratio, thresholds_cfg.max_fake_fallback_rate),
            ip_pool_threshold,
            auto_disable_threshold,
            None,
        )
    }
}
