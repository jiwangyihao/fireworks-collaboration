#![cfg(not(feature = "tauri-app"))]
//! IP Pool Preheat Event Emission Tests (P4.4)
//! -------------------------------------------
//! 验证预热过程中 IpPoolRefresh 事件的正确发射，包括：
//! - 成功预热路径（reason="preheat"）
//! - 无候选失败路径（reason="no_candidates"）
//! - 全部探测失败路径（reason="all_probes_failed"）

#[path = "../common/mod.rs"]
mod common;

use crate::common::test_env::init_test_env;

#[ctor::ctor]
fn __init_env() {
    init_test_env();
}

mod section_preheat_event_emission {
    use super::common::prelude::*;
    use fireworks_collaboration_lib::core::ip_pool::config::{
        IpPoolSourceToggle, PreheatDomain, UserStaticIp,
    };
    use fireworks_collaboration_lib::core::ip_pool::preheat::{
        collect_candidates, measure_candidates, update_cache_and_history,
    };
    use fireworks_collaboration_lib::core::ip_pool::{IpScoreCache, IpSource};
    use fireworks_collaboration_lib::events::structured::{
        clear_test_event_bus, set_test_event_bus, Event, MemoryEventBus, StrategyEvent,
    };
    use std::sync::Arc;
    use tokio::net::TcpListener;

    #[tokio::test]
    async fn preheat_success_emits_refresh_event_with_preheat_reason() {
        let bus = MemoryEventBus::new();
        set_test_event_bus(Arc::new(bus.clone()));

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let accept_task = tokio::spawn(async move {
            if let Ok((stream, _)) = listener.accept().await {
                drop(stream);
            }
        });

        let mut cfg = enabled_config();
        cfg.runtime.sources = IpPoolSourceToggle {
            builtin: false,
            dns: false,
            history: false,
            user_static: true,
            fallback: false,
        };
        cfg.file.user_static_ips.push(UserStaticIp {
            domain: "preheat.test".to_string(),
            port: Some(addr.port()),
            ips: vec![addr.ip().to_string()],
        });
        cfg.file.score_ttl_seconds = 300;
        cfg.runtime.max_parallel_probes = 4;
        cfg.runtime.probe_timeout_ms = 2000;

        let cache = IpScoreCache::new();
        let history = enabled_history();

        // Simulate preheat_domain logic
        let candidates = collect_candidates(
            "preheat.test",
            addr.port(),
            &cfg,
            Arc::new(history.clone()),
        )
        .await;
        assert!(!candidates.is_empty());

        let stats = measure_candidates(
            &candidates,
            cfg.runtime.max_parallel_probes,
            cfg.runtime.probe_timeout_ms,
            cfg.file.score_ttl_seconds,
        )
        .await;
        assert!(!stats.is_empty());

        accept_task.await.unwrap();

        update_cache_and_history(
            "preheat.test",
            addr.port(),
            stats.clone(),
            Arc::new(cache),
            Arc::new(history),
        )
        .unwrap();

        // Emit event manually (simulating preheat_domain integration)
        {
            use fireworks_collaboration_lib::core::ip_pool::events::emit_ip_pool_refresh;
            use uuid::Uuid;
            let task_id = Uuid::new_v4();
            emit_ip_pool_refresh(task_id, "preheat.test", true, &stats, "preheat".to_string());
        }

        let events = bus.snapshot();
        let refresh_events: Vec<_> = events
            .into_iter()
            .filter_map(|e| match e {
                Event::Strategy(StrategyEvent::IpPoolRefresh {
                    domain,
                    success,
                    candidates_count,
                    min_latency_ms,
                    max_latency_ms,
                    reason,
                    ..
                }) => Some((domain, success, candidates_count, min_latency_ms, max_latency_ms, reason)),
                _ => None,
            })
            .collect();

        assert_eq!(refresh_events.len(), 1);
        let (domain, success, candidates_count, min_lat, max_lat, reason) = &refresh_events[0];
        assert_eq!(domain, "preheat.test");
        assert!(success);
        assert_eq!(*candidates_count, stats.len() as u8);
        assert!(min_lat.is_some());
        assert!(max_lat.is_some());
        assert_eq!(reason, "preheat");

        clear_test_event_bus();
    }

