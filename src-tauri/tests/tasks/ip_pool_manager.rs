#![cfg(not(feature = "tauri-app"))]
//! IP Pool Manager 集成测试
//!
//! 测试 IP 池管理器的全生命周期功能。

use super::common::test_env::init_test_env;

#[ctor::ctor]
fn __init_env() {
    init_test_env();
}

// ---------------- section_selection_behaviour ----------------
mod section_selection_behaviour {
    use super::super::common::prelude::*;
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
    use super::super::common::fixtures;
    use super::super::common::prelude::*;
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
    use super::super::common::prelude::*;
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
    use super::super::common::prelude::*;
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
    use super::super::common::prelude::*;
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
            other => panic!("unexpected event variant: {other:?}"),
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
            other => panic!("unexpected event variant: {other:?}"),
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
            other => panic!("unexpected event variant: {other:?}"),
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
            "duplicate disable events detected: {disable_events:?}"
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
            other => panic!("unexpected event variant: {other:?}"),
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
            other => panic!("unexpected event variant: {other:?}"),
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
    use super::super::common::fixtures;
    use super::super::common::prelude::*;
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
            let record = history_record(&format!("host{i}.com"), 443, &stat);
            pool.history().upsert(record).unwrap();
            cache_best(pool.cache(), &format!("host{i}.com"), 443, stat);
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
            let record = history_record(&format!("expired{i}.com"), 443, &stat);
            pool.history().upsert(record).unwrap();
            cache_best(pool.cache(), &format!("expired{i}.com"), 443, stat);
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
            let record = history_record(&format!("valid{i}.com"), 443, &stat);
            pool.history().upsert(record).unwrap();
            cache_best(pool.cache(), &format!("valid{i}.com"), 443, stat);
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

// ============================================================================
// Root-level IP Pool 综合测试 (从 ip_pool.rs 合并)
// 包含 manager_tests, cache_tests, config_tests, circuit_breaker_tests,
// events_tests, preheat_tests, history_tests
// ============================================================================

// ---------------- section_ip_pool_core_manager ----------------
mod section_ip_pool_core_manager {
    use fireworks_collaboration_lib::core::ip_pool::config::EffectiveIpPoolConfig;
    use fireworks_collaboration_lib::core::ip_pool::manager::IpPool;
    use fireworks_collaboration_lib::core::ip_pool::{
        IpCandidate, IpOutcome, IpSelection, IpSource, IpStat,
    };
    use std::net::{IpAddr, Ipv4Addr};

    fn make_stat(addr: [u8; 4], port: u16) -> IpStat {
        let candidate = IpCandidate::new(
            IpAddr::from(Ipv4Addr::from(addr)),
            port,
            IpSource::UserStatic,
        );
        let mut stat = IpStat::with_latency(candidate, 12);
        stat.measured_at_epoch_ms = Some(1);
        stat.expires_at_epoch_ms = Some(i64::MAX - 1);
        stat.sources = vec![IpSource::UserStatic];
        stat
    }

    #[test]
    fn report_candidate_outcome_tracks_per_ip() {
        let mut cfg = EffectiveIpPoolConfig::default();
        cfg.runtime.enabled = true;
        let pool = IpPool::new(cfg);
        let stat = make_stat([127, 0, 0, 2], 443);
        pool.report_candidate_outcome("example.com", 443, &stat, IpOutcome::Failure);
        pool.report_candidate_outcome("example.com", 443, &stat, IpOutcome::Success);
        let metrics = pool
            .candidate_outcome_metrics("example.com", 443, stat.candidate.address)
            .expect("candidate metrics present");
        assert_eq!(metrics.success, 1);
        assert_eq!(metrics.failure, 1);
        assert_eq!(metrics.last_sources, vec![IpSource::UserStatic]);
    }

    #[test]
    fn report_outcome_updates_aggregate_counts() {
        let mut cfg = EffectiveIpPoolConfig::default();
        cfg.runtime.enabled = true;
        let pool = IpPool::new(cfg);
        let stat = make_stat([127, 0, 0, 3], 443);
        let selection = IpSelection::from_cached("example.com", 443, stat.clone());
        pool.report_outcome(&selection, IpOutcome::Success);
        let aggregate = pool
            .outcome_metrics("example.com", 443)
            .expect("aggregate metrics present");
        assert_eq!(aggregate.success, 1);
        assert_eq!(aggregate.failure, 0);
        assert!(aggregate.last_outcome_ms > 0);
    }
}

// ---------------- section_ip_pool_cache ----------------
mod section_ip_pool_cache {
    use fireworks_collaboration_lib::core::ip_pool::cache::{IpCacheKey, IpCacheSlot, IpScoreCache};
    use fireworks_collaboration_lib::core::ip_pool::{IpCandidate, IpSource, IpStat};
    use std::net::{IpAddr, Ipv4Addr};

