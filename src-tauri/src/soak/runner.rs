use anyhow::{anyhow, ensure, Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::{Instant, SystemTime};
use uuid::Uuid;

use crate::core::config::loader;
use crate::core::git::default_impl::{add, commit, init, push};
use crate::core::tasks::model::TaskState;
use crate::core::tasks::TaskRegistry;
use crate::events::structured::{self, EventBusAny, MemoryEventBus};

use super::aggregator::SoakAggregator;
use super::models::*;
use super::tasks::{run_clone_task, run_fetch_task, run_push_task};
use super::utils::*;

/// Run soak test from environment variables.
pub fn run_from_env() -> Result<SoakReport> {
    let guard = std::env::var("FWC_ADAPTIVE_TLS_SOAK").unwrap_or_else(|_| "0".to_string());
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

    let mut thresholds = SoakThresholds::default();
    if let Some(v) = parse_env_f64("FWC_SOAK_MIN_SUCCESS_RATE") {
        thresholds.min_success_rate = v;
    }
    if let Some(v) = parse_env_f64("FWC_SOAK_MAX_FAKE_FALLBACK_RATE") {
        thresholds.max_fake_fallback_rate = v;
    }
    if let Some(v) = parse_env_f64("FWC_SOAK_MIN_IP_POOL_REFRESH_RATE") {
        thresholds.min_ip_pool_refresh_success_rate = v;
    }
    if let Some(v) = parse_env_u64("FWC_SOAK_MAX_AUTO_DISABLE") {
        thresholds.max_auto_disable_triggered = v;
    }
    if let Ok(raw) = std::env::var("FWC_SOAK_MIN_LATENCY_IMPROVEMENT") {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            thresholds.min_latency_improvement = None;
        } else if let Ok(parsed) = trimmed.parse::<f64>() {
            thresholds.min_latency_improvement = Some(parsed);
        }
    }

    let opts = SoakOptions {
        iterations,
        keep_clones,
        report_path,
        base_dir,
        baseline_report,
        thresholds,
    };
    run(opts)
}

/// Run soak test with the given options.
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

    // Ensure adaptive soak env flag is visible to downstream components
    std::env::set_var("FWC_ADAPTIVE_TLS_SOAK", "1");
    loader::set_global_base_dir(&config_root);

    // Prepare origin bare repository
    if origin_dir.exists() {
        fs::remove_dir_all(&origin_dir)
            .with_context(|| format!("clean existing origin: {}", origin_dir.display()))?;
    }
    git2::Repository::init_bare(&origin_dir)
        .with_context(|| format!("init bare origin at {}", origin_dir.display()))?;

    let branch_name =
        setup_producer(&origin_dir, &producer_dir).context("initialize producer repository")?;

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

    // Bootstrap consumer clone (counts toward metrics)
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

    // Run soak iterations
    for round in 0..iterations {
        prepare_commit(&producer_dir, round, &branch_name)
            .with_context(|| format!("prepare commit for iteration {}", round))?;

        let push_state = run_push_task(&registry, &runtime, &producer_dir, &mut aggregator, &bus)
            .with_context(|| format!("execute push task at iteration {}", round))?;
        ensure!(
            matches!(push_state, TaskState::Completed),
            "push task failed at iteration {} with state {:?}",
            round,
            push_state
        );

        let fetch_state = run_fetch_task(&registry, &runtime, &consumer_dir, &mut aggregator, &bus)
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
        thresholds: opts.thresholds.clone(),
    };

    let mut report =
        aggregator.into_report(started_unix, finished_unix, duration_secs, options_snapshot);

    // Process baseline comparison if provided
    if let Some(baseline_path) = opts.baseline_report.as_ref() {
        match load_baseline_report(baseline_path) {
            Ok(baseline) => {
                let summary = build_comparison_summary(baseline_path, &baseline, &report);
                if let Some(target) = report.options.thresholds.min_latency_improvement {
                    let latency_check =
                        if let Some(improvement) = summary.git_clone_total_p50_improvement {
                            ThresholdCheck::at_least(improvement, target)
                        } else {
                            ThresholdCheck::not_applicable(
                                target,
                                ">=",
                                "GitClone total_ms p50 unavailable in baseline or current report",
                            )
                        };
                    report.thresholds.set_latency_improvement(latency_check);
                }
                report.comparison = Some(summary);
            }
            Err(err) => {
                tracing::warn!(
                    target = "soak",
                    error = %err,
                    path = %baseline_path.display(),
                    "failed to load baseline report; continuing without comparison"
                );
                if let Some(target) = report.options.thresholds.min_latency_improvement {
                    let latency_check = ThresholdCheck::not_applicable(
                        target,
                        ">=",
                        format!("failed to load baseline: {err}"),
                    );
                    report.thresholds.set_latency_improvement(latency_check);
                }
            }
        }
    } else if let Some(target) = report.options.thresholds.min_latency_improvement {
        let latency_check = ThresholdCheck::not_applicable(
            target,
            ">=",
            "baseline report not provided; latency improvement cannot be evaluated",
        );
        report.thresholds.set_latency_improvement(latency_check);
    }

    write_report(&opts.report_path, &report)
        .with_context(|| format!("write soak report to {}", opts.report_path.display()))?;

    if !opts.keep_clones {
        let _ = fs::remove_dir_all(&runtime_root);
    }

    Ok(report)
}

/// Setup producer repository with initial commit.
fn setup_producer(origin: &Path, producer: &Path) -> Result<String> {
    if producer.exists() {
        fs::remove_dir_all(producer)
            .with_context(|| format!("remove existing producer dir: {}", producer.display()))?;
    }
    fs::create_dir_all(producer)
        .with_context(|| format!("create producer dir: {}", producer.display()))?;

    let cancel = AtomicBool::new(false);
    init::git_init(producer, &cancel, |_| {}).map_err(|e| anyhow!("git init failed: {}", e))?;

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

/// Prepare a new commit for a soak iteration.
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
    .map_err(|e| anyhow!("git commit failed: {}", e))?;

    Ok(())
}
