use fireworks_collaboration_lib::core::tasks::model::TaskState;
use fireworks_collaboration_lib::events::structured::{Event, MetricAlertState, StrategyEvent};
use fireworks_collaboration_lib::soak::{
    build_comparison_summary, run, run_from_env, AlertsSummary, AutoDisableSummary,
    FallbackSummary, FieldStats, IpPoolSummary, ProxySummary, SoakAggregator, SoakOptions,
    SoakOptionsSnapshot, SoakReport, SoakThresholds, ThresholdCheck, ThresholdSummary,
    TimingSummary, TotalsSummary,
};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use uuid::Uuid;

struct EnvSnapshot {
    vars: HashMap<String, Option<String>>,
}

impl EnvSnapshot {
    fn capture(keys: &[&str]) -> Self {
        let vars = keys
            .iter()
            .map(|k| (k.to_string(), std::env::var(k).ok()))
            .collect();
        Self { vars }
    }
}

impl Drop for EnvSnapshot {
    fn drop(&mut self) {
        for (key, maybe_value) in &self.vars {
            match maybe_value {
                Some(val) => std::env::set_var(key, val),
                None => std::env::remove_var(key),
            }
        }
    }
}

#[test]
fn soak_runs_minimal_iterations() {
    let workspace = std::env::temp_dir().join(format!("fwc-soak-test-{}", Uuid::new_v4()));
    let opts = SoakOptions {
        iterations: 1,
        keep_clones: false,
        report_path: workspace.join("report.json"),
        base_dir: Some(workspace.clone()),
        baseline_report: None,
        thresholds: SoakThresholds::default(),
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
        thresholds: SoakThresholds::default(),
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
        thresholds: SoakThresholds::default(),
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
        thresholds: SoakThresholds::default(),
    };
    let report = run(opts).expect("soak run should succeed even with invalid baseline");
    assert!(report.comparison.is_none());

    let _ = fs::remove_dir_all(base);
}

#[test]
fn run_from_env_honors_environment_threshold_overrides() {
    let keys = [
        "FWC_ADAPTIVE_TLS_SOAK",
        "FWC_SOAK_ITERATIONS",
        "FWC_SOAK_KEEP_CLONES",
        "FWC_SOAK_REPORT_PATH",
        "FWC_SOAK_BASE_DIR",
        "FWC_SOAK_MIN_SUCCESS_RATE",
        "FWC_SOAK_MAX_FAKE_FALLBACK_RATE",
        "FWC_SOAK_MIN_IP_POOL_REFRESH_RATE",
        "FWC_SOAK_MAX_AUTO_DISABLE",
        "FWC_SOAK_MIN_LATENCY_IMPROVEMENT",
    ];
    let _snapshot = EnvSnapshot::capture(&keys);

    let base = std::env::temp_dir().join(format!("fwc-soak-env-{}", Uuid::new_v4()));
    let report_path = base.join("report.json");
    fs::create_dir_all(&base).expect("create base dir");

    std::env::set_var("FWC_ADAPTIVE_TLS_SOAK", "1");
    std::env::set_var("FWC_SOAK_ITERATIONS", "1");
    std::env::set_var("FWC_SOAK_KEEP_CLONES", "0");
    std::env::set_var(
        "FWC_SOAK_REPORT_PATH",
        report_path.to_str().expect("report path utf-8"),
    );
    std::env::set_var("FWC_SOAK_BASE_DIR", base.to_str().expect("base dir utf-8"));
    std::env::set_var("FWC_SOAK_MIN_SUCCESS_RATE", "0.75");
    std::env::set_var("FWC_SOAK_MAX_FAKE_FALLBACK_RATE", "0.25");
    std::env::set_var("FWC_SOAK_MIN_IP_POOL_REFRESH_RATE", "0.5");
    std::env::set_var("FWC_SOAK_MAX_AUTO_DISABLE", "2");
    std::env::set_var("FWC_SOAK_MIN_LATENCY_IMPROVEMENT", "");

    let report = run_from_env().expect("soak run should succeed via env");
    assert_eq!(report.iterations, 1);
    assert_eq!(report.options.thresholds.min_success_rate, 0.75);
    assert_eq!(report.options.thresholds.max_fake_fallback_rate, 0.25);
    assert_eq!(
        report.options.thresholds.min_ip_pool_refresh_success_rate,
        0.5
    );
    assert_eq!(report.options.thresholds.max_auto_disable_triggered, 2);
    assert!(report.options.thresholds.min_latency_improvement.is_none());
    assert!(report.thresholds.latency_improvement.is_none());
    assert!(report.thresholds.ready);
    assert!(report_path.exists());

    let _ = fs::remove_dir_all(&base);
}

