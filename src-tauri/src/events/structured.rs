use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};
use std::any::Any;
use std::sync::{Arc, Mutex, RwLock};

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
    IpPoolIpTripped { ip: String, reason: String },
    /// 单IP熔断恢复事件
    IpPoolIpRecovered { ip: String },
    /// IP池配置热重载事件
    IpPoolConfigUpdate { old: String, new: String },
    /// IP池全局自动禁用事件
    IpPoolAutoDisable { reason: String, until_ms: i64 },
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
    /// 代理状态变更事件
    ProxyState {
        id: String,
        state: String, // "enabled", "disabled", "fallback", "recovering"
        mode: String,  // "off", "http", "socks5", "system"
        reason: Option<String>,
    },
    /// 代理降级事件
    ProxyFallback {
        id: String,
        reason: String,
        failure_count: u32,
        window_seconds: u64,
    },
    /// 代理恢复事件
    ProxyRecovered {
        id: String,
        cooldown_seconds: u64,
        consecutive_successes: u32,
    },
    /// 代理健康检查事件
    ProxyHealthCheck {
        id: String,
        success: bool,
        latency_ms: Option<u32>,
        probe_url: String,
    },
    /// 系统代理检测事件
    SystemProxyDetected {
        id: String,
        success: bool,
        mode: Option<String>,
        url: Option<String>,
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
    fn as_any(&self) -> &dyn Any;
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

    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[derive(Default)]
pub struct FanoutEventBus {
    listeners: RwLock<Vec<Arc<dyn EventBusAny>>>,
    memory_listeners: RwLock<Vec<MemoryEventBus>>,
}

impl FanoutEventBus {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&self, bus: Arc<dyn EventBusAny>) {
        if bus.as_ref().as_any().is::<FanoutEventBus>() {
            return;
        }
        if let Some(mem) = bus.as_ref().as_any().downcast_ref::<MemoryEventBus>() {
            if let Ok(mut guard) = self.memory_listeners.write() {
                guard.push(mem.clone());
            }
        }
        if let Ok(mut guard) = self.listeners.write() {
            guard.push(bus);
        }
    }

    fn snapshot_memory(&self) -> Option<MemoryEventBus> {
        self.memory_listeners
            .read()
            .ok()
            .and_then(|guard| guard.first().cloned())
    }
}

impl EventBus for FanoutEventBus {
    fn publish(&self, evt: Event) {
        let listeners = match self.listeners.read() {
            Ok(guard) => guard.clone(),
            Err(_) => return,
        };
        for bus in listeners {
            bus.publish(evt.clone());
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

// ====== 全局可选事件总线（T1 引入，后续任务/策略可选择双写） ======
static GLOBAL_BUS: OnceCell<Arc<dyn EventBusAny>> = OnceCell::new();
static FANOUT_BUS: OnceCell<Arc<FanoutEventBus>> = OnceCell::new();

pub fn set_global_event_bus(bus: Arc<dyn EventBusAny>) -> Result<(), &'static str> {
    if let Some(fanout) = FANOUT_BUS.get() {
        fanout.register(bus);
        Ok(())
    } else {
        GLOBAL_BUS
            .set(bus)
            .map_err(|_| "global event bus already set")
    }
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
    if let Some(fanout) = FANOUT_BUS.get() {
        if let Some(bus) = fanout.snapshot_memory() {
            return Some(bus);
        }
    }
    GLOBAL_BUS.get().and_then(|b| {
        b.as_ref()
            .as_any()
            .downcast_ref::<MemoryEventBus>()
            .cloned()
    })
}

pub fn ensure_fanout_bus() -> Result<Arc<FanoutEventBus>, &'static str> {
    if let Some(bus) = FANOUT_BUS.get() {
        return Ok(bus.clone());
    }
    if GLOBAL_BUS.get().is_some() {
        return Err("global event bus already set");
    }
    let fanout = Arc::new(FanoutEventBus::new());
    let fanout_arc: Arc<dyn EventBusAny> = fanout.clone();
    GLOBAL_BUS
        .set(fanout_arc)
        .map_err(|_| "global event bus already set")?;
    FANOUT_BUS
        .set(fanout.clone())
        .map_err(|_| "fanout bus already set")?;
    Ok(fanout)
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
