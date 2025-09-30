//! P4.5 新增事件发射点测试：CIDR过滤、单IP熔断、配置热重载、全局自动禁用/恢复
//! - 覆盖 emit_ip_pool_cidr_filter, emit_ip_pool_ip_tripped, emit_ip_pool_ip_recovered, emit_ip_pool_config_update, emit_ip_pool_auto_disable, emit_ip_pool_auto_enable

#[path = "../common/mod.rs"]
mod common;

use fireworks_collaboration_lib::core::ip_pool::events::*;
use fireworks_collaboration_lib::core::ip_pool::config::EffectiveIpPoolConfig;
use fireworks_collaboration_lib::events::structured::{
    clear_test_event_bus, set_test_event_bus, Event, MemoryEventBus, StrategyEvent,
};
use std::net::IpAddr;
use std::sync::Arc;

#[test]
fn emit_ip_pool_cidr_filter_publishes_event() {
    let bus = MemoryEventBus::new();
    set_test_event_bus(Arc::new(bus.clone()));
    emit_ip_pool_cidr_filter("192.168.1.1".parse().unwrap(), "blacklist", "192.168.1.0/24");
    let events = bus.snapshot();
    assert_eq!(events.len(), 1);
    match &events[0] {
        Event::Strategy(StrategyEvent::IpPoolCidrFilter { ip, list_type, cidr }) => {
            assert_eq!(ip, "192.168.1.1");
            assert_eq!(list_type, "blacklist");
            assert_eq!(cidr, "192.168.1.0/24");
        }
        _ => panic!("expected IpPoolCidrFilter event"),
    }
    clear_test_event_bus();
}

#[test]
fn emit_ip_pool_ip_tripped_and_recovered_publish_events() {
    let bus = MemoryEventBus::new();
    set_test_event_bus(Arc::new(bus.clone()));
    emit_ip_pool_ip_tripped("10.0.0.2".parse().unwrap(), "failures_exceeded");
    emit_ip_pool_ip_recovered("10.0.0.2".parse().unwrap());
    let events = bus.snapshot();
    assert_eq!(events.len(), 2);
    match &events[0] {
        Event::Strategy(StrategyEvent::IpPoolIpTripped { ip, reason }) => {
            assert_eq!(ip, "10.0.0.2");
            assert_eq!(reason, "failures_exceeded");
        }
        _ => panic!("expected IpPoolIpTripped event"),
    }
    match &events[1] {
        Event::Strategy(StrategyEvent::IpPoolIpRecovered { ip }) => {
            assert_eq!(ip, "10.0.0.2");
        }
        _ => panic!("expected IpPoolIpRecovered event"),
    }
    clear_test_event_bus();
}

#[test]
fn emit_ip_pool_config_update_publishes_event() {
    let bus = MemoryEventBus::new();
    set_test_event_bus(Arc::new(bus.clone()));
    let dummy = EffectiveIpPoolConfig::default();
    emit_ip_pool_config_update(&dummy, &dummy);
    let events = bus.snapshot();
    assert_eq!(events.len(), 1);
    match &events[0] {
        Event::Strategy(StrategyEvent::IpPoolConfigUpdate { old, new }) => {
            assert!(old.contains("EffectiveIpPoolConfig"));
            assert!(new.contains("EffectiveIpPoolConfig"));
        }
        _ => panic!("expected IpPoolConfigUpdate event"),
    }
    clear_test_event_bus();
}

#[test]
fn emit_ip_pool_auto_disable_and_enable_publish_events() {
    let bus = MemoryEventBus::new();
    set_test_event_bus(Arc::new(bus.clone()));
    emit_ip_pool_auto_disable("manual_test", 1234567890);
    emit_ip_pool_auto_enable();
    let events = bus.snapshot();
    assert_eq!(events.len(), 2);
    match &events[0] {
        Event::Strategy(StrategyEvent::IpPoolAutoDisable { reason, until_ms }) => {
            assert_eq!(reason, "manual_test");
            assert_eq!(*until_ms, 1234567890);
        }
        _ => panic!("expected IpPoolAutoDisable event"),
    }
    match &events[1] {
        Event::Strategy(StrategyEvent::IpPoolAutoEnable {}) => {}
        _ => panic!("expected IpPoolAutoEnable event"),
    }
    clear_test_event_bus();
}