    #[test]
    fn cache_insert_and_get_best() {
        let cache = IpScoreCache::new();
        let key = IpCacheKey::new("github.com", 443);
        let stat = IpStat::with_latency(
            IpCandidate::new(
                IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1)),
                443,
                IpSource::Builtin,
            ),
            42,
        );
        cache.insert(key.clone(), IpCacheSlot::with_best(stat.clone()));
        let fetched = cache.get("github.com", 443).unwrap();
        assert_eq!(fetched.best.as_ref().unwrap().latency_ms, Some(42));
        assert_eq!(
            fetched.best.unwrap().candidate.address,
            stat.candidate.address
        );
        assert_eq!(stat.sources, vec![IpSource::Builtin]);
        // 确保 snapshot 复制，而不是共享引用
        let snapshot = cache.snapshot();
        assert!(snapshot.contains_key(&key));
    }

    #[test]
    fn cache_remove_and_clear() {
        let cache = IpScoreCache::new();
        cache.insert(
            IpCacheKey::new("github.com", 443),
            IpCacheSlot::with_best(IpStat::with_latency(
                IpCandidate::new(
                    IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1)),
                    443,
                    IpSource::Builtin,
                ),
                10,
            )),
        );
        cache.remove("github.com", 443);
        assert!(cache.get("github.com", 443).is_none());
        cache.insert(
            IpCacheKey::new("github.com", 80),
            IpCacheSlot::with_best(IpStat::with_latency(
                IpCandidate::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 80, IpSource::Dns),
                5,
            )),
        );
        cache.clear();
        assert!(cache.get("github.com", 80).is_none());
    }
}

// ---------------- section_ip_pool_config ----------------
mod section_ip_pool_config {
    use fireworks_collaboration_lib::core::ip_pool::config::{
        default_cache_prune_interval_secs, default_cooldown_seconds, default_failure_rate_threshold,
        default_failure_threshold, default_failure_window_seconds, default_max_cache_entries,
        default_max_parallel_probes, default_min_samples_in_window, default_probe_timeout_ms,
        default_score_ttl_seconds, default_singleflight_timeout_ms, EffectiveIpPoolConfig,
        IpPoolFileConfig, IpPoolRuntimeConfig, PreheatDomain, UserStaticIp,
    };
    use fireworks_collaboration_lib::core::ip_pool::config::{
        join_ip_config_path, load_or_init_file_at, save_file_at,
    };
    use std::fs;
    use std::sync::{Mutex, OnceLock};

    #[test]
    fn runtime_defaults_are_disabled() {
        let cfg = IpPoolRuntimeConfig::default();
        assert!(!cfg.enabled);
        assert_eq!(cfg.max_parallel_probes, default_max_parallel_probes());
        assert_eq!(cfg.probe_timeout_ms, default_probe_timeout_ms());
        assert!(cfg.history_path.is_none());
        assert!(cfg.sources.builtin);
        assert!(cfg.sources.dns);
        assert!(cfg.sources.history);
        assert!(cfg.sources.user_static);
        assert!(cfg.sources.fallback);
        assert_eq!(
            cfg.cache_prune_interval_secs,
            default_cache_prune_interval_secs()
        );
        assert_eq!(cfg.max_cache_entries, default_max_cache_entries());
        assert_eq!(
            cfg.singleflight_timeout_ms,
            default_singleflight_timeout_ms()
        );
        assert_eq!(cfg.failure_threshold, default_failure_threshold());
        assert_eq!(cfg.failure_rate_threshold, default_failure_rate_threshold());
        assert_eq!(cfg.failure_window_seconds, default_failure_window_seconds());
        assert_eq!(cfg.min_samples_in_window, default_min_samples_in_window());
        assert_eq!(cfg.cooldown_seconds, default_cooldown_seconds());
        assert!(cfg.circuit_breaker_enabled);
    }

    #[test]
    fn file_defaults_are_empty_preheat() {
        let cfg = IpPoolFileConfig::default();
        assert!(cfg.preheat_domains.is_empty());
        assert_eq!(cfg.score_ttl_seconds, default_score_ttl_seconds());
        assert!(cfg.user_static.is_empty());
        assert!(cfg.blacklist.is_empty());
        assert!(cfg.whitelist.is_empty());
    }

