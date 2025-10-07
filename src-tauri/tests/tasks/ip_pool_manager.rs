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
    use fireworks_collaboration_lib::core::ip_pool::IpScoreCache;
    use fireworks_collaboration_lib::core::ip_pool::{IpPool, IpSource};
    use fireworks_collaboration_lib::core::ip_pool::global::pick_best_async;
    use std::sync::atomic::Ordering;
    use super::super::common::ip_pool::{make_user_static_pool_with_listener, bind_ephemeral, spawn_single_accept};
    use std::sync::Arc;
    use std::sync::atomic::AtomicUsize;
    use tokio::time::{sleep, Duration};
    #[tokio::test]
    async fn selection_behaviour_variants() {
        // 场景 A: 池未启用（默认构造） -> 系统默认
        {
            let pool_disabled = IpPool::default();
            let sel = pool_disabled.pick_best("github.com", 443).await;
            assert!(sel.is_system_default() && sel.selected().is_none());
        }

        // 场景 B: 启用但所有来源关闭 -> 系统默认且缓存/历史无记录
        {
            let mut cfg = enabled_config();
            super::super::common::ip_pool::disable_all_sources(&mut cfg);
            let pool = IpPool::with_cache(cfg, IpScoreCache::new());
            let sel = pool.pick_best("missing.test", 443).await;
            assert!(sel.is_system_default());
            assert!(pool.cache().get("missing.test", 443).is_none());
            assert!(pool.history().get("missing.test", 443).is_none());
        }

        // 场景 C: 缓存命中 -> 返回缓存候选
        {
            let cache = IpScoreCache::new();
            let now = epoch_ms();
            let stat = make_latency_stat([1,1,1,1], 443, 15, IpSource::Builtin, Some(now - 1_000), Some(now + 60_000));
            cache_best(&cache, "github.com", 443, stat.clone());
            let pool = IpPool::with_cache(enabled_config(), cache);
            let sel = pool.pick_best("github.com", 443).await;
            assert!(!sel.is_system_default());
            assert_eq!(sel.selected().unwrap().candidate.address, stat.candidate.address);
        }

        // 场景 D: 按需采样使用 user_static 候选 + 第二次命中缓存
        {
            let (pool, addr, _counter, accept_task) = make_user_static_pool_with_listener("local.test").await;
            let first = pool.pick_best("local.test", addr.port()).await;
            assert!(!first.is_system_default());
            accept_task.await.unwrap();
            let cached = pool.pick_best("local.test", addr.port()).await;
            assert_eq!(cached.selected().unwrap().candidate.address, addr.ip());
        }

        // 场景 E: TTL 过期触发重新采样
        {
            let (listener1, addr) = bind_ephemeral().await;
            let counter = Arc::new(AtomicUsize::new(0));
            let accept_task = spawn_single_accept(listener1, counter.clone());
            let mut cfg = user_static_only_config("ttl.test", addr.ip(), addr.port());
            cfg.runtime.cache_prune_interval_secs = 1;
            cfg.runtime.max_cache_entries = 16;
            cfg.file.score_ttl_seconds = 1;
            let pool = IpPool::with_cache(cfg, IpScoreCache::new());
            let first = pool.pick_best("ttl.test", addr.port()).await; assert!(!first.is_system_default());
            accept_task.await.unwrap();
            assert_eq!(counter.load(Ordering::SeqCst), 1);
            // 等待略大于 1s 的 TTL，缩短总时间同时保证过期
            sleep(Duration::from_millis(1_050)).await; // wait for ttl expiry
            let listener2 = tokio::net::TcpListener::bind(addr).await.unwrap();
            let accept_task2 = spawn_single_accept(listener2, counter.clone());
            let second = pool.pick_best("ttl.test", addr.port()).await; assert!(!second.is_system_default());
            accept_task2.await.unwrap();
            assert_eq!(counter.load(Ordering::SeqCst), 2);
        }

        // 场景 F: single-flight 避免重复并发采样
        {
            let (listener, addr) = bind_ephemeral().await;
            let counter = Arc::new(AtomicUsize::new(0));
            let accept_task = spawn_single_accept(listener, counter.clone());
            let cfg = user_static_only_config("flight.test", addr.ip(), addr.port());
            let pool = Arc::new(IpPool::with_cache(cfg, IpScoreCache::new()));
            let pool_a = pool.clone();
            let pool_b = pool.clone();
            let (a, b) = tokio::join!(
                async move { pool_a.pick_best("flight.test", addr.port()).await },
                async move { pool_b.pick_best("flight.test", addr.port()).await },
            );
            assert!(!a.is_system_default() && !b.is_system_default());
            accept_task.await.unwrap();
            assert_eq!(counter.load(Ordering::SeqCst), 1);
        }

        // 场景 G: 全局异步 API pick_best_async 不 panic 且返回 selection
        {
            let host = "github.com";
            let port = 443u16;
            let sel = pick_best_async(host, port).await;
            assert_eq!(sel.host(), host);
            assert_eq!(sel.port(), port);
            // 可能为 SystemDefault 或已缓存候选，只需保证语义合理
            assert!(sel.is_system_default() || sel.selected().is_some());
        }
    }
}

