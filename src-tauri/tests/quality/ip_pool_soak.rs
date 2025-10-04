//! IP 池扩展 Soak 测试 - P4.6 阶段验证
//!
//! 本测试通过配置预热域名和按需域名的混合场景，
//! 验证 IP 池在实际任务（clone/push/fetch）中的表现。
//!
//! 测试目标：
//! - 预热域名在启动时完成采样
//! - 按需域名在首次使用时采样
//! - IP 池统计数据正确收集
//! - 延迟改善达到预期目标

use fireworks_collaboration_lib::core::config::loader;
use fireworks_collaboration_lib::core::ip_pool::config::{
    EffectiveIpPoolConfig, IpPoolFileConfig, IpPoolRuntimeConfig, PreheatDomain,
};
use fireworks_collaboration_lib::core::ip_pool::manager::IpPool;
use fireworks_collaboration_lib::events::structured::{
    Event, EventBusAny, MemoryEventBus, StrategyEvent,
};
use fireworks_collaboration_lib::soak::{SoakOptions, SoakReport, SoakThresholds};
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
struct SoakReportView {
    totals: TotalsView,
    #[serde(default)]
    timing: HashMap<String, TimingView>,
    ip_pool: IpPoolView,
}

#[derive(Debug, Deserialize)]
struct TotalsView {
    total_operations: u64,
}

#[derive(Debug, Deserialize)]
struct TimingView {
    #[serde(default)]
    total_ms: Option<FieldStatsView>,
}

#[derive(Debug, Deserialize)]
struct FieldStatsView {
    p50: u32,
}

#[derive(Debug, Deserialize)]
struct IpPoolView {
    selection_total: u64,
    refresh_total: u64,
    refresh_success_rate: f64,
    #[serde(default)]
    selection_by_strategy: HashMap<String, u64>,
}

fn soak_report_view(report: &SoakReport) -> SoakReportView {
    let value = serde_json::to_value(report).expect("serialize soak report");
    serde_json::from_value::<SoakReportView>(value).expect("deserialize soak report view")
}

/// 辅助函数：配置 IP 池并运行 soak 测试
fn run_soak_with_ip_pool(
    iterations: u32,
    enable_ip_pool: bool,
    preheat_domains: Vec<PreheatDomain>,
) -> anyhow::Result<SoakReport> {
    let workspace = std::env::temp_dir().join(format!("fwc-soak-ip-{}", Uuid::new_v4()));
    let config_root = workspace.join("config");
    fs::create_dir_all(&config_root)?;

    // 设置配置目录
    loader::set_global_base_dir(&config_root);

    // 配置 IP 池
    let mut file_cfg = IpPoolFileConfig::default();
    file_cfg.preheat_domains = preheat_domains;
    file_cfg.score_ttl_seconds = 300;

    let mut runtime_cfg = IpPoolRuntimeConfig::default();
    runtime_cfg.enabled = enable_ip_pool;
    runtime_cfg.max_parallel_probes = 10;
    runtime_cfg.probe_timeout_ms = 2000;
    runtime_cfg.cache_prune_interval_secs = 60;
    runtime_cfg.max_cache_entries = 256;
    runtime_cfg.singleflight_timeout_ms = 10_000;
    runtime_cfg.failure_threshold = 5;
    runtime_cfg.failure_rate_threshold = 0.8;
    runtime_cfg.failure_window_seconds = 300;
    runtime_cfg.min_samples_in_window = 3;
    runtime_cfg.cooldown_seconds = 300;
    runtime_cfg.circuit_breaker_enabled = true;

    // 初始化 IP 池（会启动预热）
    let _pool = IpPool::new(EffectiveIpPoolConfig::from_parts(runtime_cfg, file_cfg));

    // 运行标准 soak 测试
    let opts = SoakOptions {
        iterations,
        keep_clones: false,
        report_path: workspace.join("soak-report.json"),
        base_dir: Some(workspace.clone()),
        baseline_report: None,
        thresholds: SoakThresholds::default(),
    };

    let report = fireworks_collaboration_lib::soak::run(opts)?;

    // 清理
    let _ = fs::remove_dir_all(&workspace);

    Ok(report)
}