    #[test]
    fn deserializes_with_defaults() {
        let json = r#"{
            "runtime": {
                "enabled": true,
                "maxParallelProbes": 8,
                "historyPath": "custom/ip-history.json"
            },
            "file": {
                "preheatDomains": [{"host": "github.com", "ports": [443, 80]}]
            }
        }"#;
        let cfg: EffectiveIpPoolConfig = serde_json::from_str(json).unwrap();
        assert!(cfg.runtime.enabled);
        assert_eq!(cfg.runtime.max_parallel_probes, 8);
        assert_eq!(cfg.runtime.probe_timeout_ms, default_probe_timeout_ms());
        assert_eq!(cfg.file.score_ttl_seconds, default_score_ttl_seconds());
        assert_eq!(cfg.file.preheat_domains.len(), 1);
        let domain = &cfg.file.preheat_domains[0];
        assert_eq!(domain.host, "github.com");
        assert_eq!(domain.ports, vec![443, 80]);
        assert!(cfg.file.user_static.is_empty());
        assert_eq!(
            cfg.runtime.cache_prune_interval_secs,
            default_cache_prune_interval_secs()
        );
        assert_eq!(cfg.runtime.max_cache_entries, default_max_cache_entries());
        assert_eq!(
            cfg.runtime.singleflight_timeout_ms,
            default_singleflight_timeout_ms()
        );
    }

    #[test]
    fn load_or_init_file_creates_default() {
        let guard = test_guard().lock().unwrap();
        let temp_dir = std::env::temp_dir().join(format!("fwc-ip-pool-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&temp_dir).unwrap();
        let cfg = load_or_init_file_at(&temp_dir).expect("create default ip config");
        assert!(cfg.preheat_domains.is_empty());
        assert_eq!(cfg.score_ttl_seconds, default_score_ttl_seconds());
        let path = join_ip_config_path(&temp_dir);
        assert!(path.exists());
        fs::remove_dir_all(&temp_dir).ok();
        drop(guard);
    }

    #[test]
    fn save_file_persists_changes() {
        let guard = test_guard().lock().unwrap();
        let temp_dir = std::env::temp_dir().join(format!("fwc-ip-pool-save-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&temp_dir).unwrap();
        let mut cfg = IpPoolFileConfig::default();
        cfg.preheat_domains.push(PreheatDomain::new("github.com"));
        cfg.score_ttl_seconds = 120;
        cfg.user_static.push(UserStaticIp {
            host: "github.com".into(),
            ip: "140.82.112.3".into(),
            ports: vec![443],
        });
        save_file_at(&cfg, &temp_dir).expect("save ip config");
        let loaded = load_or_init_file_at(&temp_dir).expect("load ip config");
        assert_eq!(loaded.preheat_domains.len(), 1);
        assert_eq!(loaded.score_ttl_seconds, 120);
        assert_eq!(loaded.user_static.len(), 1);
        fs::remove_dir_all(&temp_dir).ok();
        drop(guard);
    }

    fn test_guard() -> &'static Mutex<()> {
        static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
        GUARD.get_or_init(|| Mutex::new(()))
    }
}

// ---------------- section_circuit_breaker ----------------
mod section_circuit_breaker {
    use fireworks_collaboration_lib::core::ip_pool::circuit_breaker::{
        CircuitBreaker, CircuitBreakerConfig, CircuitState,
    };
    use std::net::IpAddr;

    #[test]
    fn consecutive_failures_trigger_circuit_open() {
        let breaker = CircuitBreaker::new(CircuitBreakerConfig {
            enabled: true,
            consecutive_failure_threshold: 3,
            ..Default::default()
        });

        let ip = "192.0.2.1".parse().unwrap();

        // 前两次失败不应触发
        breaker.record_failure(ip);
        assert!(!breaker.is_tripped(ip));
        breaker.record_failure(ip);
        assert!(!breaker.is_tripped(ip));

        // 第三次失败触发熔断
        breaker.record_failure(ip);
        assert!(breaker.is_tripped(ip));

        let stats = breaker.get_stats(ip).unwrap();
        assert_eq!(stats.state, CircuitState::Cooldown);
        assert_eq!(stats.consecutive_failures, 3);
    }

    #[test]
    fn success_resets_consecutive_failures() {
        let breaker = CircuitBreaker::new(CircuitBreakerConfig {
            enabled: true,
            consecutive_failure_threshold: 3,
            failure_rate_threshold: 0.9, // 高阈值，避免失败率触发
            min_samples_in_window: 10,   // 高样本数要求
            ..Default::default()
        });

        let ip = "192.0.2.1".parse().unwrap();

        breaker.record_failure(ip);
        breaker.record_failure(ip);
        breaker.record_success(ip); // 重置连续失败计数
        breaker.record_failure(ip);
        breaker.record_failure(ip);

        // 不应触发，因为中间有成功，连续失败被重置
        assert!(!breaker.is_tripped(ip));
    }

    #[test]
    fn failure_rate_triggers_circuit_open() {
        let breaker = CircuitBreaker::new(CircuitBreakerConfig {
            enabled: true,
            consecutive_failure_threshold: 100, // 不通过连续失败触发
            failure_rate_threshold: 0.5,
            min_samples_in_window: 5,
            ..Default::default()
        });

        let ip = "192.0.2.1".parse().unwrap();

        // 2 成功 + 3 失败 = 5 样本，失败率 60%
        breaker.record_success(ip);
        breaker.record_success(ip);
        breaker.record_failure(ip);
        breaker.record_failure(ip);
        breaker.record_failure(ip);

        // 应触发熔断
        assert!(breaker.is_tripped(ip));
    }

    #[test]
    fn manual_reset_clears_circuit_state() {
        let breaker = CircuitBreaker::new(CircuitBreakerConfig {
            enabled: true,
            consecutive_failure_threshold: 2,
            ..Default::default()
        });

        let ip = "192.0.2.1".parse().unwrap();

        breaker.record_failure(ip);
        breaker.record_failure(ip);
        assert!(breaker.is_tripped(ip));

        breaker.reset_ip(ip);
        assert!(!breaker.is_tripped(ip));
    }

    #[test]
    fn disabled_breaker_never_trips() {
        let breaker = CircuitBreaker::new(CircuitBreakerConfig {
            enabled: false,
            consecutive_failure_threshold: 1,
            ..Default::default()
        });

        let ip = "192.0.2.1".parse().unwrap();

        breaker.record_failure(ip);
        breaker.record_failure(ip);
        breaker.record_failure(ip);

        assert!(!breaker.is_tripped(ip));
    }

    #[test]
    fn get_tripped_ips_returns_only_tripped() {
        let breaker = CircuitBreaker::new(CircuitBreakerConfig {
            enabled: true,
            consecutive_failure_threshold: 2,
            ..Default::default()
        });

        let ip1: IpAddr = "192.0.2.1".parse().unwrap();
        let ip2: IpAddr = "192.0.2.2".parse().unwrap();
        let ip3: IpAddr = "192.0.2.3".parse().unwrap();

        breaker.record_failure(ip1);
        breaker.record_failure(ip1); // ip1 熔断

        breaker.record_failure(ip2);
        breaker.record_success(ip2); // ip2 正常

        breaker.record_failure(ip3);
        breaker.record_failure(ip3); // ip3 熔断

        let tripped = breaker.get_tripped_ips();
        assert_eq!(tripped.len(), 2);
        assert!(tripped.contains(&ip1));
        assert!(tripped.contains(&ip3));
        assert!(!tripped.contains(&ip2));
    }
}

// ---------------- section_ip_pool_events ----------------
mod section_ip_pool_events {
    use fireworks_collaboration_lib::core::ip_pool::events::{
        emit_ip_pool_refresh, emit_ip_pool_selection,
    };
    use fireworks_collaboration_lib::core::ip_pool::{
        IpCandidate, IpSelectionStrategy, IpSource, IpStat,
    };
    use fireworks_collaboration_lib::events::structured::{
        clear_test_event_bus, set_test_event_bus, Event, MemoryEventBus, StrategyEvent,
    };
    use std::net::{IpAddr, Ipv4Addr};
    use std::sync::Arc;
    use std::time::{SystemTime, UNIX_EPOCH};
    use uuid::Uuid;

    #[test]
    fn emit_ip_pool_selection_publishes_event() {
        let bus = MemoryEventBus::new();
        set_test_event_bus(Arc::new(bus.clone()));

        let task_id = Uuid::new_v4();
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;
        let stat = IpStat {
            candidate: IpCandidate::new(
                IpAddr::V4(Ipv4Addr::new(1, 2, 3, 4)),
                443,
                IpSource::Builtin,
            ),
            sources: vec![IpSource::Builtin, IpSource::Dns],
            latency_ms: Some(42),
            measured_at_epoch_ms: Some(now_ms),
            expires_at_epoch_ms: Some(now_ms + 300_000),
        };

        emit_ip_pool_selection(
            task_id,
            "github.com",
            443,
            IpSelectionStrategy::Cached,
            Some(&stat),
            3,
        );

        let events = bus.snapshot();
        assert_eq!(events.len(), 1);
        match &events[0] {
            Event::Strategy(StrategyEvent::IpPoolSelection {
                id,
                domain,
                port,
                strategy,
                source,
                latency_ms,
                candidates_count,
            }) => {
                assert_eq!(id, &task_id.to_string());
                assert_eq!(domain, "github.com");
                assert_eq!(*port, 443);
                assert_eq!(strategy, "Cached");
                assert!(source.is_some());
                assert_eq!(*latency_ms, Some(42));
                assert_eq!(*candidates_count, 3);
            }
            _ => panic!("expected IpPoolSelection event"),
        }
        clear_test_event_bus();
    }

    #[test]
    fn emit_ip_pool_refresh_publishes_event() {
        let bus = MemoryEventBus::new();
        set_test_event_bus(Arc::new(bus.clone()));

        let task_id = Uuid::new_v4();
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;
        let candidates = vec![
            IpStat {
                candidate: IpCandidate::new(
                    IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1)),
                    443,
                    IpSource::Builtin,
                ),
                sources: vec![IpSource::Builtin],
                latency_ms: Some(10),
                measured_at_epoch_ms: Some(now_ms),
                expires_at_epoch_ms: Some(now_ms + 300_000),
            },
            IpStat {
                candidate: IpCandidate::new(IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2)), 443, IpSource::Dns),
                sources: vec![IpSource::Dns],
                latency_ms: Some(20),
                measured_at_epoch_ms: Some(now_ms),
                expires_at_epoch_ms: Some(now_ms + 300_000),
            },
        ];

        emit_ip_pool_refresh(
            task_id,
            "github.com",
            true,
            &candidates,
            "preheat".to_string(),
        );

        let events = bus.snapshot();
        assert_eq!(events.len(), 1);
        match &events[0] {
            Event::Strategy(StrategyEvent::IpPoolRefresh {
                id,
                domain,
                success,
                candidates_count,
                min_latency_ms,
                max_latency_ms,
                reason,
            }) => {
                assert_eq!(id, &task_id.to_string());
                assert_eq!(domain, "github.com");
                assert!(success);
                assert_eq!(*candidates_count, 2);
                assert_eq!(*min_latency_ms, Some(10));
                assert_eq!(*max_latency_ms, Some(20));
                assert_eq!(reason, "preheat");
            }
            _ => panic!("expected IpPoolRefresh event"),
        }
        clear_test_event_bus();
    }

    #[test]
    fn emit_ip_pool_refresh_handles_empty_candidates() {
        let bus = MemoryEventBus::new();
        set_test_event_bus(Arc::new(bus.clone()));

        let task_id = Uuid::new_v4();
        emit_ip_pool_refresh(
            task_id,
            "example.com",
            false,
            &[],
            "sampling_failed".to_string(),
        );

        let events = bus.snapshot();
        assert_eq!(events.len(), 1);
        match &events[0] {
            Event::Strategy(StrategyEvent::IpPoolRefresh {
                success,
                candidates_count,
                min_latency_ms,
                max_latency_ms,
                ..
            }) => {
                assert!(!success);
                assert_eq!(*candidates_count, 0);
                assert!(min_latency_ms.is_none());
                assert!(max_latency_ms.is_none());
            }
            _ => panic!("expected IpPoolRefresh event"),
        }
        clear_test_event_bus();
    }
}

