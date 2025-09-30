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
    use fireworks_collaboration_lib::core::ip_pool::cache::{IpCandidate, IpStat};
    use fireworks_collaboration_lib::core::ip_pool::events::emit_ip_pool_refresh;
    use fireworks_collaboration_lib::core::ip_pool::IpSource;
    use fireworks_collaboration_lib::events::structured::{
        clear_test_event_bus, set_test_event_bus, Event, MemoryEventBus, StrategyEvent,
    };
    use std::net::{IpAddr, Ipv4Addr};
    use std::sync::Arc;
    use uuid::Uuid;

    fn sample_stat(latency_ms: u32) -> IpStat {
        let candidate = IpCandidate::new(
            IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
            443,
            IpSource::UserStatic,
        );
        IpStat::with_latency(candidate, latency_ms)
    }

    #[test]
    fn preheat_success_emits_refresh_event_with_preheat_reason() {
        let bus = MemoryEventBus::new();
        set_test_event_bus(Arc::new(bus.clone()));

        let stats = vec![sample_stat(42)];
        emit_ip_pool_refresh(
            Uuid::new_v4(),
            "preheat.test",
            true,
            &stats,
            "preheat".to_string(),
        );

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
        assert_eq!(min_lat, &Some(42));
        assert_eq!(max_lat, &Some(42));
        assert_eq!(reason, "preheat");

        clear_test_event_bus();
    }

    #[test]
    fn preheat_no_candidates_emits_refresh_event_with_no_candidates_reason() {
        let bus = MemoryEventBus::new();
        set_test_event_bus(Arc::new(bus.clone()));

        emit_ip_pool_refresh(
            Uuid::new_v4(),
            "missing.test",
            false,
            &[],
            "no_candidates".to_string(),
        );

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

    #[test]
    fn preheat_all_probes_failed_emits_refresh_event() {
        let bus = MemoryEventBus::new();
        set_test_event_bus(Arc::new(bus.clone()));

        emit_ip_pool_refresh(
            Uuid::new_v4(),
            "fail.test",
            false,
            &[],
            "all_probes_failed".to_string(),
        );

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
