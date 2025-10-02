use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Readiness thresholds enforced by the soak runner.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoakThresholds {
    /// Minimum overall task success rate.
    pub min_success_rate: f64,
    /// Maximum allowed Fake→Real fallback ratio.
    pub max_fake_fallback_rate: f64,
    /// Minimum required IP pool refresh success rate when IP 池启用.
    pub min_ip_pool_refresh_success_rate: f64,
    /// Maximum allowed auto-disable triggers during the soak window.
    pub max_auto_disable_triggered: u64,
    /// Minimum required GitClone total latency improvement (p50) versus基线（若提供）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_latency_improvement: Option<f64>,
}

impl Default for SoakThresholds {
    fn default() -> Self {
        Self {
            min_success_rate: 0.99,
            max_fake_fallback_rate: 0.05,
            min_ip_pool_refresh_success_rate: 0.85,
            max_auto_disable_triggered: 0,
            min_latency_improvement: Some(0.15),
        }
    }
}

/// Options controlling how the soak runner behaves.
#[derive(Debug, Clone)]
pub struct SoakOptions {
    /// Number of iterations to run (each iteration exercises push → fetch → clone).
    pub iterations: u32,
    /// Whether to keep individual clone directories after the run.
    pub keep_clones: bool,
    /// Destination path of the generated JSON report.
    pub report_path: PathBuf,
    /// Optional workspace root. When omitted a temporary directory is created under the OS temp dir.
    pub base_dir: Option<PathBuf>,
    /// Optional baseline report to compare the new results against.
    pub baseline_report: Option<PathBuf>,
    /// Readiness thresholds enforced for this soak run.
    pub thresholds: SoakThresholds,
}