// ---------------- section_preheat ----------------
mod section_preheat {
    use fireworks_collaboration_lib::core::ip_pool::cache::IpScoreCache;
    use fireworks_collaboration_lib::core::ip_pool::config::{
        EffectiveIpPoolConfig, PreheatDomain, UserStaticIp,
    };
    use fireworks_collaboration_lib::core::ip_pool::history::IpHistoryStore;
    use fireworks_collaboration_lib::core::ip_pool::preheat::{
        builtin_lookup, collect_candidates, current_epoch_ms, next_due_schedule, probe_latency,
        update_cache_and_history, user_static_lookup, AggregatedCandidate, DomainSchedule,
    };
    use fireworks_collaboration_lib::core::ip_pool::{IpCandidate, IpSource};
    use std::net::IpAddr;
    use std::sync::Arc;
    use tokio::runtime::Builder;
    use tokio::time::{Duration as TokioDuration, Instant};

    fn test_runtime() -> tokio::runtime::Runtime {
        Builder::new_current_thread().enable_all().build().unwrap()
    }

    #[test]
    fn builtin_lookup_returns_known_ips() {
        let ips = builtin_lookup("github.com");
        assert!(!ips.is_empty());
        assert!(ips.iter().any(|ip| ip.is_ipv4()));
    }

