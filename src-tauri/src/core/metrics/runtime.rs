use once_cell::sync::OnceCell;
use rand::{rngs::OsRng, RngCore};
use sha2::{Digest, Sha256};
use std::cell::RefCell;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicU8, Ordering};
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::{Duration, Instant};

use crate::core::config::model::{
    ObservabilityConfig, ObservabilityPerformanceConfig, ObservabilityRedactIpMode,
};
use crate::events::structured::{publish_global, Event, StrategyEvent};

use super::aggregate::HistogramWindowConfig;
use super::descriptors::METRIC_MEMORY_PRESSURE_TOTAL;
use super::{MetricDescriptor, MetricInitError, MetricRegistry};
use super::{AGGREGATOR, REGISTRY};

static RUNTIME: OnceCell<Arc<MetricRuntime>> = OnceCell::new();

pub(super) fn init(
    cfg: &ObservabilityConfig,
    registry: Arc<MetricRegistry>,
) -> Result<(), MetricInitError> {
    if RUNTIME.get().is_some() {
        return Ok(());
    }

    let performance = cfg.performance.clone();
    registry.configure_histogram_sharding(performance.enable_sharding);
    let (runtime, receiver) = MetricRuntime::new(registry, performance);
    runtime.spawn_worker(receiver)?;
    let _ = RUNTIME.set(runtime);
    Ok(())
}