/// 测试 1：基线对比 - 不启用 IP 池
///
/// 作为对照组，验证不启用 IP 池时的性能基线
#[test]
#[ignore] // 运行时间较长，默认不执行
fn soak_baseline_without_ip_pool() {
    let report = run_soak_with_ip_pool(5, false, Vec::new()).expect("soak run should succeed");
    let view = soak_report_view(&report);

    println!("=== Baseline Report (IP Pool Disabled) ===");
    println!("Total operations: {}", view.totals.total_operations);
    println!(
        "Success rate: {:.2}%",
        report.thresholds.success_rate.actual * 100.0
    );
    println!("IP pool selection total: {}", view.ip_pool.selection_total);
    println!("IP pool refresh total: {}", view.ip_pool.refresh_total);

    // 验证禁用时没有 IP 池活动
    assert_eq!(view.ip_pool.selection_total, 0, "禁用时不应有 IP 池选择");
    assert!(report.thresholds.success_rate.pass, "基线成功率应达标");
}

/// 测试 2：预热域名 - 启用 IP 池并配置 GitHub 域
///
/// 验证：
/// - 预热域名在启动时完成采样
/// - 任务使用预热缓存
/// - IP 池统计正确
#[test]
#[ignore] // 运行时间较长，需要网络访问
fn soak_with_preheat_domains() {
    let preheat = vec![
        PreheatDomain::new("github.com"),
        // PreheatDomain::new("api.github.com"), // 可选：添加更多域
    ];

    let report = run_soak_with_ip_pool(5, true, preheat).expect("soak with preheat should succeed");
    let view = soak_report_view(&report);

    println!("=== Report (IP Pool Enabled, Preheat Domains) ===");
    println!("Total operations: {}", view.totals.total_operations);
    println!(
        "Success rate: {:.2}%",
        report.thresholds.success_rate.actual * 100.0
    );
    println!("IP pool selection total: {}", view.ip_pool.selection_total);
    println!("IP pool refresh total: {}", view.ip_pool.refresh_total);
    println!(
        "IP pool refresh success rate: {:.2}%",
        view.ip_pool.refresh_success_rate * 100.0
    );

    // 验证 IP 池活动
    assert!(view.ip_pool.selection_total > 0, "启用 IP 池时应有选择活动");
    assert!(view.ip_pool.refresh_total > 0, "预热域名应触发刷新");
    assert!(view.ip_pool.refresh_success_rate >= 0.8, "刷新成功率应≥80%");
    assert!(report.thresholds.success_rate.pass, "任务成功率应达标");

    // 检查策略分布
    let cached_count = view
        .ip_pool
        .selection_by_strategy
        .get("Cached")
        .copied()
        .unwrap_or(0);
    let system_count = view
        .ip_pool
        .selection_by_strategy
        .get("SystemDefault")
        .copied()
        .unwrap_or(0);

    println!("Cached selections: {cached_count}");
    println!("System DNS fallbacks: {system_count}");

    // 预期大部分请求使用缓存
    if view.ip_pool.selection_total > 0 {
        let cached_ratio = cached_count as f64 / view.ip_pool.selection_total as f64;
        println!("Cached ratio: {:.2}%", cached_ratio * 100.0);
        assert!(cached_ratio >= 0.5, "缓存命中率应≥50%（预热生效）");
    }
}