    #[test]
    fn user_static_lookup_filters_by_port() {
        let entries = vec![UserStaticIp {
            host: "example.com".into(),
            ip: "1.1.1.1".into(),
            ports: vec![80],
        }];
        assert!(user_static_lookup("example.com", 443, &entries).is_empty());
        let hits = user_static_lookup("example.com", 80, &entries);
        assert_eq!(hits.len(), 1);
    }

    #[test]
    fn collect_candidates_prefers_configured_sources() {
        let rt = test_runtime();
        let history = Arc::new(IpHistoryStore::in_memory());
        let mut cfg = EffectiveIpPoolConfig::default();
        cfg.runtime.enabled = true;
        cfg.runtime.sources.builtin = true;
        cfg.file
            .preheat_domains
            .push(PreheatDomain::new("github.com"));
        let candidates =
            rt.block_on(async { collect_candidates("github.com", 443, &cfg, history).await });
        assert!(!candidates.is_empty());
    }

    #[test]
    fn probe_latency_times_out_reasonably() {
        let rt = test_runtime();
        let result = rt.block_on(async { probe_latency("203.0.113.1".parse().unwrap(), 9, 200).await });
        assert!(result.is_err());
    }

    #[test]
    fn update_cache_and_history_writes_best_entry() {
        let cache = Arc::new(IpScoreCache::new());
        let history = Arc::new(IpHistoryStore::in_memory());
        let stat = AggregatedCandidate::new("1.1.1.1".parse().unwrap(), 443, IpSource::Builtin)
            .to_stat(10, 60);
        update_cache_and_history(
            "github.com",
            443,
            vec![stat],
            cache.clone(),
            history.clone(),
        )
        .unwrap();
        assert!(cache.get("github.com", 443).is_some());
        assert!(history.get("github.com", 443).is_some());
    }