// ---------------- section_config_mutation ----------------
mod section_config_mutation {
    use super::super::common::fixtures;
    use super::super::common::prelude::*;
    use super::super::common::ip_pool::enabled_config_with_history;
    use fireworks_collaboration_lib::core::config::loader as cfg_loader;
    use fireworks_collaboration_lib::core::config::model::AppConfig;
    use fireworks_collaboration_lib::core::ip_pool::config::{save_file_at, IpPoolFileConfig};
    use fireworks_collaboration_lib::core::ip_pool::{load_effective_config_at, IpPool, IpSource};
    use std::fs;
    use std::net::{IpAddr, Ipv4Addr};
    #[tokio::test]
    async fn config_runtime_and_history_path_variants() {
        // 变体1: runtime update 应用后可用缓存候选
        {
            let mut pool = IpPool::default();
            let stat = make_latency_stat([9,9,9,9], 443, 7, IpSource::Builtin, None, None);
            cache_best(pool.cache(), "github.com", 443, stat.clone());
            assert!(pool.pick_best("github.com", 443).await.is_system_default());
            pool.update_config(enabled_config());
            let updated = pool.pick_best("github.com", 443).await;
            assert!(!updated.is_system_default());
            assert_eq!(updated.selected().unwrap().candidate.address, IpAddr::V4(Ipv4Addr::new(9,9,9,9)));
        }

        // 变体2: 自定义 history path 持久化
        {
            let base = fixtures::create_empty_dir();
            let history_path = base.join("nested").join("custom-history.json");
            let cfg = enabled_config_with_history(&history_path);
            let pool = IpPool::new(cfg);
            let stat = make_latency_stat([2,2,2,2],443,10,IpSource::Builtin,Some(epoch_ms()-1_000),Some(epoch_ms()+1_000));
            let record = history_record("custom.test", 443, &stat); pool.history().upsert(record).unwrap();
            assert!(history_path.exists());
            assert!(fs::read_to_string(&history_path).unwrap().contains("custom.test"));
            let _ = fs::remove_dir_all(&base);
        }

        // 变体3: update_config 切换 history path
        {
            let base = fixtures::create_empty_dir();
            let history_one = base.join("history-one.json");
            let history_two = base.join("history-two.json");
            let mut cfg = enabled_config_with_history(&history_one);
            let mut pool = IpPool::new(cfg.clone());
            let stat_one = make_latency_stat([3,3,3,3],443,11,IpSource::Builtin,Some(epoch_ms()-2_000),Some(epoch_ms()+2_000));
            pool.history().upsert(history_record("first.test", 443, &stat_one)).unwrap();
            assert!(history_one.exists());
            cfg.runtime.history_path = Some(history_two.to_string_lossy().into());
            pool.update_config(cfg);
            let stat_two = make_latency_stat([4,4,4,4],443,12,IpSource::Builtin,Some(epoch_ms()-1_000),Some(epoch_ms()+3_000));
            pool.history().upsert(history_record("second.test", 443, &stat_two)).unwrap();
            assert!(fs::read_to_string(&history_one).unwrap().contains("first.test"));
            assert!(fs::read_to_string(&history_two).unwrap().contains("second.test"));
            let _ = fs::remove_dir_all(&base);
        }

        // 变体4: load_effective_config 自定义目录
        {
            let base = fixtures::create_empty_dir();
            let mut file_cfg = IpPoolFileConfig::default();
            file_cfg.preheat_domains.push(fireworks_collaboration_lib::core::ip_pool::config::PreheatDomain::new("github.com"));
            file_cfg.score_ttl_seconds = 600; save_file_at(&file_cfg, &base).expect("save ip-config.json");
            let mut app_cfg = AppConfig::default(); app_cfg.ip_pool.enabled = true; app_cfg.ip_pool.max_parallel_probes = 16;
            let effective = load_effective_config_at(&app_cfg, &base).expect("load config");
            assert!(effective.runtime.enabled && effective.runtime.max_parallel_probes == 16);
            assert_eq!(effective.file.preheat_domains.len(), 1); assert_eq!(effective.file.score_ttl_seconds, 600);
            let _ = fs::remove_dir_all(&base);
        }

        // 变体5: from_app_config 默认 runtime
        {
            let base = fixtures::create_empty_dir();
            cfg_loader::set_global_base_dir(&base);
            let mut app_cfg = AppConfig::default(); app_cfg.ip_pool.enabled = true;
            let pool = IpPool::from_app_config(&app_cfg).expect("ip pool from app config");
            assert!(pool.is_enabled() && pool.file_config().preheat_domains.is_empty());
            let _ = fs::remove_dir_all(&base);
        }

        // 变体6: load_effective_config 无效文件错误
        {
            let base = fixtures::create_empty_dir();
            let cfg_dir = base.join("config"); std::fs::create_dir_all(&cfg_dir).unwrap();
            fs::write(cfg_dir.join("ip-config.json"), b"not-json").unwrap();
            assert!(load_effective_config_at(&AppConfig::default(), &base).is_err());
            let _ = fs::remove_dir_all(&base);
        }
    }
}