#[test]
fn ip_pool_stats_process_events_correctly() {
    let mut agg = SoakAggregator::default();
    agg.expected_iterations = 3;

    // Simulate IP pool selection events
    agg.process_events(vec![
        Event::Strategy(StrategyEvent::IpPoolSelection {
            id: "task1".into(),
            domain: "github.com".into(),
            port: 443,
            strategy: "Cached".into(),
            source: Some("Builtin".into()),
            latency_ms: Some(20),
            candidates_count: 3,
        }),
        Event::Strategy(StrategyEvent::IpPoolSelection {
            id: "task2".into(),
            domain: "github.com".into(),
            port: 443,
            strategy: "SystemDefault".into(),
            source: None,
            latency_ms: None,
            candidates_count: 0,
        }),
    ]);

    assert_eq!(agg.ip_pool.selection_total, 2);
    assert_eq!(*agg.ip_pool.selection_by_strategy.get("Cached").unwrap(), 1);
    assert_eq!(
        *agg.ip_pool
            .selection_by_strategy
            .get("SystemDefault")
            .unwrap(),
        1
    );

    // Simulate IP pool refresh events
    agg.process_events(vec![
        Event::Strategy(StrategyEvent::IpPoolRefresh {
            id: "preheat1".into(),
            domain: "github.com".into(),
            success: true,
            candidates_count: 5,
            min_latency_ms: Some(10),
            max_latency_ms: Some(50),
            reason: "preheat".into(),
        }),
        Event::Strategy(StrategyEvent::IpPoolRefresh {
            id: "preheat2".into(),
            domain: "example.com".into(),
            success: false,
            candidates_count: 0,
            min_latency_ms: None,
            max_latency_ms: None,
            reason: "no_candidates".into(),
        }),
        Event::Strategy(StrategyEvent::IpPoolRefresh {
            id: "preheat3".into(),
            domain: "test.com".into(),
            success: true,
            candidates_count: 3,
            min_latency_ms: Some(15),
            max_latency_ms: Some(30),
            reason: "preheat".into(),
        }),
    ]);

    assert_eq!(agg.ip_pool.refresh_total, 3);
    assert_eq!(agg.ip_pool.refresh_success, 2);
    assert_eq!(agg.ip_pool.refresh_failure, 1);

    let opts_snapshot = SoakOptionsSnapshot {
        iterations: 3,
        keep_clones: false,
        report_path: "memory".into(),
        workspace_dir: "memory".into(),
        baseline_report: None,
        thresholds: SoakThresholds::default(),
    };
    let report = agg.into_report(0, 0, 0, opts_snapshot);
    assert_eq!(report.ip_pool.selection_total, 2);
    assert_eq!(report.ip_pool.refresh_total, 3);
    assert_eq!(report.ip_pool.refresh_success, 2);
    assert_eq!(report.ip_pool.refresh_failure, 1);
    assert!((report.ip_pool.refresh_success_rate - 2.0 / 3.0).abs() < 1e-6);
}

#[test]
fn ip_pool_stats_calculates_success_rate_with_zero_refreshes() {
    let mut agg = SoakAggregator::default();
    agg.expected_iterations = 1;

    let opts_snapshot = SoakOptionsSnapshot {
        iterations: 1,
        keep_clones: false,
        report_path: "memory".into(),
        workspace_dir: "memory".into(),
        baseline_report: None,
        thresholds: SoakThresholds::default(),
    };
    let report = agg.into_report(0, 0, 0, opts_snapshot);
    assert_eq!(report.ip_pool.refresh_total, 0);
    assert_eq!(report.ip_pool.refresh_success_rate, 1.0); // Default to 1.0 when no refreshes
}