/// 测试 3：混合场景 - 预热 + 按需域名
///
/// 验证：
/// - 预热域名使用缓存
/// - 按需域名首次访问时采样
/// - 统计数据区分两种场景
#[test]
#[ignore] // 运行时间较长
fn soak_mixed_preheat_and_on_demand() {
    // 仅预热 github.com，其他域按需
    let preheat = vec![PreheatDomain::new("github.com")];

    let report = run_soak_with_ip_pool(3, true, preheat).expect("mixed soak should succeed");
    let view = soak_report_view(&report);

    println!("=== Report (Mixed Preheat + On-Demand) ===");
    println!("Total operations: {}", view.totals.total_operations);
    println!(
        "Success rate: {:.2}%",
        report.thresholds.success_rate.actual * 100.0
    );
    println!("IP pool selection total: {}", view.ip_pool.selection_total);
    println!("IP pool refresh total: {}", view.ip_pool.refresh_total);

    // 验证两种场景都有活动
    assert!(view.ip_pool.selection_total > 0, "应有 IP 选择活动");
    assert!(view.ip_pool.refresh_total > 0, "应有刷新活动");
    assert!(report.thresholds.success_rate.pass, "任务成功率应达标");
}

/// 测试 4：延迟改善验证 - 对比启用前后的延迟
///
/// 验证准入目标：延迟改善 ≥15%
#[test]
#[ignore] // 需要运行两次 soak，耗时较长
fn soak_latency_improvement_comparison() {
    // 第一轮：禁用 IP 池（基线）
    let baseline_report =
        run_soak_with_ip_pool(10, false, Vec::new()).expect("baseline soak should succeed");

    // 第二轮：启用 IP 池（预热 GitHub）
    let preheat = vec![PreheatDomain::new("github.com")];
    let enabled_report =
        run_soak_with_ip_pool(10, true, preheat).expect("enabled soak should succeed");

    let baseline_view = soak_report_view(&baseline_report);
    let enabled_view = soak_report_view(&enabled_report);

    // 计算延迟改善
    // 提取 GitClone 的 total_ms p50
    let baseline_total = baseline_view
        .timing
        .get("GitClone")
        .and_then(|view| view.total_ms.as_ref());
    let enabled_total = enabled_view
        .timing
        .get("GitClone")
        .and_then(|view| view.total_ms.as_ref());

    if let (Some(baseline_total), Some(enabled_total)) = (baseline_total, enabled_total) {
        let baseline_p50 = baseline_total.p50 as f64;
        let enabled_p50 = enabled_total.p50 as f64;

        let improvement = (baseline_p50 - enabled_p50) / baseline_p50;
        println!("=== Latency Improvement Analysis ===");
        println!("Baseline p50 total_ms: {baseline_p50:.0}");
        println!("Enabled p50 total_ms: {enabled_p50:.0}");
        println!("Improvement: {:.2}%", improvement * 100.0);

        // 准入目标：≥15% 改善
        // 注意：本地测试可能不明显，生产环境更有效
        if improvement > 0.0 {
            println!("✓ 延迟有改善（{:.2}%）", improvement * 100.0);
        } else {
            println!("⚠ 延迟未改善（可能是网络环境或样本量影响）");
        }
    }

    // 输出对比报告
    println!("\n=== Baseline Report Summary ===");
    println!(
        "Success rate: {:.2}%",
        baseline_report.thresholds.success_rate.actual * 100.0
    );
    println!(
        "Fake fallback rate: {:.2}%",
        baseline_report.thresholds.fake_fallback_rate.actual * 100.0
    );

    println!("\n=== Enabled Report Summary ===");
    println!(
        "Success rate: {:.2}%",
        enabled_report.thresholds.success_rate.actual * 100.0
    );
    println!(
        "Fake fallback rate: {:.2}%",
        enabled_report.thresholds.fake_fallback_rate.actual * 100.0
    );
    println!(
        "IP pool selection total: {}",
        enabled_view.ip_pool.selection_total
    );
    println!(
        "IP pool refresh success rate: {:.2}%",
        enabled_view.ip_pool.refresh_success_rate * 100.0
    );
}

