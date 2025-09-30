#![cfg(not(feature = "tauri-app"))]
//! 聚合测试：IpPool 管理器核心行为（P4.2 回归）
//! -------------------------------------------------
//! 迁移来源：`core::ip_pool::manager` 内部单元测试；按模块化策略拆分至 integration suite。
//! 覆盖范围：
//!   * 选择逻辑：缓存命中 / 采样回退 / 单飞控制
//!   * 配置更新：历史路径切换 / app 配置加载
//!   * 维护流程：过期裁剪 / 容量限制 / 预热豁免
//!   * 指标曝光：Outcome 统计
//! 辅助：复用 `tests/common/ip_pool.rs` 中的构造方法，减少重复样板代码。

#[path = "../common/mod.rs"]
mod common;

use crate::common::test_env::init_test_env;

#[ctor::ctor]
fn __init_env() {
    init_test_env();
}

// ---------------- section_selection_behaviour ----------------
mod section_selection_behaviour {
    use super::common::prelude::*;
    use fireworks_collaboration_lib::core::ip_pool::config::IpPoolSourceToggle;
    use fireworks_collaboration_lib::core::ip_pool::IpScoreCache;
    use fireworks_collaboration_lib::core::ip_pool::{IpPool, IpSource};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use tokio::net::TcpListener;
    use tokio::time::{sleep, Duration};

    #[tokio::test]
    async fn pick_best_falls_back_when_disabled() {
        let pool = IpPool::default();
        let selection = pool.pick_best("github.com", 443).await;
        assert!(selection.is_system_default());
        assert!(selection.selected().is_none());
    }

    #[tokio::test]
    async fn pick_best_uses_cache_when_enabled() {
        let cfg = enabled_config();
        let cache = IpScoreCache::new();
        let now_ms = epoch_ms();
        let stat = make_latency_stat(
            [1, 1, 1, 1],
            443,
            15,
            IpSource::Builtin,
            Some(now_ms - 1_000),
            Some(now_ms + 60_000),
        );
        cache_best(&cache, "github.com", 443, stat.clone());

        let pool = IpPool::with_cache(cfg, cache);
        let selection = pool.pick_best("github.com", 443).await;
        assert!(!selection.is_system_default());
        let chosen = selection.selected().expect("cached selection");
        assert_eq!(chosen.candidate.address, stat.candidate.address);
    }

    #[tokio::test]
    async fn pick_best_returns_system_default_when_sampling_fails() {
        let mut cfg = enabled_config();
        cfg.runtime.sources = IpPoolSourceToggle {
            builtin: false,
            dns: false,
            history: false,
            user_static: false,
            fallback: false,
        };
        let pool = IpPool::with_cache(cfg, IpScoreCache::new());
        let selection = pool.pick_best("missing.test", 443).await;
        assert!(selection.is_system_default());
        assert!(pool.cache().get("missing.test", 443).is_none());
        assert!(pool.history().get("missing.test", 443).is_none());
    }

    #[tokio::test]
    async fn on_demand_sampling_uses_user_static_candidate() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let accept_task = tokio::spawn(async move {
            if let Ok((stream, _)) = listener.accept().await {
                drop(stream);
            }
        });

        let cfg = user_static_only_config("local.test", addr.ip(), addr.port());
        let pool = IpPool::with_cache(cfg, IpScoreCache::new());

        let selection = pool.pick_best("local.test", addr.port()).await;
        assert!(!selection.is_system_default());
        let stat = selection.selected().expect("latency stat");
        assert_eq!(stat.candidate.address, addr.ip());
        accept_task.await.unwrap();