#[test]
fn soak_report_serialization_includes_ip_pool() {
    let report = SoakReport {
        started_unix: 0,
        finished_unix: 0,
        duration_secs: 0,
        options: SoakOptionsSnapshot {
            iterations: 1,
            keep_clones: false,
            report_path: "memory".into(),
            workspace_dir: "memory".into(),
            baseline_report: None,
            thresholds: SoakThresholds::default(),
        },
        iterations: 1,
        operations: HashMap::new(),
        timing: HashMap::new(),
        fallback: FallbackSummary {
            counts: HashMap::new(),
            fake_to_real: 0,
            real_to_default: 0,
        },
        auto_disable: AutoDisableSummary {
            triggered: 0,
            recovered: 0,
        },
        cert_fp_events: 0,
        ip_pool: IpPoolSummary {
            selection_total: 10,
            selection_by_strategy: [("Cached".to_string(), 8), ("SystemDefault".to_string(), 2)]
                .iter()
                .cloned()
                .collect(),
            refresh_total: 5,
            refresh_success: 4,
            refresh_failure: 1,
            refresh_success_rate: 0.8,
        },
        proxy: ProxySummary {
            fallback_count: 0,
            recovered_count: 0,
            health_check_total: 0,
            health_check_success: 0,
            avg_health_check_latency_ms: None,
            system_proxy_detect_total: 0,
            system_proxy_detect_success: 0,
            system_proxy_detect_success_rate: 0.0,
        },
        totals: TotalsSummary {
            total_operations: 1,
            completed: 1,
            failed: 0,
            canceled: 0,
        },
        thresholds: ThresholdSummary::new(
            ThresholdCheck::at_least(1.0, 0.99),
            ThresholdCheck::at_most(0.0, 0.05),
            None,
            Some(ThresholdCheck::at_most(0.0, 0.0)),
            None,
        ),
        alerts: AlertsSummary::default(),
        comparison: None,
    };

    let json = serde_json::to_string(&report).expect("serialize report");
    assert!(json.contains("\"selection_total\":10"));
    assert!(json.contains("\"refresh_success_rate\":0.8"));

    let deserialized: SoakReport = serde_json::from_str(&json).expect("deserialize report");
    assert_eq!(deserialized.ip_pool.selection_total, 10);
    assert_eq!(deserialized.ip_pool.refresh_total, 5);
    assert!((deserialized.ip_pool.refresh_success_rate - 0.8).abs() < 1e-6);
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
            thresholds: SoakThresholds::default(),
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
            triggered: 0,
            recovered: 0,
        },
        cert_fp_events: 2,
        ip_pool: IpPoolSummary {
            selection_total: 0,
            selection_by_strategy: HashMap::new(),
            refresh_total: 0,
            refresh_success: 0,
            refresh_failure: 0,
            refresh_success_rate: 1.0,
        },
        proxy: ProxySummary {
            fallback_count: 0,
            recovered_count: 0,
            health_check_total: 0,
            health_check_success: 0,
            avg_health_check_latency_ms: None,
            system_proxy_detect_total: 0,
            system_proxy_detect_success: 0,
            system_proxy_detect_success_rate: 0.0,
        },
        totals: TotalsSummary {
            total_operations: 3,
            completed: 3,
            failed: 0,
            canceled: 0,
        },
        thresholds: ThresholdSummary::new(
            ThresholdCheck::at_least(1.0, 0.99),
            ThresholdCheck::at_most(0.02, 0.05),
            None,
            Some(ThresholdCheck::at_most(0.0, 0.0)),
            None,
        ),
        alerts: AlertsSummary::default(),
        comparison: None,
    };

    let mut current = baseline.clone();
    current.thresholds = ThresholdSummary::new(
        ThresholdCheck::at_least(0.9, 0.99),
        ThresholdCheck::at_most(0.1, 0.05),
        None,
        Some(ThresholdCheck::at_most(3.0, 0.0)),
        None,
    );
    current.cert_fp_events = 5;
    current.auto_disable.triggered = 3;
    current.auto_disable.recovered = 1;

    let comparison = build_comparison_summary(Path::new("baseline.json"), &baseline, &current);
    assert!(comparison.success_rate_delta < 0.0);
    assert!(comparison.fake_fallback_rate_delta > 0.0);
    assert_eq!(comparison.cert_fp_events_delta, 3);
    assert_eq!(comparison.auto_disable_triggered_delta, 3);
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
            ip_source: None,
            ip_latency_ms: None,
            ip_selection_stage: None,
        }),
        Event::Strategy(StrategyEvent::AdaptiveTlsFallback {
            id: "test-clone".into(),
            kind: "GitClone".into(),
            from: "Fake".into(),
            to: "Real".into(),
            reason: "FakeHandshakeError".into(),
            ip_source: None,
            ip_latency_ms: None,
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
            thresholds: SoakThresholds::default(),
        },
    );
    assert!(report.thresholds.success_rate.pass);
    assert!(!report.thresholds.fake_fallback_rate.pass);
    assert!((report.thresholds.fake_fallback_rate.actual - 1.0).abs() < f64::EPSILON);
}