pub(super) fn record_counter(desc: MetricDescriptor, labels: &[(&'static str, &str)], value: u64) {
    if let Some(runtime) = RUNTIME.get() {
        runtime.record_counter(desc, labels, value);
    } else {
        if let Some(registry) = REGISTRY.get() {
            let registry = registry.clone();
            let _ = registry.incr_counter(desc, labels, value);
        }
    }
}

pub(super) fn observe_histogram(
    desc: MetricDescriptor,
    labels: &[(&'static str, &str)],
    value: f64,
    sample_kind: SampleKind,
) {
    if let Some(runtime) = RUNTIME.get() {
        runtime.observe_histogram(desc, labels, value, sample_kind);
    } else {
        if let Some(registry) = REGISTRY.get() {
            let registry = registry.clone();
            let _ = registry.observe_histogram(desc, labels, value);
        }
    }
}

pub(super) fn flush_thread() {
    THREAD_BUFFER.with(|cell| {
        if let Some(buffer) = cell.borrow_mut().as_mut() {
            buffer.flush(true);
        }
    });
}

pub(super) fn configure_tls_sample_rate(rate: u32) {
    if let Some(runtime) = RUNTIME.get() {
        runtime.set_tls_sample_rate(rate);
    }
}

pub(super) fn configure_debug_mode(enabled: bool) {
    if let Some(runtime) = RUNTIME.get() {
        runtime.set_debug_mode(enabled);
    }
}

pub(super) fn configure_ip_mode(mode: ObservabilityRedactIpMode) {
    if let Some(runtime) = RUNTIME.get() {
        runtime.set_ip_mode(mode);
    }
}

pub(super) fn configure_memory_limit(bytes: u64) {
    if let Some(runtime) = RUNTIME.get() {
        runtime.set_max_memory_bytes(bytes);
    }
}

pub(super) fn force_memory_pressure_check() {
    if let Some(runtime) = RUNTIME.get() {
        runtime.check_memory_pressure();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SampleKind {
    None,
    TlsHandshake,
}

struct MetricRuntime {
    registry: Arc<MetricRegistry>,
    sender: mpsc::Sender<Vec<PendingOp>>,
    flush_interval: Duration,
    batch_capacity: usize,
    tls_sampler: Sampler,
    redactor: LabelRedactor,
    max_memory_bytes: AtomicU64,
    raw_samples_disabled: AtomicBool,
}

impl MetricRuntime {
    fn new(
        registry: Arc<MetricRegistry>,
        performance: ObservabilityPerformanceConfig,
    ) -> (Arc<Self>, mpsc::Receiver<Vec<PendingOp>>) {
        let (sender, receiver) = mpsc::channel();
        let flush_interval = Duration::from_millis(performance.batch_flush_interval_ms as u64);
        let tls_sampler = Sampler::new(performance.tls_sample_rate);
        let redactor = LabelRedactor::new(&performance);
        let runtime = Arc::new(Self {
            registry,
            sender,
            flush_interval,
            batch_capacity: 64,
            tls_sampler,
            redactor,
            max_memory_bytes: AtomicU64::new(performance.max_memory_bytes),
            raw_samples_disabled: AtomicBool::new(false),
        });
        (runtime, receiver)
    }

    fn spawn_worker(
        self: &Arc<Self>,
        receiver: mpsc::Receiver<Vec<PendingOp>>,
    ) -> Result<(), MetricInitError> {
        let weak = Arc::downgrade(self);
        thread::Builder::new()
            .name("metrics-runtime".into())
            .spawn(move || {
                while let Ok(batch) = receiver.recv() {
                    if let Some(runtime) = weak.upgrade() {
                        runtime.apply_ops(batch);
                    } else {
                        break;
                    }
                }
            })
            .map(|_| ())
            .map_err(|err| MetricInitError::Runtime(err.to_string()))
    }

    fn record_counter(&self, desc: MetricDescriptor, labels: &[(&'static str, &str)], value: u64) {
        let mut owned = Vec::with_capacity(labels.len());
        for (name, val) in labels {
            owned.push(LabelValue {
                name: *name,
                value: self.redactor.redact(*name, val),
            });
        }
        let op = PendingOp::Counter {
            descriptor: desc,
            labels: owned,
            value,
        };
        self.enqueue(op);
    }

    fn observe_histogram(
        &self,
        desc: MetricDescriptor,
        labels: &[(&'static str, &str)],
        value: f64,
        sample_kind: SampleKind,
    ) {
        if !self.should_sample(sample_kind) {
            return;
        }
        let mut owned = Vec::with_capacity(labels.len());
        for (name, val) in labels {
            owned.push(LabelValue {
                name: *name,
                value: self.redactor.redact(*name, val),
            });
        }
        let op = PendingOp::Histogram {
            descriptor: desc,
            labels: owned,
            value,
        };
        self.enqueue(op);
    }

    fn enqueue(&self, op: PendingOp) {
        THREAD_BUFFER.with(|cell| {
            let mut borrow = cell.borrow_mut();
            if borrow.is_none() {
                if let Some(runtime_arc) = RUNTIME.get() {
                    borrow.replace(ThreadBuffer::new(runtime_arc.clone()));
                }
            }
            if let Some(buffer) = borrow.as_mut() {
                buffer.push(op);
            } else {
                self.drain_now(vec![op]);
            }
        });
    }

    fn apply_ops(&self, batch: Vec<PendingOp>) {
        if batch.is_empty() {
            return;
        }
        for op in batch {
            match op {
                PendingOp::Counter {
                    descriptor,
                    labels,
                    value,
                } => {
                    let refs: Vec<(&'static str, &str)> = labels
                        .iter()
                        .map(|lv| (lv.name, lv.value.as_str()))
                        .collect();
                    if let Err(err) = self.registry.incr_counter(descriptor, &refs, value) {
                        tracing::debug!(target = "metrics", error = %err, metric = descriptor.name, "failed to record counter");
                    }
                }
                PendingOp::Histogram {
                    descriptor,
                    labels,
                    value,
                } => {
                    let refs: Vec<(&'static str, &str)> = labels
                        .iter()
                        .map(|lv| (lv.name, lv.value.as_str()))
                        .collect();
                    if let Err(err) = self.registry.observe_histogram(descriptor, &refs, value) {
                        tracing::debug!(target = "metrics", error = %err, metric = descriptor.name, "failed to record histogram");
                    }
                }
            }
        }
        self.check_memory_pressure();
    }

    fn should_sample(&self, kind: SampleKind) -> bool {
        match kind {
            SampleKind::None => true,
            SampleKind::TlsHandshake => self.tls_sampler.should_record(),
        }
    }

    fn drain_now(&self, ops: Vec<PendingOp>) {
        if ops.is_empty() {
            return;
        }
        self.apply_ops(ops);
    }

    fn send_async(&self, ops: Vec<PendingOp>) {
        if ops.is_empty() {
            return;
        }
        if let Err(err) = self.sender.send(ops) {
            tracing::warn!(target = "metrics", error = %err, "failed to enqueue metrics batch");
        }
    }

    fn check_memory_pressure(&self) {
        let limit = self.max_memory_bytes.load(Ordering::Relaxed);
        if limit == 0 || self.raw_samples_disabled.load(Ordering::Relaxed) {
            return;
        }
        let aggregator = match AGGREGATOR.get() {
            Some(agg) => agg.clone(),
            None => return,
        };
        let usage = aggregator.estimate_memory_bytes();
        if usage as u64 <= limit {
            return;
        }
        if self
            .raw_samples_disabled
            .compare_exchange(false, true, Ordering::Relaxed, Ordering::Relaxed)
            .is_err()
        {
            return;
        }
        tracing::warn!(
            target = "metrics",
            usage_bytes = usage,
            threshold_bytes = limit,
            "metrics runtime detected memory pressure; disabling raw samples"
        );
        aggregator.disable_raw_samples();
        for desc in self.registry.histogram_descriptors() {
            self.registry
                .enable_histogram_window(desc, HistogramWindowConfig::default());
        }
        let _ = self
            .registry
            .incr_counter(METRIC_MEMORY_PRESSURE_TOTAL, &[], 1);
        publish_global(Event::Strategy(StrategyEvent::MetricMemoryPressure {
            usage_bytes: usage as u64,
            threshold_bytes: limit,
            raw_samples_disabled: true,
        }));
    }

    fn set_tls_sample_rate(&self, rate: u32) {
        self.tls_sampler.set_rate(rate);
    }

    fn set_debug_mode(&self, enabled: bool) {
        self.redactor.set_debug_mode(enabled);
    }

    fn set_ip_mode(&self, mode: ObservabilityRedactIpMode) {
        self.redactor.set_ip_mode(mode);
    }

    fn set_max_memory_bytes(&self, bytes: u64) {
        self.max_memory_bytes.store(bytes, Ordering::Relaxed);
        self.raw_samples_disabled.store(false, Ordering::Relaxed);
    }
}

struct ThreadBuffer {
    runtime: Arc<MetricRuntime>,
    ops: Vec<PendingOp>,
    last_flush: Instant,
}

impl ThreadBuffer {
    fn new(runtime: Arc<MetricRuntime>) -> Self {
        Self {
            runtime,
            ops: Vec::with_capacity(64),
            last_flush: Instant::now(),
        }
    }

    fn push(&mut self, op: PendingOp) {
        self.ops.push(op);
        let now = Instant::now();
        if self.ops.len() >= self.runtime.batch_capacity
            || now.duration_since(self.last_flush) >= self.runtime.flush_interval
        {
            self.flush(false);
        }
    }

    fn flush(&mut self, force_sync: bool) {
        if self.ops.is_empty() {
            return;
        }
        self.last_flush = Instant::now();
        let batch = std::mem::take(&mut self.ops);
        if force_sync {
            self.runtime.drain_now(batch);
        } else {
            self.runtime.send_async(batch);
        }
    }
}

impl Drop for ThreadBuffer {
    fn drop(&mut self) {
        self.flush(true);
    }
}

thread_local! {
    static THREAD_BUFFER: RefCell<Option<ThreadBuffer>> = const { RefCell::new(None) };
}

#[derive(Debug)]
struct LabelValue {
    name: &'static str,
    value: String,
}

#[derive(Debug)]
enum PendingOp {
    Counter {
        descriptor: MetricDescriptor,
        labels: Vec<LabelValue>,
        value: u64,
    },
    Histogram {
        descriptor: MetricDescriptor,
        labels: Vec<LabelValue>,
        value: f64,
    },
}

#[derive(Debug)]
struct Sampler {
    rate: AtomicU32,
    counter: AtomicU64,
}

impl Sampler {
    fn new(rate: u32) -> Self {
        Self {
            rate: AtomicU32::new(rate.max(1)),
            counter: AtomicU64::new(0),
        }
    }

    fn set_rate(&self, rate: u32) {
        self.rate.store(rate.max(1), Ordering::Relaxed);
        self.counter.store(0, Ordering::Relaxed);
    }

    fn should_record(&self) -> bool {
        let rate = self.rate.load(Ordering::Relaxed).max(1);
        if rate == 1 {
            return true;
        }
        let prev = self.counter.fetch_add(1, Ordering::Relaxed);
        prev % rate as u64 == 0
    }
}

#[derive(Debug)]
struct LabelRedactor {
    repo_salt: Vec<u8>,
    ip_mode: AtomicU8,
    debug_mode: AtomicBool,
}

impl LabelRedactor {
    fn new(performance: &ObservabilityPerformanceConfig) -> Self {
        let mut salt_bytes = if performance.redact.repo_hash_salt.is_empty() {
            let mut buf = vec![0u8; 16];
            OsRng.fill_bytes(&mut buf);
            buf
        } else {
            performance.redact.repo_hash_salt.as_bytes().to_vec()
        };
        if salt_bytes.is_empty() {
            salt_bytes.extend_from_slice(b"fwc-metrics");
        }
        Self {
            repo_salt: salt_bytes,
            ip_mode: AtomicU8::new(Self::encode_ip_mode(performance.redact.ip_mode)),
            debug_mode: AtomicBool::new(performance.debug_mode),
        }
    }

    fn redact(&self, label: &'static str, value: &str) -> String {
        if self.debug_mode.load(Ordering::Relaxed) {
            return value.to_string();
        }
        if label.contains("repo") {
            return self.redact_repo(value);
        }
        if label.contains("ip") {
            return self.redact_ip(value);
        }
        value.to_string()
    }

    fn redact_repo(&self, value: &str) -> String {
        if value.is_empty() {
            return "unknown".into();
        }
        let mut hasher = Sha256::new();
        hasher.update(&self.repo_salt);
        hasher.update(value.as_bytes());
        let digest = hasher.finalize();
        let mut out = String::with_capacity(8);
        for byte in digest.iter().take(4) {
            use std::fmt::Write;
            let _ = write!(&mut out, "{:02x}", byte);
        }
        out
    }

    fn redact_ip(&self, value: &str) -> String {
        let Ok(ip) = value.parse::<IpAddr>() else {
            return value.to_string();
        };
        match self.decode_ip_mode() {
            ObservabilityRedactIpMode::Mask => mask_ip(ip),
            ObservabilityRedactIpMode::Classify => classify_ip(ip).to_string(),
            ObservabilityRedactIpMode::Full => value.to_string(),
        }
    }

    fn set_debug_mode(&self, enabled: bool) {
        self.debug_mode.store(enabled, Ordering::Relaxed);
    }

    fn set_ip_mode(&self, mode: ObservabilityRedactIpMode) {
        self.ip_mode
            .store(Self::encode_ip_mode(mode), Ordering::Relaxed);
    }

    fn decode_ip_mode(&self) -> ObservabilityRedactIpMode {
        Self::decode_ip_mode_from(self.ip_mode.load(Ordering::Relaxed))
    }

    fn encode_ip_mode(mode: ObservabilityRedactIpMode) -> u8 {
        match mode {
            ObservabilityRedactIpMode::Mask => 0,
            ObservabilityRedactIpMode::Classify => 1,
            ObservabilityRedactIpMode::Full => 2,
        }
    }

    fn decode_ip_mode_from(raw: u8) -> ObservabilityRedactIpMode {
        match raw {
            0 => ObservabilityRedactIpMode::Mask,
            1 => ObservabilityRedactIpMode::Classify,
            2 => ObservabilityRedactIpMode::Full,
            _ => ObservabilityRedactIpMode::Mask,
        }
    }
}

fn mask_ip(ip: IpAddr) -> String {
    match ip {
        IpAddr::V4(addr) => {
            let octets = addr.octets();
            format!("{}.{}.*.*", octets[0], octets[1])
        }
        IpAddr::V6(addr) => {
            let segments = addr.segments();
            format!("{:x}:{:x}::", segments[0], segments[1])
        }
    }
}

fn classify_ip(ip: IpAddr) -> &'static str {
    match ip {
        IpAddr::V4(addr) => classify_ipv4(addr),
        IpAddr::V6(addr) => classify_ipv6(addr),
    }
}

fn classify_ipv4(addr: Ipv4Addr) -> &'static str {
    if addr.is_loopback() {
        "loopback"
    } else if addr.is_multicast() {
        "multicast"
    } else if addr.is_unspecified() {
        "unspecified"
    } else if addr.is_private() {
        "private"
    } else {
        "public"
    }
}

fn classify_ipv6(addr: Ipv6Addr) -> &'static str {
    if addr.is_loopback() {
        "loopback"
    } else if addr.is_multicast() {
        "multicast"
    } else if addr.is_unspecified() {
        "unspecified"
    } else if addr.is_unique_local() {
        "private"
    } else {
        "public"
    }
}