        // 再次调用应直接命中缓存。
        let cached = pool.pick_best("local.test", addr.port()).await;
        assert_eq!(
            cached.selected().expect("cached stat").candidate.address,
            addr.ip()
        );
    }

    #[tokio::test]
    async fn ttl_expiry_triggers_resample() {
        let listener1 = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener1.local_addr().unwrap();
        let counter = Arc::new(AtomicUsize::new(0));
        let accept_counter = counter.clone();
        let accept_task = tokio::spawn(async move {
            if let Ok((stream, _)) = listener1.accept().await {
                accept_counter.fetch_add(1, Ordering::SeqCst);
                drop(stream);
            }
        });

        let mut cfg = user_static_only_config("ttl.test", addr.ip(), addr.port());
        cfg.runtime.cache_prune_interval_secs = 1;
        cfg.runtime.max_cache_entries = 16;
        cfg.file.score_ttl_seconds = 1;
        let pool = IpPool::with_cache(cfg, IpScoreCache::new());

        let first = pool.pick_best("ttl.test", addr.port()).await;
        assert!(!first.is_system_default());
        accept_task.await.unwrap();
        assert_eq!(counter.load(Ordering::SeqCst), 1);

        sleep(Duration::from_millis(1_200)).await;

        let listener2 = TcpListener::bind(addr).await.unwrap();
        let accept_counter2 = counter.clone();
        let accept_task2 = tokio::spawn(async move {
            if let Ok((stream, _)) = listener2.accept().await {
                accept_counter2.fetch_add(1, Ordering::SeqCst);
                drop(stream);
            }
        });

        let second = pool.pick_best("ttl.test", addr.port()).await;
        assert!(!second.is_system_default());
        accept_task2.await.unwrap();
        assert_eq!(counter.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn single_flight_prevents_duplicate_sampling() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let counter = Arc::new(AtomicUsize::new(0));
        let accept_counter = counter.clone();
        let accept_task = tokio::spawn(async move {
            if let Ok((stream, _)) = listener.accept().await {
                accept_counter.fetch_add(1, Ordering::SeqCst);
                drop(stream);
            }
        });

        let cfg = user_static_only_config("flight.test", addr.ip(), addr.port());
        let pool = Arc::new(IpPool::with_cache(cfg, IpScoreCache::new()));
        let pool_a = pool.clone();
        let pool_b = pool.clone();

        let (a, b) = tokio::join!(
            async move { pool_a.pick_best("flight.test", addr.port()).await },
            async move { pool_b.pick_best("flight.test", addr.port()).await },
        );

        assert!(!a.is_system_default());
        assert!(!b.is_system_default());
        accept_task.await.unwrap();
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }
}

// ---------------- section_config_mutation ----------------
mod section_config_mutation {
    use super::common::fixtures;
    use super::common::prelude::*;
    use fireworks_collaboration_lib::core::config::loader as cfg_loader;
    use fireworks_collaboration_lib::core::config::model::AppConfig;
    use fireworks_collaboration_lib::core::ip_pool::config::{save_file_at, IpPoolFileConfig};
    use fireworks_collaboration_lib::core::ip_pool::{load_effective_config_at, IpPool, IpSource};
    use std::fs;
    use std::net::{IpAddr, Ipv4Addr};

    #[tokio::test]
    async fn update_config_applies_runtime_changes() {
        let mut pool = IpPool::default();
        let stat = make_latency_stat([9, 9, 9, 9], 443, 7, IpSource::Builtin, None, None);
        cache_best(pool.cache(), "github.com", 443, stat.clone());
        let initial = pool.pick_best("github.com", 443).await;
        assert!(initial.is_system_default());

        let new_cfg = enabled_config();
        pool.update_config(new_cfg);
        let updated = pool.pick_best("github.com", 443).await;
        assert!(!updated.is_system_default());
        assert_eq!(
            updated.selected().map(|s| s.candidate.address),
            Some(IpAddr::V4(Ipv4Addr::new(9, 9, 9, 9)))
        );
    }

