use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};
use std::any::Any;
use std::sync::{Arc, Mutex};

/// 任务相关事件（示例：最小子集，后续可扩展）
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TaskEvent {
    Started {
        id: String,
        kind: String,
    },
    Completed {
        id: String,
    },
    Canceled {
        id: String,
    },
    Failed {
        id: String,
        category: String,
        code: Option<String>,
        message: String,
    },
}

/// 策略相关事件
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PolicyEvent {
    RetryApplied {
        id: String,
        code: String,
        changed: Vec<String>,
    },
}

/// 传输/能力相关事件（预留）
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TransportEvent {
    CapabilityDetected {
        id: String,
        caps: Vec<String>,
    },
    PartialFilterFallback {
        id: String,
        shallow: bool,
        message: String,
    },
    PartialFilterUnsupported {
        id: String,
        requested: String,
    },
    PartialFilterCapability {
        id: String,
        supported: bool,
    },
}

/// 策略相关事件：覆盖 HTTP/TLS/冲突/汇总
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum StrategyEvent {
    /// IP被黑白名单（CIDR）过滤事件
    IpPoolCidrFilter {
        ip: String,
        list_type: String,
        cidr: String,
    },
    /// 单IP被熔断事件
    IpPoolIpTripped {
        ip: String,
        reason: String,
    },
    /// 单IP熔断恢复事件
    IpPoolIpRecovered {
        ip: String,
    },
    /// IP池配置热重载事件
    IpPoolConfigUpdate {
        old: String,
        new: String,
    },
    /// IP池全局自动禁用事件
    IpPoolAutoDisable {
        reason: String,
        until_ms: i64,
    },
    /// IP池全局自动恢复事件
    IpPoolAutoEnable {},
    HttpApplied {
        id: String,
        follow: bool,
        max_redirects: u8,
    },
    TlsApplied {
        id: String,
        insecure_skip_verify: bool,
        skip_san_whitelist: bool,
    },
    Conflict {
        id: String,
        kind: String,
        message: String,
    },
    Summary {
        id: String,
        kind: String,
        http_follow: bool,
        http_max: u8,
        retry_max: u32,
        retry_base_ms: u64,
        retry_factor: f64,
        retry_jitter: bool,
        tls_insecure: bool,
        tls_skip_san: bool,
        applied_codes: Vec<String>,
        filter_requested: bool,
    },
    AdaptiveTlsRollout {
        id: String,
        kind: String,
        percent_applied: u8,
        sampled: bool,
    },
    IgnoredFields {
        id: String,
        kind: String,
        top_level: Vec<String>,
        nested: Vec<String>,
    },
    AdaptiveTlsTiming {
        id: String,
        kind: String,
        used_fake_sni: bool,
        fallback_stage: String,
        connect_ms: Option<u32>,
        tls_ms: Option<u32>,
        first_byte_ms: Option<u32>,
        total_ms: Option<u32>,
        cert_fp_changed: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        ip_source: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        ip_latency_ms: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        ip_selection_stage: Option<String>,
    },
    AdaptiveTlsFallback {
        id: String,
        kind: String,
        from: String,
        to: String,
        reason: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        ip_source: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        ip_latency_ms: Option<u32>,
    },
    AdaptiveTlsAutoDisable {
        id: String,
        kind: String,
        enabled: bool,
        threshold_pct: u8,
        cooldown_secs: u32,
    },
    CertFingerprintChanged {
        id: String,
        host: String,
        spki_sha256: String,
        cert_sha256: String,
    },
    CertFpPinMismatch {
        id: String,
        host: String,
        spki_sha256: String,
        pin_count: u8,
    },
    IpPoolSelection {
        id: String,
        domain: String,
        port: u16,
        strategy: String,
        source: Option<String>,
        latency_ms: Option<u32>,
        candidates_count: u8,
    },
    IpPoolRefresh {
        id: String,
        domain: String,
        success: bool,
        candidates_count: u8,
        min_latency_ms: Option<u32>,
        max_latency_ms: Option<u32>,
        reason: String,
    },
}

