use crate::core::config::loader;
use crate::core::git::default_impl::{add, commit, init, push};
use crate::core::tasks::model::TaskState;
use crate::core::tasks::{TaskKind, TaskRegistry};
use crate::events::structured::{self, Event, EventBusAny, MemoryEventBus, StrategyEvent, TaskEvent};
use anyhow::{anyhow, ensure, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::runtime::{Builder, Runtime};
use uuid::Uuid;

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
}

impl Default for SoakOptions {
    fn default() -> Self {
        Self {
            iterations: 10,
            keep_clones: false,
            report_path: PathBuf::from("soak-report.json"),
            base_dir: None,
            baseline_report: None,
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
    pub totals: TotalsSummary,
    pub thresholds: ThresholdSummary,
    pub comparison: Option<ComparisonSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationSummary {
    total: u64,
    completed: u64,
    failed: u64,
    canceled: u64,
    success_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimingSummary {
    samples: usize,
    used_fake: usize,
    cert_fp_changed_samples: usize,
    final_stage_counts: HashMap<String, u64>,
    connect_ms: Option<FieldStats>,
    tls_ms: Option<FieldStats>,
    first_byte_ms: Option<FieldStats>,
    total_ms: Option<FieldStats>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldStats {
    count: usize,
    min: u32,
    max: u32,
    avg: f64,
    p50: u32,
    p95: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FallbackSummary {
    counts: HashMap<String, u64>,
    fake_to_real: u64,
    real_to_default: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoDisableSummary {
    triggered: u64,
    recovered: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TotalsSummary {
    total_operations: u64,
    completed: u64,
    failed: u64,
    canceled: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThresholdSummary {
    success_rate: ThresholdCheck,
    fake_fallback_rate: ThresholdCheck,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThresholdCheck {
    pass: bool,
    actual: f64,
    expected: f64,
    comparator: String,
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
}


impl ThresholdCheck {
    fn at_least(actual: f64, expected: f64) -> Self {
        Self {
            pass: actual >= expected,
            actual,
            expected,
            comparator: ">=".to_string(),
        }
    }

    fn at_most(actual: f64, expected: f64) -> Self {
        Self {
            pass: actual <= expected,
            actual,
            expected,
            comparator: "<=".to_string(),
        }
    }
}

fn build_comparison_summary(
    baseline_path: &Path,
    baseline: &SoakReport,
    current: &SoakReport,
) -> ComparisonSummary {
    let success_rate_delta =
        current.thresholds.success_rate.actual - baseline.thresholds.success_rate.actual;
    let fake_fallback_rate_delta = current.thresholds.fake_fallback_rate.actual
        - baseline.thresholds.fake_fallback_rate.actual;
    let cert_fp_events_delta = current.cert_fp_events as i64 - baseline.cert_fp_events as i64;
    let auto_disable_triggered_delta = current.auto_disable.triggered as i64
        - baseline.auto_disable.triggered as i64;
    let auto_disable_recovered_delta = current.auto_disable.recovered as i64
        - baseline.auto_disable.recovered as i64;

    let mut regression_flags = Vec::new();
    if baseline.thresholds.success_rate.pass && !current.thresholds.success_rate.pass {
        regression_flags.push("success_rate.pass_regressed".to_string());
    }
    if success_rate_delta < -0.0001 {
        regression_flags.push(format!(
            "success_rate.decreased({:.4})",
            success_rate_delta
        ));
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

    ComparisonSummary {
        baseline_path: baseline_path.display().to_string(),
        success_rate_delta,
        fake_fallback_rate_delta,
        cert_fp_events_delta,
        auto_disable_triggered_delta,
        auto_disable_recovered_delta,
        regression_flags,
    }
}

fn load_baseline_report(path: &Path) -> Result<SoakReport> {
    let contents = fs::read_to_string(path)
        .with_context(|| format!("read baseline report: {}", path.display()))?;
    serde_json::from_str(&contents)
        .with_context(|| format!("parse baseline report: {}", path.display()))
}

#[derive(Default)]
struct OperationStats {
    total: u64,
    completed: u64,
    failed: u64,
    canceled: u64,
}

impl OperationStats {
    fn record(&mut self, state: &TaskState) {
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
struct TimingSample {
    connect_ms: Option<u32>,
    tls_ms: Option<u32>,
    first_byte_ms: Option<u32>,
    total_ms: Option<u32>,
    used_fake: bool,
    fallback_stage: String,
    cert_fp_changed: bool,
}

#[derive(Default)]
struct TimingData {
    samples: Vec<TimingSample>,
    final_stage_counts: HashMap<String, u64>,
    used_fake_count: u64,
}

#[derive(Default)]
struct FallbackStats {
    counts: HashMap<String, u64>,
    fake_to_real: u64,
    real_to_default: u64,
}

#[derive(Default)]
struct AutoDisableStats {
    triggered: u64,
    recovered: u64,
}

#[derive(Default)]
struct SoakAggregator {
    operations: HashMap<String, OperationStats>,
    timing: HashMap<String, TimingData>,
    fallback: FallbackStats,
    auto_disable: AutoDisableStats,
    cert_fp_events: u64,
    expected_iterations: u32,
}

impl SoakAggregator {
    fn new(expected_iterations: u32) -> Self {
        Self {
            expected_iterations,
            ..Default::default()
        }
    }

    fn record_task(&mut self, kind: &str, state: TaskState) {
        self.operations
            .entry(kind.to_string())
            .or_default()
            .record(&state);
    }

    fn process_events(&mut self, events: Vec<Event>) {
        for evt in events {
            match evt {
                Event::Task(TaskEvent::Completed { .. }) | Event::Task(TaskEvent::Failed { .. }) => {
                    // Already tracked via record_task; nothing extra needed here.
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
                    let entry = self.timing.entry(kind.clone()).or_default();
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
                Event::Strategy(StrategyEvent::AdaptiveTlsFallback { kind, from, to, reason, .. }) => {
                    let key = format!("{}:{}->{}:{}", kind, from, to, reason);
                    *self.fallback.counts.entry(key).or_default() += 1;
                    if from == "Fake" && to == "Real" {
                        self.fallback.fake_to_real += 1;
                    }
                    if from == "Real" && to == "Default" {
                        self.fallback.real_to_default += 1;
                    }
                }
                Event::Strategy(StrategyEvent::AdaptiveTlsAutoDisable { enabled, .. }) => {
                    if enabled {
                        self.auto_disable.triggered += 1;
                    } else {
                        self.auto_disable.recovered += 1;
                    }
                }
                Event::Strategy(StrategyEvent::CertFingerprintChanged { .. }) => {
                    self.cert_fp_events += 1;
                }
                _ => {}
            }
        }
    }

    fn into_report(
        self,
        started_unix: u64,
        finished_unix: u64,
        duration_secs: u64,
        options: SoakOptionsSnapshot,
    ) -> SoakReport {
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

        let mut timing_summary = HashMap::new();
        let mut total_fake_attempts = 0u64;
        let mut cert_fp_changed_samples = 0u64;
        for (kind, data) in self.timing.iter() {
            let connect_vals: Vec<u32> = data
                .samples
                .iter()
                .filter_map(|s| s.connect_ms)
                .collect();
            let tls_vals: Vec<u32> = data.samples.iter().filter_map(|s| s.tls_ms).collect();
            let first_byte_vals: Vec<u32> =
                data.samples.iter().filter_map(|s| s.first_byte_ms).collect();
            let total_vals: Vec<u32> = data.samples.iter().filter_map(|s| s.total_ms).collect();
            let changed_count = data
                .samples
                .iter()
                .filter(|s| s.cert_fp_changed)
                .count() as u64;
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

        let success_rate = if total_operations > 0 {
            total_completed as f64 / total_operations as f64
        } else {
            1.0
        };
        let fallback_ratio = if total_fake_attempts > 0 {
            self.fallback.fake_to_real as f64 / total_fake_attempts as f64
        } else {
            0.0
        };

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
            totals: TotalsSummary {
                total_operations,
                completed: total_completed,
                failed: total_failed,
                canceled: total_canceled,
            },
            thresholds: ThresholdSummary {
                success_rate: ThresholdCheck::at_least(success_rate, 0.99),
                fake_fallback_rate: ThresholdCheck::at_most(fallback_ratio, 0.05),
            },
            comparison: None,
        }
    }
}

pub fn run_from_env() -> Result<SoakReport> {
    let guard = std::env::var("FWC_ADAPTIVE_TLS_SOAK")
        .unwrap_or_else(|_| "0".to_string());
    if guard != "1" {
        return Err(anyhow!(
            "FWC_ADAPTIVE_TLS_SOAK=1 is required to run the soak mode"
        ));
    }
    let iterations = std::env::var("FWC_SOAK_ITERATIONS")
        .ok()
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(SoakOptions::default().iterations);
    let keep_clones = std::env::var("FWC_SOAK_KEEP_CLONES")
        .ok()
        .map(|v| matches!(v.as_str(), "1" | "true" | "TRUE" | "True"))
        .unwrap_or(false);
    let report_path = std::env::var("FWC_SOAK_REPORT_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| SoakOptions::default().report_path);
    let base_dir = std::env::var("FWC_SOAK_BASE_DIR").ok().map(PathBuf::from);
    let baseline_report = std::env::var("FWC_SOAK_BASELINE_REPORT")
        .ok()
        .map(|s| PathBuf::from(s.trim()))
        .filter(|p| !p.as_os_str().is_empty());
    let opts = SoakOptions {
        iterations,
        keep_clones,
        report_path,
        base_dir,
        baseline_report,
    };
    run(opts)
}

pub fn run(opts: SoakOptions) -> Result<SoakReport> {
    let iterations = opts.iterations.max(1);
    setup_git_identity();

    let workspace_root = if let Some(dir) = opts.base_dir.clone() {
        dir
    } else {
        std::env::temp_dir().join(format!("fwc-soak-{}", Uuid::new_v4()))
    };
    fs::create_dir_all(&workspace_root)
        .with_context(|| format!("create workspace dir: {}", workspace_root.display()))?;
    let config_root = workspace_root.join("config-root");
    let runtime_root = workspace_root.join("runtime");
    fs::create_dir_all(&config_root)
        .with_context(|| format!("create config dir: {}", config_root.display()))?;
    fs::create_dir_all(&runtime_root)
        .with_context(|| format!("create runtime dir: {}", runtime_root.display()))?;
    let clones_root = runtime_root.join("clones");
    fs::create_dir_all(&clones_root)
        .with_context(|| format!("create clones dir: {}", clones_root.display()))?;
    let origin_dir = runtime_root.join("origin.git");
    let producer_dir = runtime_root.join("producer");
    let consumer_dir = runtime_root.join("consumer");

    // Ensure adaptive soak env flag is visible to downstream components.
    std::env::set_var("FWC_ADAPTIVE_TLS_SOAK", "1");
    loader::set_global_base_dir(&config_root);

    // Prepare origin bare repository.
    if origin_dir.exists() {
        fs::remove_dir_all(&origin_dir)
            .with_context(|| format!("clean existing origin: {}", origin_dir.display()))?;
    }
    git2::Repository::init_bare(&origin_dir)
        .with_context(|| format!("init bare origin at {}", origin_dir.display()))?;

    let branch_name = setup_producer(&origin_dir, &producer_dir)
        .context("initialize producer repository")?;

    if consumer_dir.exists() {
        fs::remove_dir_all(&consumer_dir)
            .with_context(|| format!("clean consumer dir: {}", consumer_dir.display()))?;
    }

    let runtime = build_runtime().context("build tokio runtime")?;
    let registry = Arc::new(TaskRegistry::new());
    let bus = Arc::new(MemoryEventBus::new());
    let bus_dyn: Arc<dyn EventBusAny> = bus.clone();
    registry.inject_structured_bus(bus_dyn.clone());
    let _ = structured::set_global_event_bus(bus_dyn);

    let mut aggregator = SoakAggregator::new(iterations);

    let started_at = SystemTime::now();
    let start_instant = Instant::now();

    // Bootstrap consumer clone (counts toward metrics).
    let bootstrap_state = run_clone_task(
        &registry,
        &runtime,
        origin_dir.as_path(),
        &consumer_dir,
        &mut aggregator,
        &bus,
    )
    .context("bootstrap consumer clone")?;
    ensure!(
        matches!(bootstrap_state, TaskState::Completed),
        "initial consumer clone failed with state {:?}",
        bootstrap_state
    );

    for round in 0..iterations {
        prepare_commit(&producer_dir, round, &branch_name)
            .with_context(|| format!("prepare commit for iteration {}", round))?;
        let push_state = run_push_task(
            &registry,
            &runtime,
            &producer_dir,
            &mut aggregator,
            &bus,
        )
        .with_context(|| format!("execute push task at iteration {}", round))?;
        ensure!(
            matches!(push_state, TaskState::Completed),
            "push task failed at iteration {} with state {:?}",
            round,
            push_state
        );

        let fetch_state = run_fetch_task(
            &registry,
            &runtime,
            &consumer_dir,
            &mut aggregator,
            &bus,
        )
        .with_context(|| format!("execute fetch task at iteration {}", round))?;
        ensure!(
            matches!(fetch_state, TaskState::Completed),
            "fetch task failed at iteration {} with state {:?}",
            round,
            fetch_state
        );

        let clone_dest = clones_root.join(format!("round-{}", round));
        if clone_dest.exists() {
            fs::remove_dir_all(&clone_dest)
                .with_context(|| format!("clean clone dest: {}", clone_dest.display()))?;
        }
        let clone_state = run_clone_task(
            &registry,
            &runtime,
            origin_dir.as_path(),
            &clone_dest,
            &mut aggregator,
            &bus,
        )
        .with_context(|| format!("execute clone task at iteration {}", round))?;
        ensure!(
            matches!(clone_state, TaskState::Completed),
            "clone task failed at iteration {} with state {:?}",
            round,
            clone_state
        );
        if !opts.keep_clones {
            let _ = fs::remove_dir_all(&clone_dest);
        }
    }

    aggregator.process_events(bus.take_all());

    let duration_secs = start_instant.elapsed().as_secs();
    let finished_at = SystemTime::now();
    let started_unix = system_time_to_unix(started_at);
    let finished_unix = system_time_to_unix(finished_at);

    let options_snapshot = SoakOptionsSnapshot {
        iterations,
        keep_clones: opts.keep_clones,
        report_path: opts.report_path.display().to_string(),
        workspace_dir: workspace_root.display().to_string(),
        baseline_report: opts
            .baseline_report
            .as_ref()
            .map(|p| p.display().to_string()),
    };

    let mut report = aggregator.into_report(
        started_unix,
        finished_unix,
        duration_secs,
        options_snapshot,
    );

    if let Some(baseline_path) = opts.baseline_report.as_ref() {
        match load_baseline_report(baseline_path) {
            Ok(baseline) => {
                let summary = build_comparison_summary(baseline_path, &baseline, &report);
                report.comparison = Some(summary);
            }
            Err(err) => {
                tracing::warn!(
                    target = "soak",
                    error = %err,
                    path = %baseline_path.display(),
                    "failed to load baseline report; continuing without comparison"
                );
            }
        }
    }

    write_report(&opts.report_path, &report)
        .with_context(|| format!("write soak report to {}", opts.report_path.display()))?;

    if !opts.keep_clones {
        let _ = fs::remove_dir_all(&runtime_root);
    }

    Ok(report)
}

fn build_runtime() -> Result<Runtime> {
    Builder::new_multi_thread()
        .enable_all()
        .worker_threads(2)
        .build()
        .map_err(|e| anyhow!(e))
}

fn system_time_to_unix(t: SystemTime) -> u64 {
    t.duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_secs(0))
        .as_secs()
}

fn compute_field_stats(values: &[u32]) -> Option<FieldStats> {
    if values.is_empty() {
        return None;
    }
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    let count = sorted.len();
    let min = sorted[0];
    let max = sorted[count - 1];
    let sum: u64 = sorted.iter().map(|&v| v as u64).sum();
    let avg = sum as f64 / count as f64;
    let p50 = percentile(&sorted, 0.5);
    let p95 = percentile(&sorted, 0.95);
    Some(FieldStats {
        count,
        min,
        max,
        avg,
        p50,
        p95,
    })
}

fn percentile(sorted: &[u32], q: f64) -> u32 {
    if sorted.is_empty() {
        return 0;
    }
    let pos = ((sorted.len() as f64 - 1.0) * q).round() as usize;
    sorted[pos.clamp(0, sorted.len() - 1)]
}

fn setup_git_identity() {
    if std::env::var("GIT_AUTHOR_NAME").is_err() {
        std::env::set_var("GIT_AUTHOR_NAME", "fwc-soak");
    }
    if std::env::var("GIT_AUTHOR_EMAIL").is_err() {
        std::env::set_var("GIT_AUTHOR_EMAIL", "fwc-soak@example.com");
    }
    if std::env::var("GIT_COMMITTER_NAME").is_err() {
        std::env::set_var("GIT_COMMITTER_NAME", "fwc-soak");
    }
    if std::env::var("GIT_COMMITTER_EMAIL").is_err() {
        std::env::set_var("GIT_COMMITTER_EMAIL", "fwc-soak@example.com");
    }
}

fn setup_producer(origin: &Path, producer: &Path) -> Result<String> {
    if producer.exists() {
        fs::remove_dir_all(producer)
            .with_context(|| format!("remove existing producer dir: {}", producer.display()))?;
    }
    fs::create_dir_all(producer)
        .with_context(|| format!("create producer dir: {}", producer.display()))?;
    let cancel = AtomicBool::new(false);
    init::git_init(producer, &cancel, |_| {})
        .map_err(|e| anyhow!("git init failed: {}", e))?;
    let readme = producer.join("README.md");
    fs::write(&readme, b"Adaptive TLS Soak\n")
        .with_context(|| format!("write {}", readme.display()))?;
    add::git_add(producer, &["README.md"], &cancel, |_| {})
        .map_err(|e| anyhow!("git add failed: {}", e))?;
    commit::git_commit(producer, "Initial soak seed", None, false, &cancel, |_| {})
        .map_err(|e| anyhow!("git commit failed: {}", e))?;
    let repo = git2::Repository::open(producer)
        .with_context(|| format!("open producer repo: {}", producer.display()))?;
    if repo.find_remote("origin").is_err() {
        let origin_str = origin
            .to_str()
            .ok_or_else(|| anyhow!("origin path contains invalid UTF-8"))?;
        repo.remote("origin", origin_str)
            .with_context(|| format!("add origin remote at {}", origin_str))?;
    }
    let head = repo.head().context("get HEAD after initial commit")?;
    let shorthand = head
        .shorthand()
        .map(|s| s.to_string())
        .unwrap_or_else(|| "master".to_string());
    let branch_ref = format!("refs/heads/{}", shorthand);
    let refspec_owned = format!("{}:{}", branch_ref, branch_ref);
    let refspecs: Vec<&str> = vec![refspec_owned.as_str()];
    let cancel_push = AtomicBool::new(false);
    push::do_push(
        producer,
        Some("origin"),
        Some(&refspecs),
        None,
        &cancel_push,
        |_| {},
    )
    .map_err(|e| anyhow!("initial push failed: {}", e))?;
    Ok(shorthand)
}

fn prepare_commit(repo: &Path, iteration: u32, branch: &str) -> Result<()> {
    let cancel = AtomicBool::new(false);
    let filename = format!("soak_iter_{iteration}.txt");
    let path = repo.join(&filename);
    let content = format!(
        "iteration {iteration} on branch {branch} at {}\n",
        chrono_like_timestamp()
    );
    fs::write(&path, content.as_bytes())
        .with_context(|| format!("write file {}", path.display()))?;
    add::git_add(repo, &[filename.as_str()], &cancel, |_| {})
        .map_err(|e| anyhow!("git add failed: {}", e))?;
    commit::git_commit(
        repo,
        &format!("Soak iteration {iteration}"),
        None,
        false,
        &cancel,
        |_| {},
    )
    .map_err(|e| anyhow!("git commit failed: {}", e))?
        ;
    Ok(())
}

fn chrono_like_timestamp() -> String {
    let now = SystemTime::now();
    let secs = now
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_secs(0))
        .as_secs();
    format!("{}", secs)
}

fn run_push_task(
    registry: &Arc<TaskRegistry>,
    runtime: &Runtime,
    repo: &Path,
    aggregator: &mut SoakAggregator,
    bus: &Arc<MemoryEventBus>,
) -> Result<TaskState> {
    let dest_str = repo
        .to_str()
        .ok_or_else(|| anyhow!("push repo path invalid UTF-8"))?
        .to_string();
    let (id, token) = registry.create(TaskKind::GitPush {
        dest: dest_str.clone(),
        remote: Some("origin".to_string()),
        refspecs: None,
        username: None,
        password: None,
        strategy_override: None,
    });
    let handle = runtime.block_on({
        let registry = Arc::clone(registry);
        let dest_str = dest_str;
        async move {
            registry.spawn_git_push_task(
                None,
                id,
                token,
                dest_str,
                Some("origin".to_string()),
                None,
                None,
                None,
                None,
            )
        }
    });
    runtime
        .block_on(async { handle.await.map_err(|e| anyhow!(e)) })
        .context("await push task")?;
    let state = registry
        .snapshot(&id)
        .ok_or_else(|| anyhow!("push snapshot missing"))?
        .state;
    aggregator.record_task("GitPush", state.clone());
    aggregator.process_events(bus.take_all());
    Ok(state)
}

fn run_fetch_task(
    registry: &Arc<TaskRegistry>,
    runtime: &Runtime,
    repo: &Path,
    aggregator: &mut SoakAggregator,
    bus: &Arc<MemoryEventBus>,
) -> Result<TaskState> {
    let repo_str = repo
        .to_str()
        .ok_or_else(|| anyhow!("fetch repo path invalid UTF-8"))?
        .to_string();
    let (id, token) = registry.create(TaskKind::GitFetch {
        repo: "".to_string(),
        dest: repo_str.clone(),
        depth: None,
        filter: None,
        strategy_override: None,
    });
    let handle = runtime.block_on({
        let registry = Arc::clone(registry);
        async move {
            registry.spawn_git_fetch_task_with_opts(
                None,
                id,
                token,
                "".to_string(),
                repo_str,
                None,
                None,
                None,
                None,
            )
        }
    });
    runtime
        .block_on(async { handle.await.map_err(|e| anyhow!(e)) })
        .context("await fetch task")?;
    let state = registry
        .snapshot(&id)
        .ok_or_else(|| anyhow!("fetch snapshot missing"))?
        .state;
    aggregator.record_task("GitFetch", state.clone());
    aggregator.process_events(bus.take_all());
    Ok(state)
}

fn run_clone_task(
    registry: &Arc<TaskRegistry>,
    runtime: &Runtime,
    origin: &Path,
    dest: &Path,
    aggregator: &mut SoakAggregator,
    bus: &Arc<MemoryEventBus>,
) -> Result<TaskState> {
    let origin_str = origin
        .to_str()
        .ok_or_else(|| anyhow!("origin path invalid UTF-8"))?
        .to_string();
    let dest_str = dest
        .to_str()
        .ok_or_else(|| anyhow!("dest path invalid UTF-8"))?
        .to_string();
    let (id, token) = registry.create(TaskKind::GitClone {
        repo: origin_str.clone(),
        dest: dest_str.clone(),
        depth: None,
        filter: None,
        strategy_override: None,
    });
    let handle = runtime.block_on({
        let registry = Arc::clone(registry);
        async move {
            registry.spawn_git_clone_task_with_opts(
                None,
                id,
                token,
                origin_str,
                dest_str,
                None,
                None,
                None,
            )
        }
    });
    runtime
        .block_on(async { handle.await.map_err(|e| anyhow!(e)) })
        .context("await clone task")?;
    let state = registry
        .snapshot(&id)
        .ok_or_else(|| anyhow!("clone snapshot missing"))?
        .state;
    aggregator.record_task("GitClone", state.clone());
    aggregator.process_events(bus.take_all());
    Ok(state)
}

fn write_report(path: &Path, report: &SoakReport) -> Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)
                .with_context(|| format!("create parent dir: {}", parent.display()))?;
        }
    }
    let json = serde_json::to_string_pretty(report)?;
    fs::write(path, json)
        .with_context(|| format!("write report file: {}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn soak_runs_minimal_iterations() {
        let workspace = std::env::temp_dir().join(format!("fwc-soak-test-{}", Uuid::new_v4()));
        let opts = SoakOptions {
            iterations: 1,
            keep_clones: false,
            report_path: workspace.join("report.json"),
            base_dir: Some(workspace.clone()),
            baseline_report: None,
        };
        let report = run(opts).expect("soak run should succeed");
        assert!(report.iterations >= 1);
        assert!(report.totals.total_operations >= 3);
        let _ = fs::remove_dir_all(workspace);
    }

    #[test]
    fn soak_attaches_comparison_when_baseline_available() {
        let base = std::env::temp_dir().join(format!("fwc-soak-baseline-{}", Uuid::new_v4()));
        let baseline_workspace = base.join("baseline");
        let baseline_report_path = baseline_workspace.join("report.json");
        let baseline_opts = SoakOptions {
            iterations: 1,
            keep_clones: false,
            report_path: baseline_report_path.clone(),
            base_dir: Some(baseline_workspace.clone()),
            baseline_report: None,
        };
        let baseline_report = run(baseline_opts).expect("baseline soak run should succeed");
        assert!(baseline_report.comparison.is_none());

        let current_workspace = base.join("current");
        let current_report_path = current_workspace.join("report.json");
        let current_opts = SoakOptions {
            iterations: 1,
            keep_clones: false,
            report_path: current_report_path.clone(),
            base_dir: Some(current_workspace.clone()),
            baseline_report: Some(baseline_report_path.clone()),
        };
        let current_report = run(current_opts).expect("current soak run should succeed");
        let comparison = current_report
            .comparison
            .expect("comparison summary should be attached");
        assert_eq!(
            comparison.baseline_path,
            baseline_report_path.display().to_string()
        );
        assert!(comparison.regression_flags.is_empty());
        assert!(comparison.success_rate_delta.abs() < 1e-6);
        assert!(comparison.fake_fallback_rate_delta.abs() < 1e-6);
        assert_eq!(comparison.cert_fp_events_delta, 0);

        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn soak_ignores_invalid_baseline_report() {
        let base = std::env::temp_dir().join(format!("fwc-soak-invalid-{}", Uuid::new_v4()));
        let config_workspace = base.join("config");
        let report_path = base.join("report.json");
        let baseline_path = base.join("baseline.json");
        fs::create_dir_all(base.clone()).expect("create temp dirs");
        fs::write(&baseline_path, b"not-json").expect("write invalid baseline");

        let opts = SoakOptions {
            iterations: 1,
            keep_clones: false,
            report_path: report_path.clone(),
            base_dir: Some(config_workspace.clone()),
            baseline_report: Some(baseline_path.clone()),
        };
        let report = run(opts).expect("soak run should succeed even with invalid baseline");
        assert!(report.comparison.is_none());

        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn comparison_summary_detects_regressions() {
        let baseline = SoakReport {
            started_unix: 0,
            finished_unix: 0,
            duration_secs: 0,
            options: SoakOptionsSnapshot {
                iterations: 1,
                keep_clones: false,
                report_path: "memory".into(),
                workspace_dir: "memory".into(),
                baseline_report: None,
            },
            iterations: 1,
            operations: HashMap::new(),
            timing: HashMap::new(),
            fallback: FallbackSummary {
                counts: HashMap::new(),
                fake_to_real: 1,
                real_to_default: 0,
            },
            auto_disable: AutoDisableSummary {
                triggered: 1,
                recovered: 1,
            },
            cert_fp_events: 2,
            totals: TotalsSummary {
                total_operations: 3,
                completed: 3,
                failed: 0,
                canceled: 0,
            },
            thresholds: ThresholdSummary {
                success_rate: ThresholdCheck {
                    pass: true,
                    actual: 1.0,
                    expected: 0.99,
                    comparator: ">=".to_string(),
                },
                fake_fallback_rate: ThresholdCheck {
                    pass: true,
                    actual: 0.02,
                    expected: 0.05,
                    comparator: "<=".to_string(),
                },
            },
            comparison: None,
        };

    let mut current = baseline.clone();
        current.thresholds.success_rate = ThresholdCheck {
            pass: false,
            actual: 0.9,
            expected: 0.99,
            comparator: ">=".to_string(),
        };
        current.thresholds.fake_fallback_rate = ThresholdCheck {
            pass: false,
            actual: 0.1,
            expected: 0.05,
            comparator: "<=".to_string(),
        };
        current.cert_fp_events = 5;
        current.auto_disable.triggered = 3;
        current.auto_disable.recovered = 1;

    let comparison = build_comparison_summary(Path::new("baseline.json"), &baseline, &current);
        assert!(comparison.success_rate_delta < 0.0);
        assert!(comparison.fake_fallback_rate_delta > 0.0);
        assert_eq!(comparison.cert_fp_events_delta, 3);
        assert_eq!(comparison.auto_disable_triggered_delta, 2);
        assert!(comparison
            .regression_flags
            .iter()
            .any(|f| f.contains("success_rate.pass_regressed")));
        assert!(comparison
            .regression_flags
            .iter()
            .any(|f| f.contains("fake_fallback_rate.pass_regressed")));
        assert!(comparison
            .regression_flags
            .iter()
            .any(|f| f.contains("auto_disable.triggered_increase")));
    }

    #[test]
    fn aggregator_threshold_detects_high_fallback_ratio() {
        let mut agg = SoakAggregator::new(1);
        agg.record_task("GitClone", TaskState::Completed);
        agg.process_events(vec![
            Event::Strategy(StrategyEvent::AdaptiveTlsTiming {
                id: "test-clone".into(),
                kind: "GitClone".into(),
                used_fake_sni: true,
                fallback_stage: "Real".into(),
                connect_ms: Some(12),
                tls_ms: Some(34),
                first_byte_ms: Some(56),
                total_ms: Some(78),
                cert_fp_changed: false,
            }),
            Event::Strategy(StrategyEvent::AdaptiveTlsFallback {
                id: "test-clone".into(),
                kind: "GitClone".into(),
                from: "Fake".into(),
                to: "Real".into(),
                reason: "FakeHandshakeError".into(),
            }),
        ]);
        let report = agg.into_report(
            0,
            0,
            0,
            SoakOptionsSnapshot {
                iterations: 1,
                keep_clones: false,
                report_path: "memory".into(),
                workspace_dir: "memory".into(),
                baseline_report: None,
            },
        );
        assert!(report.thresholds.success_rate.pass);
        assert!(!report.thresholds.fake_fallback_rate.pass);
        assert!((report.thresholds.fake_fallback_rate.actual - 1.0).abs() < f64::EPSILON);
    }
}
