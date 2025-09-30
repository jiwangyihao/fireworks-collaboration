/// Emit IpPoolCidrFilter event when a candidate is filtered by blacklist/whitelist.
pub fn emit_ip_pool_cidr_filter(ip: std::net::IpAddr, list_type: &str, cidr: &str) {
    tracing::info!(target = "ip_pool", ip = %ip, list_type, cidr, "ip filtered by cidr list");
    publish_global(Event::Strategy(StrategyEvent::IpPoolCidrFilter {
        ip: ip.to_string(),
        list_type: list_type.to_string(),
        cidr: cidr.to_string(),
    }));
}
/// Emit IpPoolIpTripped event when a single IP is tripped by circuit breaker.
pub fn emit_ip_pool_ip_tripped(ip: std::net::IpAddr, reason: &str) {
    tracing::warn!(target = "ip_pool", ip = %ip, reason, "ip tripped by circuit breaker");
    publish_global(Event::Strategy(StrategyEvent::IpPoolIpTripped {
        ip: ip.to_string(),
        reason: reason.to_string(),
    }));
}

/// Emit IpPoolIpRecovered event when a single IP is recovered from circuit breaker.
pub fn emit_ip_pool_ip_recovered(ip: std::net::IpAddr) {
    tracing::info!(target = "ip_pool", ip = %ip, "ip recovered from circuit breaker");
    publish_global(Event::Strategy(StrategyEvent::IpPoolIpRecovered {
        ip: ip.to_string(),
    }));
}
/// Emit IpPoolConfigUpdate event when the pool config is updated.
pub fn emit_ip_pool_config_update(old_config: &crate::core::ip_pool::config::EffectiveIpPoolConfig, new_config: &crate::core::ip_pool::config::EffectiveIpPoolConfig) {
    tracing::info!(target = "ip_pool", "ip pool config updated");
    publish_global(Event::Strategy(StrategyEvent::IpPoolConfigUpdate {
        old: format!("{:?}", old_config),
        new: format!("{:?}", new_config),
    }));
}
/// Emit IpPoolAutoDisable event when the pool is globally auto-disabled.
pub fn emit_ip_pool_auto_disable(reason: &str, until_ms: i64) {
    tracing::warn!(target = "ip_pool", reason, until_ms, "ip pool auto-disabled");
    publish_global(Event::Strategy(StrategyEvent::IpPoolAutoDisable {
        reason: reason.to_string(),
        until_ms,
    }));
}

/// Emit IpPoolAutoEnable event when the pool is auto-enabled after cooldown.
pub fn emit_ip_pool_auto_enable() {
    tracing::info!(target = "ip_pool", "ip pool auto-enable after cooldown");
    publish_global(Event::Strategy(StrategyEvent::IpPoolAutoEnable {}));
}
/// IP Pool event emission helpers for P4.4 observability.
///
/// Provides structured event emission for IP pool selection and refresh operations,
/// ensuring observability while respecting privacy (no raw IP addresses in events).

use crate::events::structured::{publish_global, Event, StrategyEvent};
use uuid::Uuid;

use super::{IpSelectionStrategy, IpStat};

/// Emit IpPoolSelection event when a candidate is selected for use.
pub fn emit_ip_pool_selection(
    task_id: Uuid,
    domain: &str,
    port: u16,
    strategy: IpSelectionStrategy,
    selected: Option<&IpStat>,
    candidates_count: u8,
) {
    let strategy_label = match strategy {
        IpSelectionStrategy::Cached => "Cached",
        IpSelectionStrategy::SystemDefault => "SystemDefault",
    };

    let (source, latency_ms) = match selected {
        Some(stat) => {
            let source = stat
                .sources
                .iter()
                .map(|s| format!("{:?}", s))
                .collect::<Vec<_>>()
                .join(",");
            (Some(source), stat.latency_ms)
        }
        None => (None, None),
    };

    tracing::debug!(
        target = "ip_pool",
        task_id = %task_id,
        domain = %domain,
        port = %port,
        strategy = %strategy_label,
        source = ?source,
        latency_ms = ?latency_ms,
        candidates_count = %candidates_count,
        "ip pool selection completed"
    );

    publish_global(Event::Strategy(StrategyEvent::IpPoolSelection {
        id: task_id.to_string(),
        domain: domain.to_string(),
        port,
        strategy: strategy_label.to_string(),
        source,
        latency_ms,
        candidates_count,
    }));
}

/// Emit IpPoolRefresh event when preheat or on-demand sampling completes.
pub fn emit_ip_pool_refresh(
    task_id: Uuid,
    domain: &str,
    success: bool,
    candidates: &[IpStat],
    reason: String,
) {
    let candidates_count = candidates.len().min(255) as u8;
    let (min_latency_ms, max_latency_ms) = if candidates.is_empty() {
        (None, None)
    } else {
        let latencies: Vec<u32> = candidates.iter().filter_map(|c| c.latency_ms).collect();
        if latencies.is_empty() {
            (None, None)
        } else {
            let min = latencies.iter().min().copied();
            let max = latencies.iter().max().copied();
            (min, max)
        }
    };

    tracing::debug!(
        target = "ip_pool",
        task_id = %task_id,
        domain = %domain,
        success = %success,
        candidates_count = %candidates_count,
        min_latency_ms = ?min_latency_ms,
        max_latency_ms = ?max_latency_ms,
        reason = %reason,
        "ip pool refresh completed"
    );

    publish_global(Event::Strategy(StrategyEvent::IpPoolRefresh {
        id: task_id.to_string(),
        domain: domain.to_string(),
        success,
        candidates_count,
        min_latency_ms,
        max_latency_ms,
        reason,
    }));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::structured::MemoryEventBus;
    use std::net::{IpAddr, Ipv4Addr};
    use std::sync::Arc;
    use std::time::{SystemTime, UNIX_EPOCH};

    use crate::core::ip_pool::{IpCandidate, IpSource};

    #[test]
    fn emit_ip_pool_selection_publishes_event() {
        use crate::events::structured::{clear_test_event_bus, set_test_event_bus};
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
        use crate::events::structured::{clear_test_event_bus, set_test_event_bus};
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
        use crate::events::structured::{clear_test_event_bus, set_test_event_bus};
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