impl Default for SoakOptions {
    fn default() -> Self {
        Self {
            iterations: 10,
            keep_clones: false,
            report_path: PathBuf::from("soak-report.json"),
            base_dir: None,
            baseline_report: None,
            thresholds: SoakThresholds::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoakOptionsSnapshot {
    pub iterations: u32,
    pub keep_clones: bool,
    pub report_path: String,
    pub workspace_dir: String,
    pub baseline_report: Option<String>,
    #[serde(default)]
    pub thresholds: SoakThresholds,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoakReport {
    pub started_unix: u64,
    pub finished_unix: u64,
    pub duration_secs: u64,
    pub options: SoakOptionsSnapshot,
    pub iterations: u32,
    pub operations: HashMap<String, OperationSummary>,
    pub timing: HashMap<String, TimingSummary>,
    pub fallback: FallbackSummary,
    pub auto_disable: AutoDisableSummary,
    pub cert_fp_events: u64,
    pub ip_pool: IpPoolSummary,
    pub proxy: ProxySummary,
    pub totals: TotalsSummary,
    pub thresholds: ThresholdSummary,
    pub comparison: Option<ComparisonSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationSummary {
    pub total: u64,
    pub completed: u64,
    pub failed: u64,
    pub canceled: u64,
    pub success_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimingSummary {
    pub samples: usize,
    pub used_fake: usize,
    pub cert_fp_changed_samples: usize,
    pub final_stage_counts: HashMap<String, u64>,
    pub connect_ms: Option<FieldStats>,
    pub tls_ms: Option<FieldStats>,
    pub first_byte_ms: Option<FieldStats>,
    pub total_ms: Option<FieldStats>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldStats {
    pub count: usize,
    pub min: u32,
    pub max: u32,
    pub avg: f64,
    pub p50: u32,
    pub p95: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FallbackSummary {
    pub counts: HashMap<String, u64>,
    pub fake_to_real: u64,
    pub real_to_default: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoDisableSummary {
    pub triggered: u64,
    pub recovered: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpPoolSummary {
    pub selection_total: u64,
    pub selection_by_strategy: HashMap<String, u64>,
    pub refresh_total: u64,
    pub refresh_success: u64,
    pub refresh_failure: u64,
    pub refresh_success_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxySummary {
    pub fallback_count: u64,
    pub recovered_count: u64,
    pub health_check_total: u64,
    pub health_check_success: u64,
    pub avg_health_check_latency_ms: Option<f64>,
    pub system_proxy_detect_total: u64,
    pub system_proxy_detect_success: u64,
    pub system_proxy_detect_success_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TotalsSummary {
    pub total_operations: u64,
    pub completed: u64,
    pub failed: u64,
    pub canceled: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThresholdSummary {
    pub success_rate: ThresholdCheck,
    pub fake_fallback_rate: ThresholdCheck,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ip_pool_refresh_success_rate: Option<ThresholdCheck>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub auto_disable_triggered: Option<ThresholdCheck>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub latency_improvement: Option<ThresholdCheck>,
    #[serde(default)]
    pub ready: bool,
    #[serde(default)]
    pub failing_checks: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThresholdCheck {
    pub pass: bool,
    pub actual: f64,
    pub expected: f64,
    pub comparator: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComparisonSummary {
    pub baseline_path: String,
    pub success_rate_delta: f64,
    pub fake_fallback_rate_delta: f64,
    pub cert_fp_events_delta: i64,
    pub auto_disable_triggered_delta: i64,
    pub auto_disable_recovered_delta: i64,
    pub regression_flags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub git_clone_total_p50_improvement: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub git_clone_total_p50_current: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub git_clone_total_p50_baseline: Option<f64>,
}

impl ThresholdCheck {
    pub fn at_least(actual: f64, expected: f64) -> Self {
        Self {
            pass: actual >= expected,
            actual,
            expected,
            comparator: ">=".to_string(),
            details: None,
        }
    }

    pub fn at_most(actual: f64, expected: f64) -> Self {
        Self {
            pass: actual <= expected,
            actual,
            expected,
            comparator: "<=".to_string(),
            details: None,
        }
    }

    pub fn not_applicable(expected: f64, comparator: &str, reason: impl Into<String>) -> Self {
        Self {
            pass: false,
            actual: 0.0,
            expected,
            comparator: comparator.to_string(),
            details: Some(reason.into()),
        }
    }
}

impl ThresholdSummary {
    pub fn new(
        success_rate: ThresholdCheck,
        fake_fallback_rate: ThresholdCheck,
        ip_pool_refresh_success_rate: Option<ThresholdCheck>,
        auto_disable_triggered: Option<ThresholdCheck>,
        latency_improvement: Option<ThresholdCheck>,
    ) -> Self {
        let mut summary = Self {
            success_rate,
            fake_fallback_rate,
            ip_pool_refresh_success_rate,
            auto_disable_triggered,
            latency_improvement,
            ready: false,
            failing_checks: Vec::new(),
        };
        summary.recompute();
        summary
    }

    pub fn set_latency_improvement(&mut self, check: ThresholdCheck) {
        self.latency_improvement = Some(check);
        self.recompute();
    }

    fn recompute(&mut self) {
        let mut failing = Vec::new();
        if !self.success_rate.pass {
            failing.push("success_rate".to_string());
        }
        if !self.fake_fallback_rate.pass {
            failing.push("fake_fallback_rate".to_string());
        }
        if let Some(check) = &self.ip_pool_refresh_success_rate {
            if !check.pass {
                failing.push("ip_pool_refresh_success_rate".to_string());
            }
        }
        if let Some(check) = &self.auto_disable_triggered {
            if !check.pass {
                failing.push("auto_disable_triggered".to_string());
            }
        }
        if let Some(check) = &self.latency_improvement {
            if !check.pass {
                failing.push("latency_improvement".to_string());
            }
        }
        self.ready = failing.is_empty();
        self.failing_checks = failing;
    }
}

/// Build comparison summary between baseline and current soak reports.
pub fn build_comparison_summary(
    baseline_path: &Path,
    baseline: &SoakReport,
    current: &SoakReport,
) -> ComparisonSummary {
    let success_rate_delta =
        current.thresholds.success_rate.actual - baseline.thresholds.success_rate.actual;
    let fake_fallback_rate_delta = current.thresholds.fake_fallback_rate.actual
        - baseline.thresholds.fake_fallback_rate.actual;
    let cert_fp_events_delta = current.cert_fp_events as i64 - baseline.cert_fp_events as i64;
    let auto_disable_triggered_delta =
        current.auto_disable.triggered as i64 - baseline.auto_disable.triggered as i64;
    let auto_disable_recovered_delta =
        current.auto_disable.recovered as i64 - baseline.auto_disable.recovered as i64;

    let mut regression_flags = Vec::new();
    if baseline.thresholds.success_rate.pass && !current.thresholds.success_rate.pass {
        regression_flags.push("success_rate.pass_regressed".to_string());
    }
    if success_rate_delta < -0.0001 {
        regression_flags.push(format!("success_rate.decreased({:.4})", success_rate_delta));
    }
    if baseline.thresholds.fake_fallback_rate.pass && !current.thresholds.fake_fallback_rate.pass {
        regression_flags.push("fake_fallback_rate.pass_regressed".to_string());
    }
    if fake_fallback_rate_delta > 0.0001 {
        regression_flags.push(format!(
            "fake_fallback_rate.increased({:.4})",
            fake_fallback_rate_delta
        ));
    }
    if auto_disable_triggered_delta > 0 {
        regression_flags.push(format!(
            "auto_disable.triggered_increase({})",
            auto_disable_triggered_delta
        ));
    }

    let git_clone_total_p50_baseline = baseline
        .timing
        .get("GitClone")
        .and_then(|summary| summary.total_ms.as_ref())
        .map(|stats| stats.p50 as f64);
    let git_clone_total_p50_current = current
        .timing
        .get("GitClone")
        .and_then(|summary| summary.total_ms.as_ref())
        .map(|stats| stats.p50 as f64);
    let git_clone_total_p50_improvement = git_clone_total_p50_baseline.and_then(|base| {
        if base <= 0.0 {
            None
        } else {
            git_clone_total_p50_current.map(|curr| (base - curr) / base)
        }
    });
    if let (Some(expected), Some(improvement)) = (
        current.options.thresholds.min_latency_improvement,
        git_clone_total_p50_improvement,
    ) {
        if improvement + 1e-6 < expected {
            regression_flags.push(format!("latency_improvement.decreased({:.4})", improvement));
        }
    }

    ComparisonSummary {
        baseline_path: baseline_path.display().to_string(),
        success_rate_delta,
        fake_fallback_rate_delta,
        cert_fp_events_delta,
        auto_disable_triggered_delta,
        auto_disable_recovered_delta,
        regression_flags,
        git_clone_total_p50_improvement,
        git_clone_total_p50_current,
        git_clone_total_p50_baseline,
    }
}