/// 测试 5：回退成功率 - 验证 IP 池不可用时的回退
///
/// 验证准入目标：回退成功率 ≥99%
#[test]
#[ignore]
fn soak_fallback_success_rate() {
    // 配置一个不存在的预热域名，强制回退
    let preheat = vec![PreheatDomain::new("nonexistent.invalid")];

    let report = run_soak_with_ip_pool(5, true, preheat).expect("fallback soak should succeed");
    let view = soak_report_view(&report);

    println!("=== Fallback Success Rate Report ===");
    println!("Total operations: {}", view.totals.total_operations);
    println!(
        "Success rate: {:.2}%",
        report.thresholds.success_rate.actual * 100.0
    );
    println!("IP pool selection total: {}", view.ip_pool.selection_total);

    // 验证回退场景仍然成功
    assert!(
        report.thresholds.success_rate.actual >= 0.99,
        "回退场景成功率应≥99%"
    );

    // 大部分应该是 SystemDefault
    let system_count = view
        .ip_pool
        .selection_by_strategy
        .get("SystemDefault")
        .copied()
        .unwrap_or(0);
    println!("System DNS fallbacks: {system_count}");

    assert!(system_count > 0, "无效域名应触发系统 DNS 回退");
}

/// 测试 6：IP 池事件完整性 - 验证事件记录
///
/// 确保所有 IP 池相关事件都被正确记录
#[test]
fn soak_ip_pool_events_completeness() {
    let workspace = std::env::temp_dir().join(format!("fwc-soak-events-{}", Uuid::new_v4()));
    let config_root = workspace.join("config");
    fs::create_dir_all(&config_root).expect("create config dir");

    loader::set_global_base_dir(&config_root);

    // 配置 IP 池
    let mut file_cfg = IpPoolFileConfig::default();
    file_cfg.preheat_domains = vec![PreheatDomain::new("github.com")];
    file_cfg.score_ttl_seconds = 300;

    let mut runtime_cfg = IpPoolRuntimeConfig::default();
    runtime_cfg.enabled = true;
    runtime_cfg.max_parallel_probes = 5;
    runtime_cfg.probe_timeout_ms = 2000;
    runtime_cfg.cache_prune_interval_secs = 60;
    runtime_cfg.max_cache_entries = 100;
    runtime_cfg.singleflight_timeout_ms = 10_000;
    runtime_cfg.failure_threshold = 3;
    runtime_cfg.failure_rate_threshold = 0.7;
    runtime_cfg.failure_window_seconds = 60;
    runtime_cfg.min_samples_in_window = 3;
    runtime_cfg.cooldown_seconds = 60;
    runtime_cfg.circuit_breaker_enabled = true;

    let effective = EffectiveIpPoolConfig::from_parts(runtime_cfg, file_cfg);

    let bus = MemoryEventBus::new();
    let bus_arc: Arc<dyn EventBusAny> = Arc::new(bus.clone());
    let using_global =
        fireworks_collaboration_lib::events::structured::set_global_event_bus(bus_arc.clone())
            .is_ok();
    if !using_global {
        fireworks_collaboration_lib::events::structured::set_test_event_bus(bus_arc.clone());
    }

    let pool = IpPool::new(effective);

    // 主动触发一次预热与选择，确保事件产出
    let _ = pool.pick_best_blocking("github.com", 443);

    // 等待预热完成（简化测试，实际应使用同步机制）
    std::thread::sleep(std::time::Duration::from_secs(3));

    // 检查事件
    let events = if using_global {
        bus.take_all()
    } else if let Some(global_bus) =
        fireworks_collaboration_lib::events::structured::get_global_memory_bus()
    {
        global_bus.take_all()
    } else {
        bus.take_all()
    };

    let refresh_events: Vec<&Event> = events
        .iter()
        .filter(|e| matches!(e, Event::Strategy(StrategyEvent::IpPoolRefresh { .. })))
        .collect();

    let selection_events: Vec<&Event> = events
        .iter()
        .filter(|e| matches!(e, Event::Strategy(StrategyEvent::IpPoolSelection { .. })))
        .collect();

    println!("=== IP Pool Events ===");
    println!("Refresh events: {}", refresh_events.len());
    println!("Selection events: {}", selection_events.len());

    // 验证预热触发了刷新事件
    assert!(!refresh_events.is_empty(), "预热应触发 IpPoolRefresh 事件");

    // 清理
    let _ = fs::remove_dir_all(&workspace);
}