/// 统一顶层事件枚举
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "data")]
pub enum Event {
    Task(TaskEvent),
    Policy(PolicyEvent),
    Transport(TransportEvent),
    Strategy(StrategyEvent),
}

/// 事件总线 trait：T0 阶段最小能力
pub trait EventBus: Send + Sync + 'static {
    fn publish(&self, evt: Event);
}

// 为 downcast 提供标记 trait
pub trait EventBusAny: EventBus + Any {}
impl<T: EventBus + Any> EventBusAny for T {}

/// 内存事件总线（测试与开发期使用）
#[derive(Clone, Default)]
pub struct MemoryEventBus {
    inner: Arc<Mutex<Vec<Event>>>,
}

impl MemoryEventBus {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn take_all(&self) -> Vec<Event> {
        if let Ok(mut g) = self.inner.lock() {
            let out = g.clone();
            g.clear();
            out
        } else {
            Vec::new()
        }
    }
    pub fn snapshot(&self) -> Vec<Event> {
        if let Ok(g) = self.inner.lock() {
            g.clone()
        } else {
            Vec::new()
        }
    }
}

impl EventBus for MemoryEventBus {
    fn publish(&self, evt: Event) {
        if let Ok(mut g) = self.inner.lock() {
            g.push(evt);
        }
    }
}

// ====== 全局可选事件总线（T1 引入，后续任务/策略可选择双写） ======
static GLOBAL_BUS: OnceCell<Arc<dyn EventBusAny>> = OnceCell::new();

pub fn set_global_event_bus(bus: Arc<dyn EventBusAny>) -> Result<(), &'static str> {
    GLOBAL_BUS
        .set(bus)
        .map_err(|_| "global event bus already set")
}

pub fn publish_global(evt: Event) {
    // 允许线程局部覆盖（集成测试 crate 也可使用）
    if let Some(bus) = TEST_OVERRIDE_BUS.with(|cell| cell.borrow().clone()) {
        bus.publish(evt.clone());
    }
    if let Some(bus) = GLOBAL_BUS.get() {
        bus.publish(evt);
    }
}

/// 若全局已设置且为 MemoryEventBus，获取其克隆副本（共享同一内部存储）。
pub fn get_global_memory_bus() -> Option<MemoryEventBus> {
    GLOBAL_BUS.get().and_then(|b| {
        // 直接引用方式 downcast_ref
        let any_ref = b.as_ref() as &dyn Any;
        any_ref.downcast_ref::<MemoryEventBus>().map(|m| m.clone())
    })
}

// ==== 测试覆盖专用：线程局部可替换总线（不影响生产 OnceCell） ====
thread_local! {
    static TEST_OVERRIDE_BUS: std::cell::RefCell<Option<Arc<dyn EventBusAny>>> = const { std::cell::RefCell::new(None) };
}

pub fn set_test_event_bus(bus: Arc<dyn EventBusAny>) {
    TEST_OVERRIDE_BUS.with(|cell| *cell.borrow_mut() = Some(bus));
}

pub fn clear_test_event_bus() {
    TEST_OVERRIDE_BUS.with(|cell| *cell.borrow_mut() = None);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_event_bus_basic() {
        let bus = MemoryEventBus::new();
        bus.publish(Event::Task(TaskEvent::Started {
            id: "1".into(),
            kind: "GitClone".into(),
        }));
        bus.publish(Event::Policy(PolicyEvent::RetryApplied {
            id: "1".into(),
            code: "retry_strategy_override_applied".to_string(),
            changed: vec!["max".to_string()],
        }));
        let snapshot = bus.snapshot();
        assert_eq!(snapshot.len(), 2);
        // take_all should clear
        let taken = bus.take_all();
        assert_eq!(taken.len(), 2);
        assert!(bus.take_all().is_empty());
    }
}
