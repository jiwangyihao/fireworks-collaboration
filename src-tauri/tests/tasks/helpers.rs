#![cfg(not(feature = "tauri-app"))]
//! Tasks 模块综合测试
//! 合并了 `tasks/ip_pool_preheat_events.rs` 和 `tasks/registry/git/helpers_tests.rs`

use crate::common::test_env::init_test_env;

#[ctor::ctor]
fn __init_env() {
    init_test_env();
}

// ============================================================================
// tasks/ip_pool_preheat_events.rs 的测试
// ============================================================================

// IP Pool Preheat Event Emission Tests (P4.4)
// -------------------------------------------
// 验证预热过程中 IpPoolRefresh 事件的正确发射，包括：
// - 成功预热路径（reason="preheat"）
// - 无候选失败路径（reason="no_candidates"）
// - 全部探测失败路径（reason="all_probes_failed"）

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
                }) => Some((
                    domain,
                    success,
                    candidates_count,
                    min_latency_ms,
                    max_latency_ms,
                    reason,
                )),
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
                }) => Some((
                    domain,
                    success,
                    candidates_count,
                    min_latency_ms,
                    max_latency_ms,
                    reason,
                )),
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

// ============================================================================
// tasks/registry/git/helpers_tests.rs 的测试
// ============================================================================

mod section_registry_git_helpers {
    use fireworks_collaboration_lib::core::config::model::{AppConfig, RetryCfg};
    use fireworks_collaboration_lib::core::git::default_impl::opts::{
        StrategyHttpOverride, StrategyRetryOverride,
    };
    use fireworks_collaboration_lib::core::tasks::registry::TaskRegistry;
    use uuid::Uuid;

    #[test]
    fn no_http_override() {
        let global = AppConfig::default();
        let (f, m, changed, conflict) =
            TaskRegistry::apply_http_override("GitClone", &Uuid::nil(), &global, None);
        assert_eq!(f, global.http.follow_redirects);
        assert_eq!(m, global.http.max_redirects);
        assert!(!changed);
        assert!(conflict.is_none());
    }

    #[test]
    fn http_override_changes() {
        let global = AppConfig::default();
        let over = StrategyHttpOverride {
            follow_redirects: Some(!global.http.follow_redirects),
            max_redirects: Some(3),
        };
        let (f, m, changed, conflict) =
            TaskRegistry::apply_http_override("GitClone", &Uuid::nil(), &global, Some(&over));
        if !global.http.follow_redirects {
            assert!(f);
        }
        if !f {
            assert_eq!(m, 0);
            assert!(conflict.is_some());
        } else {
            assert_eq!(m, 3);
            assert!(conflict.is_none());
        }
        assert!(changed);
    }

    #[test]
    fn http_override_clamp_applies() {
        let global = AppConfig::default();
        let over = StrategyHttpOverride {
            follow_redirects: None,
            max_redirects: Some(99),
        };
        let (_f, m, changed, _conflict) =
            TaskRegistry::apply_http_override("GitClone", &Uuid::nil(), &global, Some(&over));
        assert_eq!(m, 20);
        assert!(changed);
    }

    #[test]
    fn no_retry_override() {
        let global = RetryCfg::default();
        let (plan, changed) = TaskRegistry::apply_retry_override(&global, None);
        assert_eq!(plan.max, global.max);
        assert_eq!(plan.base_ms, global.base_ms);
        assert!(!changed);
    }

    #[test]
    fn retry_override_changes() {
        let mut global = RetryCfg::default();
        global.max = 6;
        let over = StrategyRetryOverride {
            max: Some(3),
            base_ms: Some(500),
            factor: Some(2.0),
            jitter: Some(false),
        };
        let (plan, changed) = TaskRegistry::apply_retry_override(&global, Some(&over));
        assert!(changed);
        assert_eq!(plan.max, 3);
        assert_eq!(plan.base_ms, 500);
        assert!((plan.factor - 2.0).abs() < f64::EPSILON);
        assert!(!plan.jitter);
    }
}
