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
        assert!(pool.cache().get("keep.test", 443).is_some());
        assert!(pool.history().get("keep.test", 443).is_some());
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
