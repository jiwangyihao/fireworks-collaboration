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
            candidate: IpCandidate::new(
                IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2)),
                443,
                IpSource::Dns,
            ),
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