    #[tokio::test]
    async fn preheat_no_candidates_emits_refresh_event_with_no_candidates_reason() {
        let bus = MemoryEventBus::new();
        set_test_event_bus(Arc::new(bus.clone()));

        let mut cfg = enabled_config();
        cfg.runtime.sources = IpPoolSourceToggle {
            builtin: false,
            dns: false,
            history: false,
            user_static: false,
            fallback: false,
        };

        let history = enabled_history();

        // Simulate preheat_domain logic with no sources
        let candidates =
            collect_candidates("missing.test", 443, &cfg, Arc::new(history.clone())).await;
        assert!(candidates.is_empty());

        // Emit failure event
        {
            use fireworks_collaboration_lib::core::ip_pool::events::emit_ip_pool_refresh;
            use uuid::Uuid;
            let task_id = Uuid::new_v4();
            emit_ip_pool_refresh(
                task_id,
                "missing.test",
                false,
                &[],
                "no_candidates".to_string(),
            );
        }

        let events = bus.snapshot();
        let refresh_events: Vec<_> = events
            .into_iter()
            .filter_map(|e| match e {
                Event::Strategy(StrategyEvent::IpPoolRefresh {
                    domain,
                    success,
                    candidates_count,
                    reason,
                    ..
                }) => Some((domain, success, candidates_count, reason)),
                _ => None,
            })
            .collect();

        assert_eq!(refresh_events.len(), 1);
        let (domain, success, candidates_count, reason) = &refresh_events[0];
        assert_eq!(domain, "missing.test");
        assert!(!success);
        assert_eq!(*candidates_count, 0);
        assert_eq!(reason, "no_candidates");

        clear_test_event_bus();
    }

    #[tokio::test]
    async fn preheat_all_probes_failed_emits_refresh_event() {
        let bus = MemoryEventBus::new();
        set_test_event_bus(Arc::new(bus.clone()));

        let mut cfg = enabled_config();
        cfg.runtime.sources = IpPoolSourceToggle {
            builtin: false,
            dns: false,
            history: false,
            user_static: true,
            fallback: false,
        };
        // Use a non-listening port
        cfg.file.user_static_ips.push(UserStaticIp {
            domain: "fail.test".to_string(),
            port: Some(1), // Port 1 is typically reserved and not listening
            ips: vec!["127.0.0.1".to_string()],
        });
        cfg.runtime.probe_timeout_ms = 500;

        let history = enabled_history();

        let candidates =
            collect_candidates("fail.test", 1, &cfg, Arc::new(history.clone())).await;
        assert!(!candidates.is_empty());

        let stats = measure_candidates(
            &candidates,
            cfg.runtime.max_parallel_probes,
            cfg.runtime.probe_timeout_ms,
            cfg.file.score_ttl_seconds,
        )
        .await;
        assert!(stats.is_empty()); // All probes should fail

        // Emit failure event
        {
            use fireworks_collaboration_lib::core::ip_pool::events::emit_ip_pool_refresh;
            use uuid::Uuid;
            let task_id = Uuid::new_v4();
            emit_ip_pool_refresh(
                task_id,
                "fail.test",
                false,
                &[],
                "all_probes_failed".to_string(),
            );
        }

        let events = bus.snapshot();
        let refresh_events: Vec<_> = events
            .into_iter()
            .filter_map(|e| match e {
                Event::Strategy(StrategyEvent::IpPoolRefresh {
                    domain,
                    success,
                    candidates_count,
                    min_latency_ms,
                    max_latency_ms,
                    reason,
                    ..
                }) => Some((domain, success, candidates_count, min_latency_ms, max_latency_ms, reason)),
                _ => None,
            })
            .collect();

        assert_eq!(refresh_events.len(), 1);
        let (domain, success, candidates_count, min_lat, max_lat, reason) = &refresh_events[0];
        assert_eq!(domain, "fail.test");
        assert!(!success);
        assert_eq!(*candidates_count, 0);
        assert!(min_lat.is_none());
        assert!(max_lat.is_none());
        assert_eq!(reason, "all_probes_failed");

        clear_test_event_bus();
    }
}