    #[test]
    fn custom_history_path_is_respected() {
        let base = fixtures::create_empty_dir();
        let history_path = base.join("nested").join("custom-history.json");
        let mut cfg = enabled_config();
        cfg.runtime.history_path = Some(history_path.to_string_lossy().into());
        let pool = IpPool::new(cfg);

        let stat = make_latency_stat(
            [2, 2, 2, 2],
            443,
            10,
            IpSource::Builtin,
            Some(epoch_ms() - 1_000),
            Some(epoch_ms() + 1_000),
        );
        let record = history_record("custom.test", 443, &stat);
        pool.history().upsert(record).unwrap();
        assert!(history_path.exists());
        let content = fs::read_to_string(&history_path).unwrap();
        assert!(content.contains("custom.test"));
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn update_config_reloads_history_path() {
        let base = fixtures::create_empty_dir();
        let history_one = base.join("history-one.json");
        let history_two = base.join("history-two.json");

        let mut cfg = enabled_config();
        cfg.runtime.history_path = Some(history_one.to_string_lossy().into());
        let mut pool = IpPool::new(cfg.clone());

        let stat_one = make_latency_stat(
            [3, 3, 3, 3],
            443,
            11,
            IpSource::Builtin,
            Some(epoch_ms() - 2_000),
            Some(epoch_ms() + 2_000),
        );
        let record_one = history_record("first.test", 443, &stat_one);
        pool.history().upsert(record_one).unwrap();
        assert!(history_one.exists());

        cfg.runtime.history_path = Some(history_two.to_string_lossy().into());
        pool.update_config(cfg);

        let stat_two = make_latency_stat(
            [4, 4, 4, 4],
            443,
            12,
            IpSource::Builtin,
            Some(epoch_ms() - 1_000),
            Some(epoch_ms() + 3_000),
        );
        let record_two = history_record("second.test", 443, &stat_two);
        pool.history().upsert(record_two).unwrap();

        let file_one = fs::read_to_string(&history_one).unwrap();
        let file_two = fs::read_to_string(&history_two).unwrap();
        assert!(file_one.contains("first.test"));
        assert!(file_two.contains("second.test"));
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn load_effective_config_with_custom_base_dir() {
        let base = fixtures::create_empty_dir();
        let mut file_cfg = IpPoolFileConfig::default();
        file_cfg.preheat_domains.push(
            fireworks_collaboration_lib::core::ip_pool::config::PreheatDomain::new("github.com"),
        );
        file_cfg.score_ttl_seconds = 600;
        save_file_at(&file_cfg, &base).expect("save ip-config.json");

        let mut app_cfg = AppConfig::default();
        app_cfg.ip_pool.enabled = true;
        app_cfg.ip_pool.max_parallel_probes = 16;

        let effective = load_effective_config_at(&app_cfg, &base).expect("load config");
        assert!(effective.runtime.enabled);
        assert_eq!(effective.runtime.max_parallel_probes, 16);
        assert_eq!(effective.file.preheat_domains.len(), 1);
        assert_eq!(effective.file.score_ttl_seconds, 600);
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn from_app_config_uses_runtime_defaults() {
        let base = fixtures::create_empty_dir();
        cfg_loader::set_global_base_dir(&base);

        let mut app_cfg = AppConfig::default();
        app_cfg.ip_pool.enabled = true;
        let pool = IpPool::from_app_config(&app_cfg).expect("ip pool from app config");
        assert!(pool.is_enabled());
        assert!(pool.file_config().preheat_domains.is_empty());

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn load_effective_config_at_errors_on_invalid_file() {
        let base = fixtures::create_empty_dir();
        let cfg_dir = base.join("config");
        std::fs::create_dir_all(&cfg_dir).unwrap();
        let invalid = cfg_dir.join("ip-config.json");
        fs::write(&invalid, b"not-json").unwrap();

        let app_cfg = AppConfig::default();
        let result = load_effective_config_at(&app_cfg, &base);
        assert!(result.is_err());
        let _ = fs::remove_dir_all(&base);
    }
}

// ---------------- section_cache_maintenance ----------------
mod section_cache_maintenance {
    use super::common::prelude::*;
    use fireworks_collaboration_lib::core::ip_pool::config::PreheatDomain;
    use fireworks_collaboration_lib::core::ip_pool::IpScoreCache;
    use fireworks_collaboration_lib::core::ip_pool::{IpPool, IpSource};

    #[test]
    fn prune_cache_removes_expired_entries_and_history() {
        let cfg = enabled_config();
        let pool = IpPool::with_cache(cfg, IpScoreCache::new());
        let now_ms = epoch_ms();

        let expired = make_latency_stat(
            [10, 0, 0, 1],
            443,
            20,
            IpSource::Builtin,
            Some(now_ms - 10_000),
            Some(now_ms - 1_000),
        );
        cache_best(pool.cache(), "expire.test", 443, expired.clone());
        let record = history_record("expire.test", 443, &expired);
        pool.history().upsert(record).unwrap();

        pool.maintenance_tick_at(now_ms);
        assert!(pool.cache().get("expire.test", 443).is_none());
        assert!(pool.history().get("expire.test", 443).is_none());
    }

    #[test]
    fn enforce_cache_capacity_limits_entries() {
        let mut cfg = enabled_config();
        cfg.runtime.max_cache_entries = 1;
        let pool = IpPool::with_cache(cfg, IpScoreCache::new());
        let now_ms = epoch_ms();

        let older = make_latency_stat(
            [11, 0, 0, 1],
            443,
            40,
            IpSource::Builtin,
            Some(now_ms - 50_000),
            Some(now_ms + 50_000),
        );
        let newer = make_latency_stat(
            [11, 0, 0, 2],
            443,
            30,
            IpSource::Builtin,
            Some(now_ms - 10_000),
            Some(now_ms + 50_000),
        );
        cache_best(pool.cache(), "old.example", 443, older.clone());
        cache_best(pool.cache(), "new.example", 443, newer.clone());
        pool.history()
            .upsert(history_record("old.example", 443, &older))
            .unwrap();
        pool.history()
            .upsert(history_record("new.example", 443, &newer))
            .unwrap();

        pool.maintenance_tick_at(now_ms + 1_000);
        assert!(pool.cache().get("old.example", 443).is_none());
        assert!(pool.history().get("old.example", 443).is_none());
        assert!(pool.cache().get("new.example", 443).is_some());
        assert!(pool.history().get("new.example", 443).is_some());
    }

    #[test]
    fn enforce_cache_capacity_preserves_preheat_entries() {
        let mut cfg = enabled_config();
        cfg.runtime.max_cache_entries = 1;
        cfg.file
            .preheat_domains
            .push(PreheatDomain::new("keep.test"));
        let pool = IpPool::with_cache(cfg, IpScoreCache::new());
        let now_ms = epoch_ms();

        let keep = make_latency_stat(
            [12, 0, 0, 1],
            443,
            18,
            IpSource::Builtin,
            Some(now_ms - 5_000),
            Some(now_ms + 60_000),
        );
        let drop_old = make_latency_stat(
            [13, 0, 0, 1],
            443,
            30,
            IpSource::Builtin,
            Some(now_ms - 10_000),
            Some(now_ms + 60_000),
        );
        let drop_new = make_latency_stat(
            [14, 0, 0, 1],
            443,
            16,
            IpSource::Builtin,
            Some(now_ms - 1_000),
            Some(now_ms + 60_000),
        );

        cache_best(pool.cache(), "keep.test", 443, keep.clone());
        cache_best(pool.cache(), "drop-old.test", 443, drop_old.clone());
        cache_best(pool.cache(), "drop-new.test", 443, drop_new.clone());
        pool.history()
            .upsert(history_record("drop-old.test", 443, &drop_old))
            .unwrap();
        pool.history()
            .upsert(history_record("drop-new.test", 443, &drop_new))
            .unwrap();
        pool.history()
            .upsert(history_record("keep.test", 443, &keep))
            .unwrap();

        pool.maintenance_tick_at(now_ms + 2_000);
        assert!(pool.cache().get("drop-old.test", 443).is_none());
        assert!(pool.history().get("drop-old.test", 443).is_none());
        assert!(pool.cache().get("drop-new.test", 443).is_some());
        assert!(pool.history().get("drop-new.test", 443).is_some());
        assert!(pool.cache().get("keep.test", 443).is_some());
        assert!(pool.history().get("keep.test", 443).is_some());
    }

    #[test]
    fn maybe_prune_cache_skips_preheat_entries() {
        let mut cfg = enabled_config();
        cfg.runtime.cache_prune_interval_secs = 1;
        cfg.file
            .preheat_domains
            .push(PreheatDomain::new("keep.test"));
        let pool = IpPool::with_cache(cfg, IpScoreCache::new());
        let now_ms = epoch_ms();

        // Preheat entry is expired but should still be kept in cache (not history due to expiry)
        let keep = make_latency_stat(
            [21, 0, 0, 1],
            443,
            19,
            IpSource::Builtin,
            Some(now_ms - 10_000),
            Some(now_ms - 1),
        );
        let drop = make_latency_stat(
            [22, 0, 0, 1],
            443,
            33,
            IpSource::Builtin,
            Some(now_ms - 9_000),
            Some(now_ms - 1),
        );
        cache_best(pool.cache(), "keep.test", 443, keep.clone());
        cache_best(pool.cache(), "drop.test", 443, drop.clone());
        pool.history()
            .upsert(history_record("drop.test", 443, &drop))
            .unwrap();
        pool.history()
            .upsert(history_record("keep.test", 443, &keep))
            .unwrap();

        pool.maintenance_tick_at(now_ms + 5_000);
        assert!(pool.cache().get("drop.test", 443).is_none());
        assert!(pool.history().get("drop.test", 443).is_none());
        // Preheat entry should be kept in cache even when expired
        assert!(pool.cache().get("keep.test", 443).is_some());
        // But expired preheat entries ARE removed from history to allow refresh
        assert!(pool.history().get("keep.test", 443).is_none());
    }
}

// ---------------- section_outcome_metrics ----------------
mod section_outcome_metrics {
    use super::common::prelude::*;
    use fireworks_collaboration_lib::core::ip_pool::{
        IpOutcome, IpPool, IpScoreCache, IpSelection, IpSource,
    };

    #[test]
    fn report_outcome_records_success_and_failure() {
        let cfg = enabled_config();
        let cache = IpScoreCache::new();
        let stat = make_latency_stat(
            [20, 20, 20, 20],
            443,
            12,
            IpSource::Builtin,
            Some(epoch_ms()),
            Some(epoch_ms() + 60_000),
        );
        cache_best(&cache, "outcome.test", 443, stat.clone());
        let pool = IpPool::with_cache(cfg, cache);
        let selection = IpSelection::from_cached("outcome.test", 443, stat);

        pool.report_outcome(&selection, IpOutcome::Success);
        pool.report_outcome(&selection, IpOutcome::Failure);

        let metrics = pool
            .outcome_metrics("outcome.test", 443)
            .expect("outcome metrics");
        assert_eq!(metrics.success, 1);
        assert_eq!(metrics.failure, 1);
        assert!(metrics.last_outcome_ms > 0);
    }
}

// ---------------- section_event_emission ----------------
mod section_event_emission {
    use super::common::prelude::*;
    use fireworks_collaboration_lib::core::ip_pool::config::EffectiveIpPoolConfig;
    use fireworks_collaboration_lib::core::ip_pool::events::{
        emit_ip_pool_auto_disable, emit_ip_pool_auto_enable, emit_ip_pool_cidr_filter,
        emit_ip_pool_config_update, emit_ip_pool_ip_recovered, emit_ip_pool_ip_tripped,
        emit_ip_pool_refresh, emit_ip_pool_selection,
    };
    use fireworks_collaboration_lib::core::ip_pool::{
        IpPool, IpScoreCache, IpSelectionStrategy, IpSource,
    };
    use fireworks_collaboration_lib::events::structured::StrategyEvent;
    use tokio::net::TcpListener;
    use uuid::Uuid;

    fn expect_single_event<F>(events: Vec<StrategyEvent>, assert_fn: F)
    where
        F: FnOnce(&StrategyEvent),
    {
        assert_eq!(events.len(), 1, "expected a single strategy event");
        assert_fn(&events[0]);
    }

    #[test]
    fn emit_ip_pool_selection_includes_strategy_and_latency() {
        let bus = install_test_event_bus();
        let task_id = Uuid::new_v4();
        let stat = make_latency_stat(
            [1, 2, 3, 4],
            443,
            25,
            IpSource::Builtin,
            Some(epoch_ms()),
            Some(epoch_ms() + 60_000),
        );

        emit_ip_pool_selection(
            task_id,
            "github.com",
            443,
            IpSelectionStrategy::Cached,
            Some(&stat),
            3,
        );

        expect_single_event(bus.strategy_events(), |event| match event {
            StrategyEvent::IpPoolSelection {
                id,
                domain,
                port,
                strategy,
                source,
                latency_ms,
                candidates_count,
            } => {
                assert_eq!(id, &task_id.to_string());
                assert_eq!(domain, "github.com");
                assert_eq!(*port, 443);
                assert_eq!(strategy, "Cached");
                assert!(source.is_some());
                assert_eq!(*latency_ms, Some(25));
                assert_eq!(*candidates_count, 3);
            }
            other => panic!("unexpected event variant: {:?}", other),
        });
    }

    #[test]
    fn emit_ip_pool_refresh_includes_latency_range() {
        let bus = install_test_event_bus();
        let task_id = Uuid::new_v4();
        let stats = vec![
            make_latency_stat(
                [1, 1, 1, 1],
                443,
                10,
                IpSource::Builtin,
                Some(epoch_ms()),
                Some(epoch_ms() + 60_000),
            ),
            make_latency_stat(
                [2, 2, 2, 2],
                443,
                30,
                IpSource::Dns,
                Some(epoch_ms()),
                Some(epoch_ms() + 60_000),
            ),
        ];

        emit_ip_pool_refresh(task_id, "example.com", true, &stats, "test".to_string());

        expect_single_event(bus.strategy_events(), |event| match event {
            StrategyEvent::IpPoolRefresh {
                id,
                domain,
                success,
                candidates_count,
                min_latency_ms,
                max_latency_ms,
                reason,
            } => {
                assert_eq!(id, &task_id.to_string());
                assert_eq!(domain, "example.com");
                assert!(success);
                assert_eq!(*candidates_count, 2);
                assert_eq!(*min_latency_ms, Some(10));
                assert_eq!(*max_latency_ms, Some(30));
                assert_eq!(reason, "test");
            }
            other => panic!("unexpected event variant: {:?}", other),
        });
    }

    #[test]
    fn emit_ip_pool_refresh_handles_empty_candidates() {
        let bus = install_test_event_bus();
        let task_id = Uuid::new_v4();
        emit_ip_pool_refresh(
            task_id,
            "empty.com",
            false,
            &[],
            "no_candidates".to_string(),
        );

        expect_single_event(bus.strategy_events(), |event| match event {
            StrategyEvent::IpPoolRefresh {
                success,
                candidates_count,
                min_latency_ms,
                max_latency_ms,
                reason,
                ..
            } => {
                assert!(!success);
                assert_eq!(*candidates_count, 0);
                assert!(min_latency_ms.is_none());
                assert!(max_latency_ms.is_none());
                assert_eq!(reason, "no_candidates");
            }
            other => panic!("unexpected event variant: {:?}", other),
        });
    }

    #[test]
    fn auto_disable_extends_without_duplicate_events() {
        let bus = install_test_event_bus();
        let mut cfg = enabled_config();
        cfg.runtime.enabled = true;
        cfg.runtime.max_cache_entries = 8;
        let pool = IpPool::new(cfg);

        // Prime cache with a candidate to observe selection behavior before/after auto-disable.
        let now_ms = epoch_ms();
        let stat = make_latency_stat(
            [10, 0, 0, 1],
            443,
            25,
            IpSource::Builtin,
            Some(now_ms),
            Some(now_ms + 60_000),
        );
        cache_best(pool.cache(), "auto.test", 443, stat);

        let initial = pool.pick_best_blocking("auto.test", 443);
        assert!(!initial.is_system_default());

        pool.set_auto_disabled("test reason", 1_000);
        let first_until = pool.auto_disabled_until().expect("pool should be disabled");

        // Second call should extend duration but not emit duplicate events.
        pool.set_auto_disabled("extended reason", 5_000);
        let extended_until = pool
            .auto_disabled_until()
            .expect("pool should remain disabled");
        assert!(extended_until >= first_until);

        let disabled = pool.pick_best_blocking("auto.test", 443);
        assert!(disabled.is_system_default());

        assert!(pool.clear_auto_disabled());
        // Second clear should be a no-op and not emit another event.
        assert!(!pool.clear_auto_disabled());

        let restored = pool.pick_best_blocking("auto.test", 443);
        assert!(!restored.is_system_default());

        let events = bus.strategy_events();
        let disable_events: Vec<_> = events
            .iter()
            .filter_map(|event| match event {
                StrategyEvent::IpPoolAutoDisable { reason, until_ms } => {
                    Some((reason.clone(), *until_ms))
                }
                _ => None,
            })
            .collect();
        assert_eq!(
            disable_events.len(),
            1,
            "duplicate disable events detected: {:?}",
            disable_events
        );
        assert_eq!(disable_events[0].0, "test reason");

        let enable_events: Vec<_> = events
            .iter()
            .filter(|event| matches!(event, StrategyEvent::IpPoolAutoEnable {}))
            .collect();
        assert_eq!(enable_events.len(), 1);
    }

    #[test]
    fn emit_ip_pool_cidr_filter_publishes_event() {
        let bus = install_test_event_bus();
        emit_ip_pool_cidr_filter(
            "192.168.1.1".parse().unwrap(),
            "blacklist",
            "192.168.1.0/24",
        );

        expect_single_event(bus.strategy_events(), |event| match event {
            StrategyEvent::IpPoolCidrFilter {
                ip,
                list_type,
                cidr,
            } => {
                assert_eq!(ip, "192.168.1.1");
                assert_eq!(list_type, "blacklist");
                assert_eq!(cidr, "192.168.1.0/24");
            }
            other => panic!("unexpected event variant: {:?}", other),
        });
    }

    #[test]
    fn emit_ip_pool_ip_tripped_and_recovered_publish_events() {
        let bus = install_test_event_bus();
        emit_ip_pool_ip_tripped("10.0.0.2".parse().unwrap(), "failures_exceeded");
        emit_ip_pool_ip_recovered("10.0.0.2".parse().unwrap());

        let events = bus.strategy_events();
        assert_eq!(events.len(), 2);
        assert!(matches!(
            events[0],
            StrategyEvent::IpPoolIpTripped { ref ip, ref reason }
                if ip == "10.0.0.2" && reason == "failures_exceeded"
        ));
        assert!(matches!(
            events[1],
            StrategyEvent::IpPoolIpRecovered { ref ip } if ip == "10.0.0.2"
        ));
    }

    #[test]
    fn emit_ip_pool_config_update_publishes_event() {
        let bus = install_test_event_bus();
        let dummy = EffectiveIpPoolConfig::default();
        emit_ip_pool_config_update(&dummy, &dummy);

        expect_single_event(bus.strategy_events(), |event| match event {
            StrategyEvent::IpPoolConfigUpdate { old, new } => {
                assert!(old.contains("EffectiveIpPoolConfig"));
                assert!(new.contains("EffectiveIpPoolConfig"));
            }
            other => panic!("unexpected event variant: {:?}", other),
        });
    }

    #[test]
    fn emit_ip_pool_auto_disable_and_enable_publish_events() {
        let bus = install_test_event_bus();
        emit_ip_pool_auto_disable("manual_test", 1_234_567_890);
        emit_ip_pool_auto_enable();

        let events = bus.strategy_events();
        assert_eq!(events.len(), 2);
        assert!(matches!(
            events[0],
            StrategyEvent::IpPoolAutoDisable { ref reason, until_ms }
                if reason == "manual_test" && until_ms == 1_234_567_890
        ));
        assert!(matches!(events[1], StrategyEvent::IpPoolAutoEnable {}));
    }

    #[test]
    fn circuit_breaker_repeated_tripped_and_recovered() {
        let bus = install_test_event_bus();
        for _ in 0..3 {
            emit_ip_pool_ip_tripped("10.0.0.1".parse().unwrap(), "failures_exceeded");
            emit_ip_pool_ip_recovered("10.0.0.1".parse().unwrap());
        }

        let events = bus.strategy_events();
        let tripped = events
            .iter()
            .filter(|event| matches!(event, StrategyEvent::IpPoolIpTripped { .. }))
            .count();
        let recovered = events
            .iter()
            .filter(|event| matches!(event, StrategyEvent::IpPoolIpRecovered { .. }))
            .count();
        assert_eq!(tripped, 3);
        assert_eq!(recovered, 3);
    }

    #[test]
    fn blacklist_whitelist_empty_and_invalid_cidr() {
        let bus = install_test_event_bus();
        emit_ip_pool_cidr_filter("1.2.3.4".parse().unwrap(), "blacklist", "");
        emit_ip_pool_cidr_filter("1.2.3.5".parse().unwrap(), "whitelist", "invalid_cidr");

        let events = bus.strategy_events();
        assert_eq!(events.len(), 2);
        assert!(matches!(
            events[0],
            StrategyEvent::IpPoolCidrFilter { ref ip, ref list_type, ref cidr }
                if ip == "1.2.3.4" && list_type == "blacklist" && cidr.is_empty()
        ));
        assert!(matches!(
            events[1],
            StrategyEvent::IpPoolCidrFilter { ref ip, ref list_type, ref cidr }
                if ip == "1.2.3.5" && list_type == "whitelist" && cidr == "invalid_cidr"
        ));
    }

    #[test]
    fn config_hot_reload_concurrent() {
        let bus = install_test_event_bus();
        let dummy = EffectiveIpPoolConfig::default();
        for _ in 0..5 {
            emit_ip_pool_config_update(&dummy, &dummy);
        }

        let updates = bus
            .strategy_events()
            .into_iter()
            .filter(|event| matches!(event, StrategyEvent::IpPoolConfigUpdate { .. }))
            .count();
        assert_eq!(updates, 5);
    }

    #[tokio::test]
    async fn pick_best_with_on_demand_sampling_does_not_emit_selection_event() {
        // Note: IP pool selection event is emitted at transport layer, not in pick_best
        // This test documents expected behavior: pick_best prepares candidates but doesn't emit
        let bus = install_test_event_bus();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let accept_task = tokio::spawn(async move {
            if let Ok((stream, _)) = listener.accept().await {
                drop(stream);
            }
        });

        let cfg = user_static_only_config("local.test", addr.ip(), addr.port());
        let pool = IpPool::with_cache(cfg, IpScoreCache::new());

        let _selection = pool.pick_best("local.test", addr.port()).await;
        accept_task.await.unwrap();

        // IP pool manager itself doesn't emit IpPoolSelection - that's transport layer's job
        let selection_events = bus
            .strategy_events()
            .into_iter()
            .filter(|event| matches!(event, StrategyEvent::IpPoolSelection { .. }))
            .count();
        assert_eq!(selection_events, 0);
    }

    #[test]
    fn event_bus_thread_safety_and_replacement() {
        use fireworks_collaboration_lib::events::structured::{
            clear_test_event_bus, set_test_event_bus, Event, EventBusAny,
        };
        use std::sync::Arc;
        use std::thread;

        // Verify concurrent publishers can share the same thread-local override bus.
        {
            let bus_guard = install_test_event_bus();
            let bus_handle = bus_guard.handle();
            let bus_dyn: Arc<dyn EventBusAny> = bus_handle.clone();

            let handles: Vec<_> = (0..10)
                .map(|i| {
                    let bus_clone = bus_dyn.clone();
                    thread::spawn(move || {
                        set_test_event_bus(bus_clone);
                        emit_ip_pool_auto_disable(&format!("t{i}"), i as i64);
                        clear_test_event_bus();
                    })
                })
                .collect();

            for handle in handles {
                handle.join().unwrap();
            }

            let disable_count = bus_guard
                .snapshot()
                .into_iter()
                .filter(|event| {
                    matches!(
                        event,
                        Event::Strategy(StrategyEvent::IpPoolAutoDisable { .. })
                    )
                })
                .count();
            assert_eq!(disable_count, 10);
        }

        // Replacing the bus should isolate subsequent events.
        {
            let bus_guard = install_test_event_bus();
            emit_ip_pool_auto_enable();

            let enable_count = bus_guard
                .snapshot()
                .into_iter()
                .filter(|event| {
                    matches!(event, Event::Strategy(StrategyEvent::IpPoolAutoEnable {}))
                })
                .count();
            assert_eq!(enable_count, 1);
        }
    }
}

// ---------------- section_history_auto_cleanup ----------------
mod section_history_auto_cleanup {
    use super::common::fixtures;
    use super::common::prelude::*;
    use fireworks_collaboration_lib::core::ip_pool::{IpPool, IpScoreCache, IpSource};
    use std::fs;

    #[test]
    fn maintenance_tick_prunes_history_capacity() {
        let base = fixtures::create_empty_dir();
        let mut cfg = enabled_config();
        cfg.runtime.history_path = Some(base.join("history.json").to_string_lossy().into());
        cfg.runtime.max_cache_entries = 2;
        cfg.runtime.cache_prune_interval_secs = 1;
        let pool = IpPool::with_cache(cfg, IpScoreCache::new());
        let now_ms = epoch_ms();

        // Insert 4 records, max capacity is 2
        // Also put them in cache to ensure maintenance triggers
        for i in 0..4 {
            let stat = make_latency_stat(
                [10, 10, 10, i as u8 + 1],
                443,
                20 + i,
                IpSource::Builtin,
                Some(now_ms + i as i64 * 1000),
                Some(now_ms + 60_000),
            );
            let record = history_record(&format!("host{}.com", i), 443, &stat);
            pool.history().upsert(record).unwrap();
            cache_best(pool.cache(), &format!("host{}.com", i), 443, stat);
        }

        assert_eq!(pool.history().snapshot().unwrap().len(), 4);

        // Trigger maintenance, should prune oldest 2 entries
        pool.maintenance_tick_at(now_ms + 2_000);

        let remaining = pool.history().snapshot().unwrap();
        assert_eq!(remaining.len(), 2);
        // Should keep the 2 newest (host2, host3)
        assert!(remaining.iter().any(|r| r.host == "host2.com"));
        assert!(remaining.iter().any(|r| r.host == "host3.com"));

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn maintenance_tick_prunes_expired_and_capacity() {
        let base = fixtures::create_empty_dir();
        let mut cfg = enabled_config();
        cfg.runtime.history_path = Some(base.join("history.json").to_string_lossy().into());
        cfg.runtime.max_cache_entries = 2;
        cfg.runtime.cache_prune_interval_secs = 1;
        let pool = IpPool::with_cache(cfg, IpScoreCache::new());
        let now_ms = epoch_ms();

        // Insert 2 expired records
        for i in 0..2 {
            let stat = make_latency_stat(
                [11, 11, 11, i as u8 + 1],
                443,
                15,
                IpSource::Builtin,
                Some(now_ms - 10_000),
                Some(now_ms - 1_000),
            );
            let record = history_record(&format!("expired{}.com", i), 443, &stat);
            pool.history().upsert(record).unwrap();
            cache_best(pool.cache(), &format!("expired{}.com", i), 443, stat);
        }

        // Insert 3 valid records (exceeds capacity of 2)
        for i in 0..3 {
            let stat = make_latency_stat(
                [12, 12, 12, i as u8 + 1],
                443,
                18,
                IpSource::Builtin,
                Some(now_ms + i as i64 * 1000),
                Some(now_ms + 60_000),
            );
            let record = history_record(&format!("valid{}.com", i), 443, &stat);
            pool.history().upsert(record).unwrap();
            cache_best(pool.cache(), &format!("valid{}.com", i), 443, stat);
        }

        assert_eq!(pool.history().snapshot().unwrap().len(), 5);

        // Trigger maintenance: remove 2 expired + 1 oldest valid (valid0)
        pool.maintenance_tick_at(now_ms + 2_000);

        let remaining = pool.history().snapshot().unwrap();
        assert_eq!(remaining.len(), 2);
        // Should keep valid1 and valid2
        assert!(remaining.iter().all(|r| r.host.starts_with("valid")));
        assert!(remaining.iter().any(|r| r.host == "valid1.com"));
        assert!(remaining.iter().any(|r| r.host == "valid2.com"));

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn history_prune_failure_does_not_block_cache_maintenance() {
        let base = fixtures::create_empty_dir();
        let invalid_path = base.join("readonly").join("history.json");
        std::fs::create_dir_all(invalid_path.parent().unwrap()).unwrap();
        std::fs::write(&invalid_path, b"{}").unwrap();

        // Make directory readonly on Windows (best effort)
        #[cfg(windows)]
        {
            let mut perms = std::fs::metadata(invalid_path.parent().unwrap())
                .unwrap()
                .permissions();
            perms.set_readonly(true);
            let _ = std::fs::set_permissions(invalid_path.parent().unwrap(), perms);
        }

        let mut cfg = enabled_config();
        cfg.runtime.history_path = Some(invalid_path.to_string_lossy().into());
        cfg.runtime.cache_prune_interval_secs = 1;
        let pool = IpPool::with_cache(cfg, IpScoreCache::new());
        let now_ms = epoch_ms();

        let expired = make_latency_stat(
            [13, 13, 13, 13],
            443,
            22,
            IpSource::Builtin,
            Some(now_ms - 10_000),
            Some(now_ms - 1),
        );
        cache_best(pool.cache(), "expired.test", 443, expired.clone());

        // Maintenance should still prune cache even if history cleanup fails
        pool.maintenance_tick_at(now_ms + 2_000);
        assert!(pool.cache().get("expired.test", 443).is_none());

        let _ = fs::remove_dir_all(&base);
    }
}