    #[test]
    fn domain_schedule_backoff_caps_after_retries() {
        let now = Instant::now();
        let mut schedule = DomainSchedule::new(PreheatDomain::new("example.com"), 120, now);
        schedule.mark_failure(now);
        assert_eq!(schedule.current_backoff().as_secs(), 240);
        schedule.mark_failure(now);
        assert_eq!(schedule.current_backoff().as_secs(), 480);
        schedule.mark_failure(now);
        assert_eq!(schedule.current_backoff().as_secs(), 720);
        schedule.mark_failure(now);
        assert_eq!(schedule.current_backoff().as_secs(), 720);
        schedule.mark_success(now);
        assert_eq!(schedule.current_backoff().as_secs(), 120);
    }

    #[test]
    fn domain_schedule_force_refresh_resets_state() {
        let now = Instant::now();
        let mut schedule = DomainSchedule::new(PreheatDomain::new("refresh.com"), 300, now);
        schedule.mark_failure(now);
        assert!(schedule.failure_streak() > 0);
        let later = now + TokioDuration::from_secs(5);
        schedule.force_refresh(later);
        assert_eq!(schedule.failure_streak(), 0);
        assert_eq!(schedule.current_backoff().as_secs(), 300);
        assert_eq!(schedule.next_due(), later);
    }

    #[test]
    fn next_due_schedule_selects_earliest_entry() {
        let base = Instant::now();
        let mut first = DomainSchedule::new(PreheatDomain::new("early.com"), 120, base);
        first.mark_success(base);
        let mut second = DomainSchedule::new(PreheatDomain::new("now.com"), 120, base);
        second.force_refresh(base);
        let schedules = vec![first.clone(), second.clone()];
        let (idx, due) = next_due_schedule(&schedules).expect("schedule entry");
        assert_eq!(schedules[idx].domain.host, "now.com");
        assert_eq!(due, schedules[idx].next_due());
        assert!(due <= first.next_due());
    }