#[test]
fn aggregator_records_blocking_alerts() {
    let mut agg = SoakAggregator::new(1);
    agg.process_events(vec![Event::Strategy(StrategyEvent::MetricAlert {
        rule_id: "critical_rule".into(),
        severity: "critical".into(),
        state: MetricAlertState::Firing,
        value: 0.92,
        threshold: 0.80,
        comparator: ">".into(),
        timestamp_ms: Some(1_700_000_000_000),
    })]);

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
            thresholds: SoakThresholds::default(),
        },
    );

    assert_eq!(report.alerts.active.len(), 1);
    assert!(report.alerts.has_blocking);
    assert!(!report.thresholds.ready);
    assert!(report
        .thresholds
        .failing_checks
        .contains(&"alerts_active".to_string()));
}

#[test]
fn aggregator_alert_resolution_clears_blocking() {
    let mut agg = SoakAggregator::new(1);
    agg.process_events(vec![
        Event::Strategy(StrategyEvent::MetricAlert {
            rule_id: "critical_rule".into(),
            severity: "critical".into(),
            state: MetricAlertState::Firing,
            value: 1.2,
            threshold: 1.0,
            comparator: ">".into(),
            timestamp_ms: Some(1_700_000_100_000),
        }),
        Event::Strategy(StrategyEvent::MetricAlert {
            rule_id: "critical_rule".into(),
            severity: "critical".into(),
            state: MetricAlertState::Resolved,
            value: 0.8,
            threshold: 1.0,
            comparator: ">".into(),
            timestamp_ms: Some(1_700_000_200_000),
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
            thresholds: SoakThresholds::default(),
        },
    );

    assert!(report.alerts.active.is_empty());
    assert!(!report.alerts.has_blocking);
    assert!(report.thresholds.ready);
    assert!(!report
        .thresholds
        .failing_checks
        .contains(&"alerts_active".to_string()));
    assert_eq!(report.alerts.history.len(), 2);
}

#[test]
fn threshold_summary_recompute_after_latency_update() {
    let mut summary = ThresholdSummary::new(
        ThresholdCheck::at_least(1.0, 0.99),
        ThresholdCheck::at_most(0.01, 0.05),
        None,
        Some(ThresholdCheck::at_most(0.0, 0.0)),
        None,
    );
    assert!(summary.ready);

    summary.set_latency_improvement(ThresholdCheck::at_least(0.20, 0.15));
    assert!(summary.ready);

    summary.set_latency_improvement(ThresholdCheck::at_least(0.05, 0.15));
    assert!(!summary.ready);
    assert!(summary
        .failing_checks
        .contains(&"latency_improvement".to_string()));
    assert!(summary
        .latency_improvement
        .as_ref()
        .and_then(|c| c.details.as_ref())
        .is_none());
}

#[test]
fn comparison_summary_computes_latency_improvement() {
    let mut baseline_timing = HashMap::new();
    baseline_timing.insert(
        "GitClone".to_string(),
        TimingSummary {
            samples: 10,
            used_fake: 5,
            cert_fp_changed_samples: 0,
            final_stage_counts: HashMap::new(),
            connect_ms: None,
            tls_ms: None,
            first_byte_ms: None,
            total_ms: Some(FieldStats {
                count: 10,
                min: 180,
                max: 240,
                avg: 200.0,
                p50: 200,
                p95: 230,
            }),
        },
    );

    let baseline = SoakReport {
        started_unix: 0,
        finished_unix: 0,
        duration_secs: 0,
        options: SoakOptionsSnapshot {
            iterations: 1,
            keep_clones: false,
            report_path: "baseline".into(),
            workspace_dir: "baseline".into(),
            baseline_report: None,
            thresholds: SoakThresholds::default(),
        },
        iterations: 1,
        operations: HashMap::new(),
        timing: baseline_timing,
        fallback: FallbackSummary {
            counts: HashMap::new(),
            fake_to_real: 0,
            real_to_default: 0,
        },
        auto_disable: AutoDisableSummary {
            triggered: 0,
            recovered: 0,
        },
        cert_fp_events: 0,
        ip_pool: IpPoolSummary {
            selection_total: 0,
            selection_by_strategy: HashMap::new(),
            refresh_total: 0,
            refresh_success: 0,
            refresh_failure: 0,
            refresh_success_rate: 1.0,
        },
        proxy: ProxySummary {
            fallback_count: 0,
            recovered_count: 0,
            health_check_total: 0,
            health_check_success: 0,
            avg_health_check_latency_ms: None,
            system_proxy_detect_total: 0,
            system_proxy_detect_success: 0,
            system_proxy_detect_success_rate: 0.0,
        },
        totals: TotalsSummary {
            total_operations: 0,
            completed: 0,
            failed: 0,
            canceled: 0,
        },
        thresholds: ThresholdSummary::new(
            ThresholdCheck::at_least(1.0, 0.99),
            ThresholdCheck::at_most(0.0, 0.05),
            None,
            Some(ThresholdCheck::at_most(0.0, 0.0)),
            None,
        ),
        alerts: AlertsSummary::default(),
        comparison: None,
    };

    let mut current_timing = HashMap::new();
    current_timing.insert(
        "GitClone".to_string(),
        TimingSummary {
            samples: 10,
            used_fake: 5,
            cert_fp_changed_samples: 0,
            final_stage_counts: HashMap::new(),
            connect_ms: None,
            tls_ms: None,
            first_byte_ms: None,
            total_ms: Some(FieldStats {
                count: 10,
                min: 140,
                max: 210,
                avg: 150.0,
                p50: 150,
                p95: 180,
            }),
        },
    );

    let mut thresholds_override = SoakThresholds::default();
    thresholds_override.min_latency_improvement = Some(0.30);
    let current = SoakReport {
        started_unix: 0,
        finished_unix: 0,
        duration_secs: 0,
        options: SoakOptionsSnapshot {
            iterations: 1,
            keep_clones: false,
            report_path: "current".into(),
            workspace_dir: "current".into(),
            baseline_report: None,
            thresholds: thresholds_override,
        },
        iterations: 1,
        operations: HashMap::new(),
        timing: current_timing,
        fallback: FallbackSummary {
            counts: HashMap::new(),
            fake_to_real: 0,
            real_to_default: 0,
        },
        auto_disable: AutoDisableSummary {
            triggered: 0,
            recovered: 0,
        },
        cert_fp_events: 0,
        ip_pool: IpPoolSummary {
            selection_total: 0,
            selection_by_strategy: HashMap::new(),
            refresh_total: 0,
            refresh_success: 0,
            refresh_failure: 0,
            refresh_success_rate: 1.0,
        },
        proxy: ProxySummary {
            fallback_count: 0,
            recovered_count: 0,
            health_check_total: 0,
            health_check_success: 0,
            avg_health_check_latency_ms: None,
            system_proxy_detect_total: 0,
            system_proxy_detect_success: 0,
            system_proxy_detect_success_rate: 0.0,
        },
        totals: TotalsSummary {
            total_operations: 0,
            completed: 0,
            failed: 0,
            canceled: 0,
        },
        thresholds: ThresholdSummary::new(
            ThresholdCheck::at_least(1.0, 0.99),
            ThresholdCheck::at_most(0.0, 0.05),
            None,
            Some(ThresholdCheck::at_most(0.0, 0.0)),
            None,
        ),
        alerts: AlertsSummary::default(),
        comparison: None,
    };

    let summary = build_comparison_summary(Path::new("baseline.json"), &baseline, &current);
    let improvement = summary
        .git_clone_total_p50_improvement
        .expect("latency improvement should exist");
    assert!((improvement - 0.25).abs() < 1e-6);
    assert_eq!(summary.git_clone_total_p50_baseline, Some(200.0));
    assert_eq!(summary.git_clone_total_p50_current, Some(150.0));
    assert!(summary
        .regression_flags
        .iter()
        .any(|f| f.contains("latency_improvement")));
}

#[test]
fn run_marks_latency_not_applicable_without_baseline() {
    let base = std::env::temp_dir().join(format!("fwc-soak-latency-{}", Uuid::new_v4()));
    let report_path = base.join("report.json");
    let mut thresholds = SoakThresholds::default();
    thresholds.min_latency_improvement = Some(0.2);

    let opts = SoakOptions {
        iterations: 1,
        keep_clones: false,
        report_path: report_path.clone(),
        base_dir: Some(base.clone()),
        baseline_report: None,
        thresholds,
    };

    let report = run(opts).expect("soak run should succeed without baseline");
    let latency_check = report
        .thresholds
        .latency_improvement
        .expect("latency check should exist");
    assert!(!latency_check.pass);
    let reason = latency_check
        .details
        .expect("latency check should include details");
    assert!(reason.contains("baseline report not provided"));
    assert!(report
        .thresholds
        .failing_checks
        .contains(&"latency_improvement".to_string()));
    assert!(!report.thresholds.ready);

    let _ = fs::remove_dir_all(&base);
}