// ---------------- section_cache_maintenance ----------------
mod section_cache_maintenance {
    use super::super::common::prelude::*;
    use fireworks_collaboration_lib::core::ip_pool::config::PreheatDomain;
    use fireworks_collaboration_lib::core::ip_pool::IpScoreCache;
    use fireworks_collaboration_lib::core::ip_pool::{IpPool, IpSource};
    use super::super::common::ip_pool::cache_and_history;
    #[test]
    fn cache_maintenance_variants() {
        // 变体1: 过期条目同时从 cache 与 history 移除
        {
            let pool = IpPool::with_cache(enabled_config(), IpScoreCache::new());
            let now = epoch_ms();
            let expired = make_latency_stat([10,0,0,1], 443, 20, IpSource::Builtin, Some(now-10_000), Some(now-1_000));
            cache_and_history("expire.test", 443, expired, pool.cache(), Some(pool.history()));
            pool.maintenance_tick_at(now);
            assert!(pool.cache().get("expire.test", 443).is_none());
            assert!(pool.history().get("expire.test", 443).is_none());
        }

        // 变体2: 容量淘汰保留较新条目
        {
            let mut cfg = enabled_config(); cfg.runtime.max_cache_entries = 1; let pool = IpPool::with_cache(cfg, IpScoreCache::new());
            let now = epoch_ms();
            let older = make_latency_stat([11,0,0,1],443,40,IpSource::Builtin,Some(now-50_000),Some(now+50_000));
            let newer = make_latency_stat([11,0,0,2],443,30,IpSource::Builtin,Some(now-10_000),Some(now+50_000));
            cache_and_history("old.example",443,older,pool.cache(),Some(pool.history()));
            cache_and_history("new.example",443,newer,pool.cache(),Some(pool.history()));
            pool.maintenance_tick_at(now+1_000);
            assert!(pool.cache().get("old.example",443).is_none());
            assert!(pool.cache().get("new.example",443).is_some());
        }

        // 变体3: 预热域优先保留 + 淘汰旧条目
        {
            let mut cfg = enabled_config(); cfg.runtime.max_cache_entries = 1; cfg.file.preheat_domains.push(PreheatDomain::new("keep.test"));
            let pool = IpPool::with_cache(cfg, IpScoreCache::new()); let now = epoch_ms();
            let keep = make_latency_stat([12,0,0,1],443,18,IpSource::Builtin,Some(now-5_000),Some(now+60_000));
            let drop_old = make_latency_stat([13,0,0,1],443,30,IpSource::Builtin,Some(now-10_000),Some(now+60_000));
            let drop_new = make_latency_stat([14,0,0,1],443,16,IpSource::Builtin,Some(now-1_000),Some(now+60_000));
            cache_and_history("keep.test",443,keep,pool.cache(),Some(pool.history()));
            cache_and_history("drop-old.test",443,drop_old,pool.cache(),Some(pool.history()));
            cache_and_history("drop-new.test",443,drop_new,pool.cache(),Some(pool.history()));
            pool.maintenance_tick_at(now+2_000);
            assert!(pool.cache().get("drop-old.test",443).is_none());
            assert!(pool.cache().get("drop-new.test",443).is_some());
            assert!(pool.cache().get("keep.test",443).is_some());
        }

        // 变体4: 过期 preheat 仍留在 cache 但从 history 移除
        {
            let mut cfg = enabled_config(); cfg.runtime.cache_prune_interval_secs = 1; cfg.file.preheat_domains.push(PreheatDomain::new("keep.test"));
            let pool = IpPool::with_cache(cfg, IpScoreCache::new()); let now = epoch_ms();
            let keep = make_latency_stat([21,0,0,1],443,19,IpSource::Builtin,Some(now-10_000),Some(now-1));
            let drop = make_latency_stat([22,0,0,1],443,33,IpSource::Builtin,Some(now-9_000),Some(now-1));
            cache_and_history("keep.test",443,keep,pool.cache(),Some(pool.history()));
            cache_and_history("drop.test",443,drop,pool.cache(),Some(pool.history()));
            pool.maintenance_tick_at(now+5_000);
            assert!(pool.cache().get("drop.test",443).is_none());
            assert!(pool.cache().get("keep.test",443).is_some());
            assert!(pool.history().get("keep.test",443).is_none());
        }
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
        let pool = IpPool::with_cache(enabled_config(), cache);
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
        IpPool, IpSelectionStrategy, IpSource,
    };
    use fireworks_collaboration_lib::events::structured::StrategyEvent;
    use uuid::Uuid;
    use super::super::common::ip_pool::{
        expect_single_strategy_event as expect_single_event,
        expect_strategy_event_count,
    };

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
    fn emit_ip_pool_refresh_variants() {
        // 场景 A: 正常刷新包含延迟范围
        {
            let bus = install_test_event_bus();
            let task_id = Uuid::new_v4();
            let stats = vec![
                make_latency_stat([1, 1, 1, 1], 443, 10, IpSource::Builtin, Some(epoch_ms()), Some(epoch_ms() + 60_000)),
                make_latency_stat([2, 2, 2, 2], 443, 30, IpSource::Dns, Some(epoch_ms()), Some(epoch_ms() + 60_000)),
            ];
            emit_ip_pool_refresh(task_id, "example.com", true, &stats, "test".to_string());
            expect_single_event(bus.strategy_events(), |event| match event {
                StrategyEvent::IpPoolRefresh { id, domain, success, candidates_count, min_latency_ms, max_latency_ms, reason } => {
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

        // 场景 B: 空候选刷新
        {
            let bus = install_test_event_bus();
            let task_id = Uuid::new_v4();
            emit_ip_pool_refresh(task_id, "empty.com", false, &[], "no_candidates".to_string());
            expect_single_event(bus.strategy_events(), |event| match event {
                StrategyEvent::IpPoolRefresh { success, candidates_count, min_latency_ms, max_latency_ms, reason, .. } => {
                    assert!(!success);
                    assert_eq!(*candidates_count, 0);
                    assert!(min_latency_ms.is_none());
                    assert!(max_latency_ms.is_none());
                    assert_eq!(reason, "no_candidates");
                }
                other => panic!("unexpected event variant: {other:?}"),
            });
        }
    }

    #[test]
    fn auto_disable_extends_without_duplicate_events() {
        let bus = install_test_event_bus();
        let mut cfg = enabled_config();
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

        // 使用公共计数断言，确保只出现一次 AutoDisable 与一次 AutoEnable 事件
        expect_strategy_event_count(
            bus.strategy_events(),
            |event| matches!(event, StrategyEvent::IpPoolAutoDisable { .. }),
            1,
        );
        expect_strategy_event_count(
            bus.strategy_events(),
            |event| matches!(event, StrategyEvent::IpPoolAutoEnable { .. }),
            1,
        );
        // 仍然验证第一次 disable 的 reason 为初始 reason（扩展时不重复事件）
        let reason = bus
            .strategy_events()
            .iter()
            .find_map(|e| match e {
                StrategyEvent::IpPoolAutoDisable { reason, .. } => Some(reason.clone()),
                _ => None,
            })
            .expect("auto disable event reason");
        assert_eq!(reason, "test reason");
    }

    #[test]
    fn misc_cidr_circuit_config_events_variants() {
        // CIDR filter variants
        {
            let bus = install_test_event_bus();
            emit_ip_pool_cidr_filter("192.168.1.1".parse().unwrap(), "blacklist", "192.168.1.0/24");
            emit_ip_pool_cidr_filter("1.2.3.4".parse().unwrap(), "blacklist", "");
            emit_ip_pool_cidr_filter("1.2.3.5".parse().unwrap(), "whitelist", "invalid_cidr");
            let events = bus.strategy_events();
            assert_eq!(events.len(), 3);
            assert!(matches!(events[0], StrategyEvent::IpPoolCidrFilter { ref ip, ref list_type, ref cidr } if ip == "192.168.1.1" && list_type == "blacklist" && cidr == "192.168.1.0/24"));
            assert!(matches!(events[1], StrategyEvent::IpPoolCidrFilter { ref ip, ref list_type, ref cidr } if ip == "1.2.3.4" && list_type == "blacklist" && cidr.is_empty()));
            assert!(matches!(events[2], StrategyEvent::IpPoolCidrFilter { ref ip, ref list_type, ref cidr } if ip == "1.2.3.5" && list_type == "whitelist" && cidr == "invalid_cidr"));
        }
        // IP tripped + recovered
        {
            let bus = install_test_event_bus();
            emit_ip_pool_ip_tripped("10.0.0.2".parse().unwrap(), "failures_exceeded");
            emit_ip_pool_ip_recovered("10.0.0.2".parse().unwrap());
            let events = bus.strategy_events();
            assert_eq!(events.len(), 2);
            assert!(matches!(events[0], StrategyEvent::IpPoolIpTripped { ref ip, ref reason } if ip == "10.0.0.2" && reason == "failures_exceeded"));
            assert!(matches!(events[1], StrategyEvent::IpPoolIpRecovered { ref ip } if ip == "10.0.0.2"));
        }
        // config update single + hot reload burst
        {
            let bus = install_test_event_bus();
            let dummy = EffectiveIpPoolConfig::default();
            emit_ip_pool_config_update(&dummy, &dummy);
            expect_single_event(bus.strategy_events(), |event| match event {
                StrategyEvent::IpPoolConfigUpdate { old, new } => {
                    assert!(old.contains("EffectiveIpPoolConfig"));
                    assert!(new.contains("EffectiveIpPoolConfig"));
                }
                other => panic!("unexpected variant: {other:?}"),
            });
        }
        {
            let bus = install_test_event_bus();
            let dummy = EffectiveIpPoolConfig::default();
            for _ in 0..5 { emit_ip_pool_config_update(&dummy, &dummy); }
            expect_strategy_event_count(bus.strategy_events(), |e| matches!(e, StrategyEvent::IpPoolConfigUpdate { .. }), 5);
        }
    }

    #[tokio::test]
    async fn pick_best_with_on_demand_sampling_does_not_emit_selection_event() {
        // Note: IP pool selection event is emitted at transport layer, not in pick_best
        // This test documents expected behavior: pick_best prepares candidates but doesn't emit
        let bus = install_test_event_bus();
        // 复用公共 helper：减少绑定监听与 accept 样板
        let (pool, addr, _counter, accept_task) = super::super::common::ip_pool::make_user_static_pool_with_listener("local.test").await;
        let _selection = pool.pick_best("local.test", addr.port()).await;
        accept_task.await.unwrap();

        // 使用公共 helper 验证未产生任何 IpPoolSelection 事件
        expect_strategy_event_count(
            bus.strategy_events(),
            |event| matches!(event, StrategyEvent::IpPoolSelection { .. }),
            0,
        );
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

// ============================================================================
// Root-level IP Pool 综合测试 (从 ip_pool.rs 合并)
// 包含 manager_tests, cache_tests, config_tests, circuit_breaker_tests,
// events_tests, preheat_tests, history_tests
// ============================================================================

// ---------------- section_ip_pool_core_manager ----------------
mod section_ip_pool_core_manager {
    use super::super::common::prelude::*; // brings in enabled_config
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
    fn outcome_reporting_tracks_candidate_and_aggregate() {
        let cfg = enabled_config();
        let pool = IpPool::new(cfg);
        // candidate outcome
        let stat_ip = make_stat([127, 0, 0, 2], 443);
        pool.report_candidate_outcome("example.com", 443, &stat_ip, IpOutcome::Failure);
        pool.report_candidate_outcome("example.com", 443, &stat_ip, IpOutcome::Success);
        let metrics = pool
            .candidate_outcome_metrics("example.com", 443, stat_ip.candidate.address)
            .expect("candidate metrics present");
        assert_eq!(metrics.success, 1);
        assert_eq!(metrics.failure, 1);
        assert_eq!(metrics.last_sources, vec![IpSource::UserStatic]);

        // aggregate outcome
        let stat_agg = make_stat([127, 0, 0, 3], 443);
        let selection = IpSelection::from_cached("example.com", 443, stat_agg.clone());
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
    use fireworks_collaboration_lib::core::ip_pool::cache::{
        IpCacheKey, IpCacheSlot, IpScoreCache,
    };
    use fireworks_collaboration_lib::core::ip_pool::{IpCandidate, IpSource, IpStat};
    use std::net::{IpAddr, Ipv4Addr};
    #[test]
    fn cache_basic_operations() {
        let cache = IpScoreCache::new();
        // 插入 + 获取 + snapshot 验证
        {
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
            assert_eq!(fetched.best.unwrap().candidate.address, stat.candidate.address);
            assert_eq!(stat.sources, vec![IpSource::Builtin]);
            assert!(cache.snapshot().contains_key(&key));
        }
        // remove + clear 验证
        {
            cache.insert(
                IpCacheKey::new("github.com", 443),
                IpCacheSlot::with_best(IpStat::with_latency(
                    IpCandidate::new(IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1)), 443, IpSource::Builtin),
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
}

// ---------------- section_ip_pool_config ----------------
mod section_ip_pool_config {
    use fireworks_collaboration_lib::core::ip_pool::config::{
        default_cache_prune_interval_secs, default_cooldown_seconds,
        default_failure_rate_threshold, default_failure_threshold, default_failure_window_seconds,
        default_max_cache_entries, default_max_parallel_probes, default_min_samples_in_window,
        default_probe_timeout_ms, default_score_ttl_seconds, default_singleflight_timeout_ms,
        EffectiveIpPoolConfig, IpPoolFileConfig, IpPoolRuntimeConfig, PreheatDomain, UserStaticIp,
    };
    use fireworks_collaboration_lib::core::ip_pool::config::{
        join_ip_config_path, load_or_init_file_at, save_file_at,
    };
    use std::fs;
    use super::super::common::fixtures; // reuse common temp dir helper

    #[test]
    fn config_defaults_and_deserialize_behaviour() {
        // defaults (runtime + file)
        {
            let r = IpPoolRuntimeConfig::default();
            assert!(!r.enabled);
            assert_eq!(r.max_parallel_probes, default_max_parallel_probes());
            assert_eq!(r.probe_timeout_ms, default_probe_timeout_ms());
            assert!(r.history_path.is_none());
            assert!(r.sources.builtin && r.sources.dns && r.sources.history && r.sources.user_static && r.sources.fallback);
            assert_eq!(r.cache_prune_interval_secs, default_cache_prune_interval_secs());
            assert_eq!(r.max_cache_entries, default_max_cache_entries());
            assert_eq!(r.singleflight_timeout_ms, default_singleflight_timeout_ms());
            assert_eq!(r.failure_threshold, default_failure_threshold());
            assert_eq!(r.failure_rate_threshold, default_failure_rate_threshold());
            assert_eq!(r.failure_window_seconds, default_failure_window_seconds());
            assert_eq!(r.min_samples_in_window, default_min_samples_in_window());
            assert_eq!(r.cooldown_seconds, default_cooldown_seconds());
            assert!(r.circuit_breaker_enabled);
            let f = IpPoolFileConfig::default();
            assert!(f.preheat_domains.is_empty());
            assert_eq!(f.score_ttl_seconds, default_score_ttl_seconds());
            assert!(f.user_static.is_empty());
            assert!(f.blacklist.is_empty());
            assert!(f.whitelist.is_empty());
        }

        // deserialize + fill defaults
        {
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
            assert_eq!(cfg.runtime.cache_prune_interval_secs, default_cache_prune_interval_secs());
            assert_eq!(cfg.runtime.max_cache_entries, default_max_cache_entries());
            assert_eq!(cfg.runtime.singleflight_timeout_ms, default_singleflight_timeout_ms());
        }
    }

    // 合并文件持久化两个测试：减少重复锁/目录创建逻辑
    #[test]
    fn load_and_save_file_config_variants() {
        // variant A: 创建默认配置文件
        {
            let temp_dir = fixtures::create_empty_dir();
            let cfg = load_or_init_file_at(&temp_dir).expect("create default ip config");
            assert!(cfg.preheat_domains.is_empty());
            assert_eq!(cfg.score_ttl_seconds, default_score_ttl_seconds());
            assert!(join_ip_config_path(&temp_dir).exists());
            fs::remove_dir_all(&temp_dir).ok();
        }
        // variant B: 保存修改后再加载
        {
            let temp_dir = fixtures::create_empty_dir();
            let mut cfg = IpPoolFileConfig::default();
            cfg.preheat_domains.push(PreheatDomain::new("github.com"));
            cfg.score_ttl_seconds = 120;
            cfg.user_static.push(UserStaticIp { host: "github.com".into(), ip: "140.82.112.3".into(), ports: vec![443] });
            save_file_at(&cfg, &temp_dir).expect("save ip config");
            let loaded = load_or_init_file_at(&temp_dir).expect("load ip config");
            assert_eq!(loaded.preheat_domains.len(), 1);
            assert_eq!(loaded.score_ttl_seconds, 120);
            assert_eq!(loaded.user_static.len(), 1);
            fs::remove_dir_all(&temp_dir).ok();
        }
    }
}

// ---------------- section_circuit_breaker ----------------
mod section_circuit_breaker {
    use fireworks_collaboration_lib::core::ip_pool::circuit_breaker::{
        CircuitBreaker, CircuitBreakerConfig, CircuitState,
    };
    use std::net::IpAddr;

    #[test]
    fn circuit_breaker_comprehensive_behaviour() {
        // 场景 1: 连续失败触发熔断
        {
            let breaker = CircuitBreaker::new(CircuitBreakerConfig { enabled: true, consecutive_failure_threshold: 3, ..Default::default() });
            let ip = "192.0.2.1".parse().unwrap();
            breaker.record_failure(ip);
            breaker.record_failure(ip);
            assert!(!breaker.is_tripped(ip));
            breaker.record_failure(ip);
            assert!(breaker.is_tripped(ip));
            let stats = breaker.get_stats(ip).unwrap();
            assert_eq!(stats.state, CircuitState::Cooldown);
            assert_eq!(stats.consecutive_failures, 3);
        }

        // 场景 2: 中途成功重置连续失败
        {
            let breaker = CircuitBreaker::new(CircuitBreakerConfig { enabled: true, consecutive_failure_threshold: 3, failure_rate_threshold: 0.9, min_samples_in_window: 10, ..Default::default() });
            let ip = "192.0.2.2".parse().unwrap();
            breaker.record_failure(ip);
            breaker.record_failure(ip);
            breaker.record_success(ip);
            breaker.record_failure(ip);
            breaker.record_failure(ip);
            assert!(!breaker.is_tripped(ip));
        }

        // 场景 3: 失败率触发熔断
        {
            let breaker = CircuitBreaker::new(CircuitBreakerConfig { enabled: true, consecutive_failure_threshold: 100, failure_rate_threshold: 0.5, min_samples_in_window: 5, ..Default::default() });
            let ip = "192.0.2.3".parse().unwrap();
            breaker.record_success(ip);
            breaker.record_success(ip);
            breaker.record_failure(ip);
            breaker.record_failure(ip);
            breaker.record_failure(ip);
            assert!(breaker.is_tripped(ip));
        }

        // 场景 4: 手动重置
        {
            let breaker = CircuitBreaker::new(CircuitBreakerConfig { enabled: true, consecutive_failure_threshold: 2, ..Default::default() });
            let ip = "192.0.2.4".parse().unwrap();
            breaker.record_failure(ip);
            breaker.record_failure(ip);
            assert!(breaker.is_tripped(ip));
            breaker.reset_ip(ip);
            assert!(!breaker.is_tripped(ip));
        }

        // 场景 5: 禁用状态永不熔断
        {
            let breaker = CircuitBreaker::new(CircuitBreakerConfig { enabled: false, consecutive_failure_threshold: 1, ..Default::default() });
            let ip = "192.0.2.5".parse().unwrap();
            breaker.record_failure(ip);
            breaker.record_failure(ip);
            breaker.record_failure(ip);
            assert!(!breaker.is_tripped(ip));
        }

        // 场景 6: get_tripped_ips 返回正确集合
        {
            let breaker = CircuitBreaker::new(CircuitBreakerConfig { enabled: true, consecutive_failure_threshold: 2, ..Default::default() });
            let ip1: IpAddr = "192.0.2.6".parse().unwrap();
            let ip2: IpAddr = "192.0.2.7".parse().unwrap();
            let ip3: IpAddr = "192.0.2.8".parse().unwrap();
            breaker.record_failure(ip1);
            breaker.record_failure(ip1); // trip
            breaker.record_failure(ip2);
            breaker.record_success(ip2); // not trip
            breaker.record_failure(ip3);
            breaker.record_failure(ip3); // trip
            let tripped = breaker.get_tripped_ips();
            assert_eq!(tripped.len(), 2);
            assert!(tripped.contains(&ip1));
            assert!(tripped.contains(&ip3));
            assert!(!tripped.contains(&ip2));
        }
    }
}

// 已移除 section_ip_pool_events 模块：其事件发布测试与 section_event_emission 重复。

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
    use fireworks_collaboration_lib::core::ip_pool::IpSource;
    use super::super::common::ip_pool::make_history_record;
    use std::net::IpAddr;
    use std::sync::Arc;
    use tokio::runtime::Builder;
    use tokio::time::{Duration as TokioDuration, Instant};

    fn test_runtime() -> tokio::runtime::Runtime {
        Builder::new_current_thread().enable_all().build().unwrap()
    }

    #[test]
    fn preheat_operations_variants() {
        // builtin + user static lookup + schedule/backoff
        {
            let ips = builtin_lookup("github.com"); assert!(!ips.is_empty());
            let entries = vec![UserStaticIp { host: "example.com".into(), ip: "1.1.1.1".into(), ports: vec![80] }];
            assert!(user_static_lookup("example.com", 443, &entries).is_empty());
            assert_eq!(user_static_lookup("example.com", 80, &entries).len(), 1);
            let base = Instant::now();
            let mut sched = DomainSchedule::new(PreheatDomain::new("example.com"), 120, base);
            for expected in [240,480,720,720] { sched.mark_failure(base); assert_eq!(sched.current_backoff().as_secs(), expected); }
            sched.mark_success(base); assert_eq!(sched.current_backoff().as_secs(), 120);
            let mut second = DomainSchedule::new(PreheatDomain::new("refresh.com"), 300, base); second.mark_failure(base);
            let later = base + TokioDuration::from_secs(5); second.force_refresh(later); assert_eq!(second.failure_streak(),0); assert_eq!(second.next_due(), later);
            let mut early = DomainSchedule::new(PreheatDomain::new("early.com"),120,base); early.mark_success(base);
            let mut now_sched = DomainSchedule::new(PreheatDomain::new("now.com"),120,base); now_sched.force_refresh(base);
            let schedules = vec![early.clone(), now_sched.clone()];
            let (idx, due) = next_due_schedule(&schedules).unwrap(); assert_eq!(schedules[idx].domain.host, "now.com"); assert!(due <= early.next_due());
        }

        // collect_candidates builtin prefer + history merge + skip expired + update_cache_and_history + probe timeout
        {
            let rt = test_runtime();
            let history = Arc::new(IpHistoryStore::in_memory());
            // merge scenario
            let ip: IpAddr = "140.82.112.3".parse().unwrap();
            let future_expire = current_epoch_ms() + 60_000;
            history.upsert(make_history_record("github.com",443,[140,82,112,3],vec![IpSource::History,IpSource::UserStatic],12,future_expire-60_000,future_expire)).unwrap();
            let mut cfg = EffectiveIpPoolConfig::default(); cfg.runtime.enabled = true; cfg.runtime.sources.builtin = true; cfg.runtime.sources.history = true; cfg.runtime.sources.dns=false; cfg.runtime.sources.user_static=false; cfg.runtime.sources.fallback=false;
            let merged = rt.block_on(async { collect_candidates("github.com",443,&cfg,history.clone()).await });
            assert!(merged.iter().any(|c| c.candidate.address==ip && c.sources.contains(&IpSource::Builtin) && c.sources.contains(&IpSource::History) && c.sources.contains(&IpSource::UserStatic)));
            // skip expired
            let expired_store = Arc::new(IpHistoryStore::in_memory());
            expired_store.upsert(make_history_record("expired.test",443,[1,1,1,1],vec![IpSource::History],10,1,2)).unwrap();
            let mut cfg2 = EffectiveIpPoolConfig::default(); cfg2.runtime.enabled = true; cfg2.runtime.sources.history = true; for f in [&mut cfg2.runtime.sources.builtin,&mut cfg2.runtime.sources.dns,&mut cfg2.runtime.sources.user_static,&mut cfg2.runtime.sources.fallback] { *f = false; }
            let empty = rt.block_on(async { collect_candidates("expired.test",443,&cfg2,expired_store.clone()).await }); assert!(empty.is_empty()); assert!(expired_store.get("expired.test",443).is_none());
            // update_cache_and_history
            let cache = Arc::new(IpScoreCache::new()); let stat = AggregatedCandidate::new("1.1.1.1".parse().unwrap(),443,IpSource::Builtin).to_stat(10,60);
            update_cache_and_history("github.com",443,vec![stat],cache.clone(),history.clone()).unwrap();
            assert!(cache.get("github.com",443).is_some() && history.get("github.com",443).is_some());
            // probe latency timing
            let start = std::time::Instant::now(); let ip_probe: IpAddr = "198.51.100.1".parse().unwrap(); let timeout_ms=150; let result = rt.block_on(async { probe_latency(ip_probe,9,timeout_ms).await });
            let elapsed_ms = start.elapsed().as_millis() as u64; assert!(elapsed_ms <= timeout_ms * 2 + 50); if let Ok(lat)=result { assert!(u64::from(lat) <= timeout_ms * 2); }
        }
    }
}

// ---------------- section_history_tests ----------------
mod section_history_tests {
    use fireworks_collaboration_lib::core::ip_pool::history::IpHistoryStore;
    use fireworks_collaboration_lib::core::ip_pool::IpSource;
    use std::fs;
    use super::super::common::fixtures; // temp dir helper
    use super::super::common::ip_pool::{make_history_record, make_history_record_builtin};


    #[test]
    fn history_store_variants() {
        // init (dir + file)
        {
            let dir = fixtures::create_empty_dir();
            let store = IpHistoryStore::load_or_init_at(&dir).unwrap();
            assert!(IpHistoryStore::join_history_path(&dir).exists());
            assert!(store.snapshot().unwrap().is_empty());
            fs::remove_dir_all(&dir).ok();
            let base = fixtures::create_empty_dir();
            let path = base.join("nested").join("custom-history.json");
            let store_file = IpHistoryStore::load_or_init_from_file(&path).unwrap();
            assert!(path.exists() && store_file.snapshot().unwrap().is_empty());
            fs::remove_dir_all(&base).ok();
        }
        // CRUD + snapshot
        {
            let dir = fixtures::create_empty_dir();
            let store = IpHistoryStore::load_or_init_at(&dir).unwrap();
            let rec = make_history_record("github.com",443,[1,1,1,1],vec![IpSource::Builtin,IpSource::Dns],32,1,2);
            store.upsert(rec.clone()).unwrap();
            let fetched = store.get("github.com",443).unwrap();
            assert_eq!(fetched.latency_ms,32); assert_eq!(fetched.sources, vec![IpSource::Builtin,IpSource::Dns]);
            assert_eq!(store.snapshot().unwrap().len(),1); fs::remove_dir_all(&dir).ok();
        }
        // get_fresh 过期驱逐 + 有效
        {
            let store = IpHistoryStore::in_memory(); store.upsert(make_history_record_builtin("github.com",443,[1,1,1,1],10,1,5)).unwrap();
            assert!(store.get_fresh("github.com",443,10).is_none() && store.snapshot().unwrap().is_empty());
            let store2 = IpHistoryStore::in_memory(); let rec = make_history_record("github.com",443,[1,1,1,1],vec![IpSource::Builtin,IpSource::History],8,1,10_000); store2.upsert(rec.clone()).unwrap();
            let fetched = store2.get_fresh("github.com",443,5_000).unwrap(); assert_eq!(fetched.latency_ms,rec.latency_ms); assert_eq!(store2.snapshot().unwrap().len(),1);
        }
        // remove idempotent
        {
            let store = IpHistoryStore::in_memory(); store.upsert(make_history_record_builtin("github.com",443,[1,1,1,1],10,1,10_000)).unwrap();
            assert!(store.remove("github.com",443).unwrap()); assert!(store.get("github.com",443).is_none()); assert!(!store.remove("github.com",443).unwrap());
        }
        // enforce_capacity
        {
            let store = IpHistoryStore::in_memory(); for i in 0..5 { let rec = make_history_record_builtin(&format!("host{i}.com"),443,[1,1,1,i as u8 +1],10 + i as u32,(i+1) as i64 *1000,100_000); store.upsert(rec).unwrap(); }
            assert_eq!(store.enforce_capacity(3).unwrap(),2); let snap = store.snapshot().unwrap(); assert_eq!(snap.len(),3); assert!(snap.iter().all(|e| matches!(e.host.as_str(),"host2.com"|"host3.com"|"host4.com")));
        }
        // prune_and_enforce
        {
            let store = IpHistoryStore::in_memory(); for i in 0..2 { store.upsert(make_history_record_builtin(&format!("expired{i}.com"),443,[1,1,1,i as u8 +1],10,1000,5000)).unwrap(); }
            for i in 0..4 { store.upsert(make_history_record_builtin(&format!("valid{i}.com"),443,[2,2,2,i as u8 +1],20,(i+1) as i64 *2000 + 10000,100_000)).unwrap(); }
            let (expired,cap) = store.prune_and_enforce(10_000,3).unwrap(); assert_eq!(expired,2); assert_eq!(cap,1); let snap = store.snapshot().unwrap(); assert_eq!(snap.len(),3); assert!(snap.iter().all(|e| e.host.starts_with("valid")));
        }
    }
}