    #[test]
    fn collect_candidates_merges_sources_from_history() {
        let rt = test_runtime();
        let history = Arc::new(IpHistoryStore::in_memory());
        let ip: IpAddr = "140.82.112.3".parse().unwrap();
        let future_expire = current_epoch_ms() + 60_000;
        let record = fireworks_collaboration_lib::core::ip_pool::history::IpHistoryRecord {
            host: "github.com".into(),
            port: 443,
            candidate: IpCandidate::new(ip, 443, IpSource::History),
            sources: vec![IpSource::History, IpSource::UserStatic],
            latency_ms: 12,
            measured_at_epoch_ms: future_expire - 60_000,
            expires_at_epoch_ms: future_expire,
        };
        history.upsert(record).unwrap();
        let mut cfg = EffectiveIpPoolConfig::default();
        cfg.runtime.enabled = true;
        cfg.runtime.sources.builtin = true;
        cfg.runtime.sources.history = true;
        cfg.runtime.sources.dns = false;
        cfg.runtime.sources.user_static = false;
        cfg.runtime.sources.fallback = false;
        let candidates =
            rt.block_on(async { collect_candidates("github.com", 443, &cfg, history.clone()).await });
        let merged = candidates
            .into_iter()
            .find(|candidate| candidate.candidate.address == ip)
            .expect("candidate merged from history and builtin");
        assert!(merged.sources.contains(&IpSource::History));
        assert!(merged.sources.contains(&IpSource::Builtin));
        assert!(merged.sources.contains(&IpSource::UserStatic));
        assert!(history.get("github.com", 443).is_some());
    }

    #[test]
    fn collect_candidates_skips_expired_history_entries() {
        let rt = test_runtime();
        let history = Arc::new(IpHistoryStore::in_memory());
        let mut cfg = EffectiveIpPoolConfig::default();
        cfg.runtime.enabled = true;
        cfg.runtime.sources.builtin = false;
        cfg.runtime.sources.dns = false;
        cfg.runtime.sources.user_static = false;
        cfg.runtime.sources.fallback = false;
        cfg.runtime.sources.history = true;
        let record = fireworks_collaboration_lib::core::ip_pool::history::IpHistoryRecord {
            host: "expired.test".into(),
            port: 443,
            candidate: IpCandidate::new("1.1.1.1".parse().unwrap(), 443, IpSource::History),
            sources: vec![IpSource::History],
            latency_ms: 10,
            measured_at_epoch_ms: 1,
            expires_at_epoch_ms: 2,
        };
        history.upsert(record).unwrap();
        let candidates =
            rt.block_on(async { collect_candidates("expired.test", 443, &cfg, history.clone()).await });
        assert!(candidates.is_empty());
        assert!(history.get("expired.test", 443).is_none());
    }
}

// ---------------- section_history_tests ----------------
mod section_history_tests {
    use fireworks_collaboration_lib::core::ip_pool::history::{IpHistoryRecord, IpHistoryStore};
    use fireworks_collaboration_lib::core::ip_pool::{IpCandidate, IpSource};
    use std::fs;
    use std::net::{IpAddr, Ipv4Addr};
    use uuid::Uuid;

