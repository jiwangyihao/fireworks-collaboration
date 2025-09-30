//! P4.5 边界与异常场景测试：熔断、黑白名单、配置热重载、事件总线

#[path = "../common/mod.rs"]
mod common;

use fireworks_collaboration_lib::core::ip_pool::events::*;
use fireworks_collaboration_lib::core::ip_pool::config::{EffectiveIpPoolConfig, IpPoolFileConfig, IpPoolRuntimeConfig, UserStaticIp};
use fireworks_collaboration_lib::core::ip_pool::{IpPool, IpSource};
use fireworks_collaboration_lib::events::structured::{clear_test_event_bus, set_test_event_bus, Event, MemoryEventBus, StrategyEvent};
use std::net::{IpAddr, Ipv4Addr};
use std::sync::Arc;

#[test]
fn circuit_breaker_repeated_tripped_and_recovered() {
    let bus = MemoryEventBus::new();
    set_test_event_bus(Arc::new(bus.clone()));
    // 多次对同一IP触发熔断与恢复，事件应全部发射
    for _ in 0..3 {
        emit_ip_pool_ip_tripped(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)), "failures_exceeded");
        emit_ip_pool_ip_recovered(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)));
    }
    let events = bus.snapshot();
    assert_eq!(events.len(), 6);
    let tripped = events.iter().filter(|e| matches!(e, Event::Strategy(StrategyEvent::IpPoolIpTripped{..}))).count();
    let recovered = events.iter().filter(|e| matches!(e, Event::Strategy(StrategyEvent::IpPoolIpRecovered{..}))).count();
    assert_eq!(tripped, 3);
    assert_eq!(recovered, 3);
    clear_test_event_bus();
}

#[test]
fn blacklist_whitelist_empty_and_invalid_cidr() {
    let bus = MemoryEventBus::new();
    set_test_event_bus(Arc::new(bus.clone()));
    // 空名单不应发射事件
    // 但手动调用事件发射函数模拟极端情况
    emit_ip_pool_cidr_filter(IpAddr::V4(Ipv4Addr::new(1,2,3,4)), "blacklist", "");
    emit_ip_pool_cidr_filter(IpAddr::V4(Ipv4Addr::new(1,2,3,5)), "whitelist", "invalid_cidr");
    let events = bus.snapshot();
    assert_eq!(events.len(), 2);
    match &events[0] {
        Event::Strategy(StrategyEvent::IpPoolCidrFilter { ip, list_type, cidr }) => {
            assert_eq!(ip, "1.2.3.4");
            assert_eq!(list_type, "blacklist");
            assert_eq!(cidr, "");
        }
        _ => panic!("expected IpPoolCidrFilter event"),
    }
    match &events[1] {
        Event::Strategy(StrategyEvent::IpPoolCidrFilter { ip, list_type, cidr }) => {
            assert_eq!(ip, "1.2.3.5");
            assert_eq!(list_type, "whitelist");
            assert_eq!(cidr, "invalid_cidr");
        }
        _ => panic!("expected IpPoolCidrFilter event"),
    }
    clear_test_event_bus();
}

#[test]
fn config_hot_reload_concurrent() {
    let bus = MemoryEventBus::new();
    set_test_event_bus(Arc::new(bus.clone()));
    let dummy = EffectiveIpPoolConfig::default();
    // 并发多次热重载
    for _ in 0..5 {
        emit_ip_pool_config_update(&dummy, &dummy);
    }
    let events = bus.snapshot();
    assert_eq!(events.len(), 5);
    for evt in events {
        match evt {
            Event::Strategy(StrategyEvent::IpPoolConfigUpdate { old, new }) => {
                assert!(old.contains("EffectiveIpPoolConfig"));
                assert!(new.contains("EffectiveIpPoolConfig"));
            }
            _ => panic!("expected IpPoolConfigUpdate event"),
        }
    }
    clear_test_event_bus();
}

#[test]
fn event_bus_thread_safety_and_replacement() {
    use std::thread;
    let bus1 = Arc::new(MemoryEventBus::new());
    let bus2 = Arc::new(MemoryEventBus::new());
    set_test_event_bus(bus1.clone());
    // 多线程并发publish
    let handles: Vec<_> = (0..10).map(|i| {
        let bus = bus1.clone();
        thread::spawn(move || {
            emit_ip_pool_auto_disable(&format!("t{i}"), i as i64);
        })
    }).collect();
    for h in handles { h.join().unwrap(); }
    // 替换事件总线后再发射
    set_test_event_bus(bus2.clone());
    emit_ip_pool_auto_enable();
    // bus1应收到10条auto_disable，bus2收到1条auto_enable
    assert_eq!(bus1.snapshot().iter().filter(|e| matches!(e, Event::Strategy(StrategyEvent::IpPoolAutoDisable{..}))).count(), 10);
    assert_eq!(bus2.snapshot().iter().filter(|e| matches!(e, Event::Strategy(StrategyEvent::IpPoolAutoEnable{}))).count(), 1);
    clear_test_event_bus();
}