    #[test]
    fn load_or_init_creates_file() {
        let dir = std::env::temp_dir().join(format!("ip-history-{}", Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        let store = IpHistoryStore::load_or_init_at(&dir).expect("load history");
        let path = IpHistoryStore::join_history_path(&dir);
        assert!(path.exists());
        assert!(store.snapshot().unwrap().is_empty());
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn upsert_and_get_roundtrip() {
        let dir = std::env::temp_dir().join(format!("ip-history-{}", Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        let store = IpHistoryStore::load_or_init_at(&dir).expect("load history");
        let record = IpHistoryRecord {
            host: "github.com".into(),
            port: 443,
            candidate: IpCandidate::new(
                IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1)),
                443,
                IpSource::Builtin,
            ),
            sources: vec![IpSource::Builtin, IpSource::Dns],
            latency_ms: 32,
            measured_at_epoch_ms: 1,
            expires_at_epoch_ms: 2,
        };
        store.upsert(record.clone()).expect("write history");
        let fetched = store.get("github.com", 443).expect("history entry");
        assert_eq!(fetched.latency_ms, 32);
        assert_eq!(fetched.sources, vec![IpSource::Builtin, IpSource::Dns]);
        let snapshot = store.snapshot().unwrap();
        assert_eq!(snapshot.len(), 1);
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn load_or_init_from_file_creates_parent_dirs() {
        let base = std::env::temp_dir().join(format!("ip-history-file-{}", Uuid::new_v4()));
        let path = base.join("nested").join("custom-history.json");
        let store = IpHistoryStore::load_or_init_from_file(&path).expect("load history file");
        assert!(path.exists());
        assert!(store.snapshot().unwrap().is_empty());
        fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn get_fresh_evicts_expired_records() {
        let store = IpHistoryStore::in_memory();
        let record = IpHistoryRecord {
            host: "github.com".into(),
            port: 443,
            candidate: IpCandidate::new(
                IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1)),
                443,
                IpSource::Builtin,
            ),
            sources: vec![IpSource::Builtin],
            latency_ms: 10,
            measured_at_epoch_ms: 1,
            expires_at_epoch_ms: 5,
        };
        store.upsert(record).expect("write history");
        let missing = store.get_fresh("github.com", 443, 10);
        assert!(missing.is_none());
        assert!(store.snapshot().unwrap().is_empty());
    }

    #[test]
    fn get_fresh_returns_valid_record() {
        let store = IpHistoryStore::in_memory();
        let record = IpHistoryRecord {
            host: "github.com".into(),
            port: 443,
            candidate: IpCandidate::new(
                IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1)),
                443,
                IpSource::Builtin,
            ),
            sources: vec![IpSource::Builtin, IpSource::History],
            latency_ms: 8,
            measured_at_epoch_ms: 1,
            expires_at_epoch_ms: 10_000,
        };
        store.upsert(record.clone()).expect("write history");
        let fetched = store
            .get_fresh("github.com", 443, 5_000)
            .expect("fresh record");
        assert_eq!(fetched.latency_ms, record.latency_ms);
        assert_eq!(fetched.sources, record.sources);
        assert_eq!(store.snapshot().unwrap().len(), 1);
    }

    #[test]
    fn remove_clears_matching_entry() {
        let store = IpHistoryStore::in_memory();
        let record = IpHistoryRecord {
            host: "github.com".into(),
            port: 443,
            candidate: IpCandidate::new(
                IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1)),
                443,
                IpSource::Builtin,
            ),
            sources: vec![IpSource::Builtin],
            latency_ms: 10,
            measured_at_epoch_ms: 1,
            expires_at_epoch_ms: 10_000,
        };
        store.upsert(record).expect("write history");
        assert!(store.remove("github.com", 443).expect("remove entry"));
        assert!(store.get("github.com", 443).is_none());
        assert!(!store.remove("github.com", 443).expect("idempotent remove"));
    }

    #[test]
    fn enforce_capacity_removes_oldest_entries() {
        let store = IpHistoryStore::in_memory();
        for i in 0..5 {
            let record = IpHistoryRecord {
                host: format!("host{i}.com"),
                port: 443,
                candidate: IpCandidate::new(
                    IpAddr::V4(Ipv4Addr::new(1, 1, 1, i as u8 + 1)),
                    443,
                    IpSource::Builtin,
                ),
                sources: vec![IpSource::Builtin],
                latency_ms: 10 + i as u32,
                measured_at_epoch_ms: (i + 1) as i64 * 1000,
                expires_at_epoch_ms: 100_000,
            };
            store.upsert(record).expect("write history");
        }
        let removed = store.enforce_capacity(3).expect("enforce capacity");
        assert_eq!(removed, 2);
        let snapshot = store.snapshot().unwrap();
        assert_eq!(snapshot.len(), 3);
        // Should keep the 3 newest (host2, host3, host4)
        assert!(snapshot.iter().all(|e| e.host.starts_with("host")
            && (e.host == "host2.com" || e.host == "host3.com" || e.host == "host4.com")));
    }

    #[test]
    fn prune_and_enforce_removes_expired_and_old_entries() {
        let store = IpHistoryStore::in_memory();
        // Add 2 expired entries
        for i in 0..2 {
            let record = IpHistoryRecord {
                host: format!("expired{i}.com"),
                port: 443,
                candidate: IpCandidate::new(
                    IpAddr::V4(Ipv4Addr::new(1, 1, 1, i as u8 + 1)),
                    443,
                    IpSource::Builtin,
                ),
                sources: vec![IpSource::Builtin],
                latency_ms: 10,
                measured_at_epoch_ms: 1000,
                expires_at_epoch_ms: 5000,
            };
            store.upsert(record).expect("write history");
        }
        // Add 4 valid entries
        for i in 0..4 {
            let record = IpHistoryRecord {
                host: format!("valid{i}.com"),
                port: 443,
                candidate: IpCandidate::new(
                    IpAddr::V4(Ipv4Addr::new(2, 2, 2, i as u8 + 1)),
                    443,
                    IpSource::Builtin,
                ),
                sources: vec![IpSource::Builtin],
                latency_ms: 20,
                measured_at_epoch_ms: (i + 1) as i64 * 2000 + 10000,
                expires_at_epoch_ms: 100_000,
            };
            store.upsert(record).expect("write history");
        }
        let (expired, capacity_pruned) = store
            .prune_and_enforce(10_000, 3)
            .expect("prune and enforce");
        assert_eq!(expired, 2);
        assert_eq!(capacity_pruned, 1);
        let snapshot = store.snapshot().unwrap();
        assert_eq!(snapshot.len(), 3);
        // Should only have valid entries, and the 3 newest
        assert!(snapshot.iter().all(|e| e.host.starts_with("valid")));
    }
}
