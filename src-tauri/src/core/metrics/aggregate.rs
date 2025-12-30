use dashmap::{mapref::entry::Entry, DashMap, DashSet};
use hdrhistogram::Histogram;
use std::collections::VecDeque;
use std::hash::{Hash, Hasher};
use std::mem;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::registry::{LabelKey, MetricDescriptor};

const MINUTE_SLOTS: usize = 60;
const HOUR_SLOTS: usize = 24;
const HISTOGRAM_MAX_MS: u64 = 3_600_000; // one hour ceiling in milliseconds
const HISTOGRAM_SIG_FIGS: u8 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowRange {
    LastMinute,
    LastFiveMinutes,
    LastHour,
    LastDay,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowResolution {
    Minute,
    Hour,
}

impl WindowRange {
    pub fn resolution(self) -> WindowResolution {
        match self {
            WindowRange::LastMinute => WindowResolution::Minute,
            WindowRange::LastFiveMinutes => WindowResolution::Minute,
            WindowRange::LastHour => WindowResolution::Minute,
            WindowRange::LastDay => WindowResolution::Hour,
        }
    }

    pub fn slots(self) -> usize {
        match self {
            WindowRange::LastMinute => 1,
            WindowRange::LastFiveMinutes => 5,
            WindowRange::LastHour => MINUTE_SLOTS,
            WindowRange::LastDay => HOUR_SLOTS,
        }
    }
}

impl WindowResolution {
    fn seconds(self) -> u64 {
        match self {
            WindowResolution::Minute => 60,
            WindowResolution::Hour => 3_600,
        }
    }
}

pub trait TimeProvider: Send + Sync {
    fn now(&self) -> Instant;
}

#[derive(Default)]
pub struct SystemTimeProvider;

impl TimeProvider for SystemTimeProvider {
    fn now(&self) -> Instant {
        Instant::now()
    }
}

pub struct ManualTimeProvider {
    base: Instant,
    offset_ms: AtomicU64,
}

impl ManualTimeProvider {
    pub fn new() -> Self {
        Self {
            base: Instant::now(),
            offset_ms: AtomicU64::new(0),
        }
    }

    pub fn advance(&self, duration: Duration) {
        let delta = millis_to_u64(duration);
        self.offset_ms.fetch_add(delta, Ordering::Relaxed);
    }

    pub fn reset(&self) {
        self.offset_ms.store(0, Ordering::Relaxed);
    }

    pub fn set(&self, duration: Duration) {
        let value = millis_to_u64(duration);
        self.offset_ms.store(value, Ordering::Relaxed);
    }
}

impl Default for ManualTimeProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl TimeProvider for ManualTimeProvider {
    fn now(&self) -> Instant {
        let millis = self.offset_ms.load(Ordering::Relaxed);
        self.base
            .checked_add(Duration::from_millis(millis))
            .unwrap_or(self.base)
    }
}

fn millis_to_u64(duration: Duration) -> u64 {
    let millis = duration.as_millis();
    if millis > u128::from(u64::MAX) {
        u64::MAX
    } else {
        millis as u64
    }
}

#[derive(Debug, Clone)]
pub struct WindowPoint {
    pub offset_seconds: u64,
    pub value: u64,
}

#[derive(Debug, Clone)]
pub struct CounterWindowSnapshot {
    pub descriptor: MetricDescriptor,
    pub labels: Vec<String>,
    pub range: WindowRange,
    pub resolution: WindowResolution,
    pub points: Vec<WindowPoint>,
    pub total: u64,
}

#[derive(Debug, Clone)]
pub struct HistogramWindowPoint {
    pub offset_seconds: u64,
    pub count: u64,
    pub sum: f64,
}

#[derive(Debug, Clone)]
pub struct HistogramWindowSnapshot {
    pub descriptor: MetricDescriptor,
    pub labels: Vec<String>,
    pub range: WindowRange,
    pub resolution: WindowResolution,
    pub sum: f64,
    pub count: u64,
    pub buckets: Vec<(f64, u64)>,
    pub quantiles: Vec<(f64, f64)>,
    pub points: Vec<HistogramWindowPoint>,
    pub raw_samples: Vec<HistogramRawSample>,
}

#[derive(Debug, Clone, Default)]
pub struct HistogramWindowConfig {
    raw_samples: Option<RawSampleConfig>,
}

impl HistogramWindowConfig {
    pub fn enable_raw_samples(window: Duration, max_samples: usize) -> Self {
        if window.is_zero() || max_samples == 0 {
            return Self::default();
        }
        let window_minutes = ((window.as_secs() + 59) / 60).max(1);
        Self {
            raw_samples: Some(RawSampleConfig {
                window_minutes,
                max_samples,
            }),
        }
    }
}

#[derive(Debug, Clone)]
struct RawSampleConfig {
    window_minutes: u64,
    max_samples: usize,
}

#[derive(Debug, Clone)]
struct HistogramWindowOptions {
    raw_window_minutes: Option<u64>,
    raw_max_samples: usize,
}

impl HistogramWindowOptions {
    fn from_config(config: HistogramWindowConfig) -> Self {
        match config.raw_samples {
            Some(raw) => Self {
                raw_window_minutes: Some(raw.window_minutes),
                raw_max_samples: raw.max_samples,
            },
            None => Self {
                raw_window_minutes: None,
                raw_max_samples: 0,
            },
        }
    }
}

#[derive(Debug, Clone)]
pub struct HistogramRawSample {
    pub offset_seconds: u64,
    pub value: f64,
}

#[derive(Debug, Clone)]
pub struct WindowSeriesDescriptor {
    pub labels: Vec<String>,
    pub last_updated_seconds: Option<u64>,
}

pub struct WindowAggregator {
    provider: Arc<dyn TimeProvider>,
    start: Instant,
    counters: DashMap<AggregateKey, Arc<Mutex<CounterEntry>>>,
    histograms: DashMap<AggregateKey, Arc<Mutex<HistogramEntry>>>,
    enabled_counters: DashSet<&'static str>,
    enabled_histograms: DashMap<&'static str, HistogramWindowOptions>,
}

impl WindowAggregator {
    pub(super) fn new(provider: Arc<dyn TimeProvider>) -> Self {
        let start = provider.now();
        Self {
            provider,
            start,
            counters: DashMap::new(),
            histograms: DashMap::new(),
            enabled_counters: DashSet::new(),
            enabled_histograms: DashMap::new(),
        }
    }

    pub(super) fn record_counter(&self, desc: MetricDescriptor, key: LabelKey, value: u64) {
        if !self.enabled_counters.contains(&desc.name) {
            self.enabled_counters.insert(desc.name);
        }
        let slices = self.current_slices();
        let agg_key = AggregateKey::new(desc.name, key);
        let series = match self.counters.entry(agg_key) {
            Entry::Occupied(entry) => Arc::clone(entry.get()),
            Entry::Vacant(vacant) => {
                let label_values = vacant.key().label_values().to_vec();
                let arc = Arc::new(Mutex::new(CounterEntry::new(desc, label_values)));
                vacant.insert(arc.clone());
                arc
            }
        };
        if let Ok(mut guard) = series.lock() {
            guard.record(slices.minute, value);
        };
    }

    pub(super) fn record_histogram(&self, desc: MetricDescriptor, key: LabelKey, value: f64) {
        let options = self
            .enabled_histograms
            .get(desc.name)
            .map(|opts| opts.clone())
            .unwrap_or_else(|| {
                HistogramWindowOptions::from_config(HistogramWindowConfig::default())
            });
        let boundaries = match desc.buckets {
            Some(b) => b,
            None => return,
        };
        let slices = self.current_slices();
        let agg_key = AggregateKey::new(desc.name, key);
        let series = match self.histograms.entry(agg_key) {
            Entry::Occupied(entry) => Arc::clone(entry.get()),
            Entry::Vacant(vacant) => {
                let label_values = vacant.key().label_values().to_vec();
                let arc = Arc::new(Mutex::new(HistogramEntry::new(
                    desc,
                    boundaries,
                    label_values,
                    options.clone(),
                )));
                vacant.insert(arc.clone());
                arc
            }
        };
        if let Ok(mut guard) = series.lock() {
            guard.update_options(options.clone());
            guard.record(slices.minute, value);
        };
    }

    pub(super) fn snapshot_counter(
        &self,
        desc: MetricDescriptor,
        key: LabelKey,
        range: WindowRange,
    ) -> Option<CounterWindowSnapshot> {
        let agg_key = AggregateKey::new(desc.name, key);
        let series = self
            .counters
            .get(&agg_key)
            .map(|entry| Arc::clone(entry.value()))?;
        let slices = self.current_slices();
        let guard = series.lock().ok()?;
        Some(guard.snapshot(range, slices.minute))
    }

    pub(super) fn snapshot_histogram(
        &self,
        desc: MetricDescriptor,
        key: LabelKey,
        range: WindowRange,
        quantiles: &[f64],
    ) -> Option<HistogramWindowSnapshot> {
        let agg_key = AggregateKey::new(desc.name, key);
        let series = self
            .histograms
            .get(&agg_key)
            .map(|entry| Arc::clone(entry.value()))?;
        let slices = self.current_slices();
        let guard = series.lock().ok()?;
        Some(guard.snapshot(range, slices.minute, quantiles))
    }

    pub(super) fn enable_counter_metric(&self, name: &'static str) {
        self.enabled_counters.insert(name);
    }

    pub(super) fn enable_histogram_metric(
        &self,
        name: &'static str,
        config: HistogramWindowConfig,
    ) {
        let options = HistogramWindowOptions::from_config(config);
        self.enabled_histograms.insert(name, options.clone());
        let entries: Vec<Arc<Mutex<HistogramEntry>>> = self
            .histograms
            .iter()
            .filter(|entry| entry.key().metric == name)
            .map(|entry| Arc::clone(entry.value()))
            .collect();
        for arc in entries {
            if let Ok(mut guard) = arc.lock() {
                guard.update_options(options.clone());
            }
        }
    }

    pub(super) fn estimate_memory_bytes(&self) -> u64 {
        let mut total = 0u64;
        for entry in self.histograms.iter() {
            if let Ok(guard) = entry.value().lock() {
                total = total.saturating_add(guard.memory_usage_bytes() as u64);
            }
        }
        total
    }

    pub(super) fn disable_raw_samples(&self) {
        let entries: Vec<Arc<Mutex<HistogramEntry>>> = self
            .histograms
            .iter()
            .map(|entry| Arc::clone(entry.value()))
            .collect();
        for arc in entries {
            if let Ok(mut guard) = arc.lock() {
                guard.disable_raw_samples();
            }
        }
    }

    pub(super) fn list_counter_series(&self, metric: &'static str) -> Vec<WindowSeriesDescriptor> {
        self.counters
            .iter()
            .filter(|entry| entry.key().metric == metric)
            .map(|entry| {
                let labels = entry.key().labels.values().to_vec();
                let arc = Arc::clone(entry.value());
                (labels, arc)
            })
            .map(|(labels, arc)| {
                let last_updated = arc
                    .lock()
                    .ok()
                    .and_then(|guard| guard.last_updated_seconds());
                WindowSeriesDescriptor {
                    labels,
                    last_updated_seconds: last_updated,
                }
            })
            .collect()
    }

    pub(super) fn list_histogram_series(
        &self,
        metric: &'static str,
    ) -> Vec<WindowSeriesDescriptor> {
        self.histograms
            .iter()
            .filter(|entry| entry.key().metric == metric)
            .map(|entry| {
                let labels = entry.key().labels.values().to_vec();
                let arc = Arc::clone(entry.value());
                (labels, arc)
            })
            .map(|(labels, arc)| {
                let last_updated = arc
                    .lock()
                    .ok()
                    .and_then(|guard| guard.last_updated_seconds());
                WindowSeriesDescriptor {
                    labels,
                    last_updated_seconds: last_updated,
                }
            })
            .collect()
    }

    fn current_slices(&self) -> TimeSlices {
        let now = self.provider.now();
        let elapsed = now
            .checked_duration_since(self.start)
            .unwrap_or_else(|| Duration::from_secs(0));
        let seconds = elapsed.as_secs();
        let minute = seconds / 60;
        let hour = minute / 60;
        TimeSlices { minute, hour }
    }
}

struct TimeSlices {
    minute: u64,
    hour: u64,
}

#[derive(Clone)]
struct AggregateKey {
    metric: &'static str,
    labels: LabelKey,
}

impl AggregateKey {
    fn new(metric: &'static str, labels: LabelKey) -> Self {
        Self { metric, labels }
    }

    fn label_values(&self) -> &[String] {
        self.labels.values()
    }
}

impl PartialEq for AggregateKey {
    fn eq(&self, other: &Self) -> bool {
        self.metric == other.metric && self.labels == other.labels
    }
}

impl Eq for AggregateKey {}

impl Hash for AggregateKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.metric.hash(state);
        self.labels.hash(state);
    }
}

struct CounterEntry {
    descriptor: MetricDescriptor,
    label_values: Vec<String>,
    minutes: Vec<CounterSlot>,
    hours: Vec<CounterSlot>,
    last_minute: Option<u64>,
}

impl CounterEntry {
    fn new(descriptor: MetricDescriptor, label_values: Vec<String>) -> Self {
        Self {
            descriptor,
            label_values,
            minutes: (0..MINUTE_SLOTS).map(|_| CounterSlot::new()).collect(),
            hours: (0..HOUR_SLOTS).map(|_| CounterSlot::new()).collect(),
            last_minute: None,
        }
    }

    fn record(&mut self, minute: u64, value: u64) {
        let minute_idx = (minute % MINUTE_SLOTS as u64) as usize;
        let slot = &mut self.minutes[minute_idx];
        slot.ensure(minute);
        slot.value = slot.value.saturating_add(value);

        let hour = minute / 60;
        let hour_idx = (hour % HOUR_SLOTS as u64) as usize;
        let hslot = &mut self.hours[hour_idx];
        hslot.ensure(hour);
        hslot.value = hslot.value.saturating_add(value);
        self.last_minute = Some(minute);
    }

    fn snapshot(&self, range: WindowRange, end_minute: u64) -> CounterWindowSnapshot {
        let resolution = range.resolution();
        let slots = range.slots();
        let mut points = Vec::with_capacity(slots);
        match resolution {
            WindowResolution::Minute => {
                for idx in 0..slots {
                    let offset = (slots - 1 - idx) as u64;
                    let in_range = offset <= end_minute;
                    let minute = end_minute.saturating_sub(offset);
                    let series_idx = (minute % MINUTE_SLOTS as u64) as usize;
                    let slot = &self.minutes[series_idx];
                    let value = if in_range && slot.stamp == minute {
                        slot.value
                    } else {
                        0
                    };
                    points.push(WindowPoint {
                        offset_seconds: minute.saturating_mul(resolution.seconds()),
                        value,
                    });
                }
            }
            WindowResolution::Hour => {
                let end_hour = end_minute / 60;
                for idx in 0..slots {
                    let offset = (slots - 1 - idx) as u64;
                    let in_range = offset <= end_hour;
                    let hour = end_hour.saturating_sub(offset);
                    let series_idx = (hour % HOUR_SLOTS as u64) as usize;
                    let slot = &self.hours[series_idx];
                    let value = if in_range && slot.stamp == hour {
                        slot.value
                    } else {
                        0
                    };
                    points.push(WindowPoint {
                        offset_seconds: hour.saturating_mul(resolution.seconds()),
                        value,
                    });
                }
            }
        }
        let total = points.iter().map(|p| p.value).sum();
        CounterWindowSnapshot {
            descriptor: self.descriptor,
            labels: self.label_values.clone(),
            range,
            resolution,
            points,
            total,
        }
    }

    fn last_updated_seconds(&self) -> Option<u64> {
        self.last_minute.map(|minute| minute.saturating_mul(60))
    }
}

struct CounterSlot {
    stamp: u64,
    value: u64,
}

impl CounterSlot {
    fn new() -> Self {
        Self {
            stamp: u64::MAX,
            value: 0,
        }
    }

    fn ensure(&mut self, stamp: u64) {
        if self.stamp != stamp {
            self.stamp = stamp;
            self.value = 0;
        }
    }
}

struct HistogramEntry {
    descriptor: MetricDescriptor,
    label_values: Vec<String>,
    boundaries: Vec<f64>,
    bucket_count: usize,
    minutes: Vec<HistogramSlot>,
    hours: Vec<HistogramSlot>,
    options: HistogramWindowOptions,
    raw_samples: VecDeque<RawSamplePoint>,
    last_minute: Option<u64>,
}

#[derive(Debug, Clone)]
struct RawSamplePoint {
    minute: u64,
    value: f64,
}

impl HistogramEntry {
    fn new(
        descriptor: MetricDescriptor,
        boundaries: &[f64],
        label_values: Vec<String>,
        options: HistogramWindowOptions,
    ) -> Self {
        let bucket_count = boundaries.len() + 1;
        Self {
            descriptor,
            label_values,
            boundaries: boundaries.to_vec(),
            bucket_count,
            minutes: (0..MINUTE_SLOTS)
                .map(|_| HistogramSlot::new(bucket_count))
                .collect(),
            hours: (0..HOUR_SLOTS)
                .map(|_| HistogramSlot::new(bucket_count))
                .collect(),
            options,
            raw_samples: VecDeque::new(),
            last_minute: None,
        }
    }

    fn record(&mut self, minute: u64, value: f64) {
        let bucket_idx = self.bucket_index(value);
        let minute_idx = (minute % MINUTE_SLOTS as u64) as usize;
        let slot = &mut self.minutes[minute_idx];
        slot.observe(minute, bucket_idx, value);

        let hour = minute / 60;
        let hour_idx = (hour % HOUR_SLOTS as u64) as usize;
        let hslot = &mut self.hours[hour_idx];
        hslot.observe(hour, bucket_idx, value);
        self.record_raw_sample(minute, value);
        self.last_minute = Some(minute);
    }

    fn disable_raw_samples(&mut self) {
        self.options.raw_window_minutes = None;
        self.options.raw_max_samples = 0;
        self.raw_samples.clear();
    }

    fn snapshot(
        &self,
        range: WindowRange,
        end_minute: u64,
        quantiles: &[f64],
    ) -> HistogramWindowSnapshot {
        let resolution = range.resolution();
        let slots = range.slots();
        let mut points = Vec::with_capacity(slots);
        let mut total_sum = 0.0;
        let mut total_count = 0u64;
        let mut bucket_totals = vec![0u64; self.bucket_count];
        let mut combined_hist = Histogram::new_with_bounds(1, HISTOGRAM_MAX_MS, HISTOGRAM_SIG_FIGS)
            .expect("histogram init");

        match resolution {
            WindowResolution::Minute => {
                for idx in 0..slots {
                    let offset = (slots - 1 - idx) as u64;
                    let minute = end_minute.saturating_sub(offset);
                    let series_idx = (minute % MINUTE_SLOTS as u64) as usize;
                    let slot = &self.minutes[series_idx];
                    let in_range = offset <= end_minute;
                    if in_range && slot.stamp == minute {
                        total_sum += slot.sum;
                        total_count += slot.count;
                        add_buckets(&mut bucket_totals, &slot.buckets);
                        let _ = combined_hist.add(&slot.hist);
                        points.push(HistogramWindowPoint {
                            offset_seconds: minute.saturating_mul(resolution.seconds()),
                            count: slot.count,
                            sum: slot.sum,
                        });
                    } else {
                        points.push(HistogramWindowPoint {
                            offset_seconds: minute.saturating_mul(resolution.seconds()),
                            count: 0,
                            sum: 0.0,
                        });
                    }
                }
            }
            WindowResolution::Hour => {
                let end_hour = end_minute / 60;
                for idx in 0..slots {
                    let offset = (slots - 1 - idx) as u64;
                    let hour = end_hour.saturating_sub(offset);
                    let series_idx = (hour % HOUR_SLOTS as u64) as usize;
                    let slot = &self.hours[series_idx];
                    let in_range = offset <= end_hour;
                    if in_range && slot.stamp == hour {
                        total_sum += slot.sum;
                        total_count += slot.count;
                        add_buckets(&mut bucket_totals, &slot.buckets);
                        let _ = combined_hist.add(&slot.hist);
                        points.push(HistogramWindowPoint {
                            offset_seconds: hour.saturating_mul(resolution.seconds()),
                            count: slot.count,
                            sum: slot.sum,
                        });
                    } else {
                        points.push(HistogramWindowPoint {
                            offset_seconds: hour.saturating_mul(resolution.seconds()),
                            count: 0,
                            sum: 0.0,
                        });
                    }
                }
            }
        }

        let mut buckets = Vec::with_capacity(self.bucket_count);
        for idx in 0..self.bucket_count {
            let boundary = if idx < self.boundaries.len() {
                self.boundaries[idx]
            } else {
                f64::INFINITY
            };
            buckets.push((boundary, bucket_totals[idx]));
        }

        let mut quantile_values = Vec::with_capacity(quantiles.len());
        if total_count > 0 {
            for &q in quantiles {
                if q.is_nan() {
                    continue;
                }
                let value = combined_hist.value_at_quantile(q.clamp(0.0, 1.0));
                quantile_values.push((q, value as f64));
            }
        }

        HistogramWindowSnapshot {
            descriptor: self.descriptor,
            labels: self.label_values.clone(),
            range,
            resolution,
            sum: total_sum,
            count: total_count,
            buckets,
            quantiles: quantile_values,
            points,
            raw_samples: self.collect_raw_samples(range, end_minute),
        }
    }

    fn bucket_index(&self, value: f64) -> usize {
        let value = value.max(0.0);
        for (idx, boundary) in self.boundaries.iter().enumerate() {
            if value <= *boundary {
                return idx;
            }
        }
        self.bucket_count.saturating_sub(1)
    }

    fn update_options(&mut self, options: HistogramWindowOptions) {
        self.options = options;
        self.trim_raw_samples(self.last_minute.unwrap_or(0));
        if self.options.raw_window_minutes.is_none() || self.options.raw_max_samples == 0 {
            self.raw_samples.clear();
        }
    }

    fn record_raw_sample(&mut self, minute: u64, value: f64) {
        if self.options.raw_window_minutes.is_none() || self.options.raw_max_samples == 0 {
            return;
        }
        self.raw_samples.push_back(RawSamplePoint { minute, value });
        self.trim_raw_samples(minute);
        while self.raw_samples.len() > self.options.raw_max_samples {
            self.raw_samples.pop_front();
        }
    }

    fn trim_raw_samples(&mut self, current_minute: u64) {
        if let Some(window_minutes) = self.options.raw_window_minutes {
            while let Some(front) = self.raw_samples.front() {
                if front.minute + window_minutes <= current_minute {
                    self.raw_samples.pop_front();
                } else {
                    break;
                }
            }
        }
    }

    fn collect_raw_samples(&self, range: WindowRange, end_minute: u64) -> Vec<HistogramRawSample> {
        if self.options.raw_window_minutes.is_none() || self.options.raw_max_samples == 0 {
            return Vec::new();
        }
        let span_minutes = match range.resolution() {
            WindowResolution::Minute => range.slots() as u64,
            WindowResolution::Hour => (range.slots() as u64).saturating_mul(60),
        };
        let start_minute = end_minute.saturating_sub(span_minutes.saturating_sub(1));
        self.raw_samples
            .iter()
            .filter(|sample| sample.minute >= start_minute && sample.minute <= end_minute)
            .map(|sample| HistogramRawSample {
                offset_seconds: sample.minute.saturating_mul(60),
                value: sample.value,
            })
            .collect()
    }

    fn last_updated_seconds(&self) -> Option<u64> {
        self.last_minute.map(|minute| minute.saturating_mul(60))
    }

    fn memory_usage_bytes(&self) -> usize {
        self.raw_samples.len() * mem::size_of::<RawSamplePoint>()
    }
}

fn add_buckets(target: &mut [u64], source: &[u64]) {
    for (dst, src) in target.iter_mut().zip(source.iter()) {
        *dst = dst.saturating_add(*src);
    }
}

struct HistogramSlot {
    stamp: u64,
    buckets: Vec<u64>,
    sum: f64,
    count: u64,
    hist: Histogram<u64>,
}

impl HistogramSlot {
    fn new(bucket_count: usize) -> Self {
        Self {
            stamp: u64::MAX,
            buckets: vec![0; bucket_count],
            sum: 0.0,
            count: 0,
            hist: Histogram::new_with_bounds(1, HISTOGRAM_MAX_MS, HISTOGRAM_SIG_FIGS)
                .expect("histogram slot init"),
        }
    }

    fn observe(&mut self, stamp: u64, bucket_idx: usize, value: f64) {
        if self.stamp != stamp {
            self.reset(stamp);
        }
        if let Some(bucket) = self.buckets.get_mut(bucket_idx) {
            *bucket = bucket.saturating_add(1);
        }
        self.count = self.count.saturating_add(1);
        self.sum += value;
        let mut record_value = value.max(0.0).round() as u64;
        if record_value == 0 {
            record_value = 1;
        }
        if record_value > HISTOGRAM_MAX_MS {
            record_value = HISTOGRAM_MAX_MS;
        }
        let _ = self.hist.record(record_value);
    }

    fn reset(&mut self, stamp: u64) {
        self.stamp = stamp;
        for bucket in &mut self.buckets {
            *bucket = 0;
        }
        self.sum = 0.0;
        self.count = 0;
        self.hist.reset();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_window_aggregator_memory_estimation() {
        let provider = Arc::new(ManualTimeProvider::new());
        let agg = WindowAggregator::new(provider);
        let desc = MetricDescriptor::histogram("test_hist", "test", &["l1"], &[0.1, 1.0]);
        let labels = LabelKey::new(vec!["v1".into()]);

        let config = HistogramWindowConfig::enable_raw_samples(Duration::from_secs(60), 10);
        agg.enable_histogram_metric("test_hist", config);

        // Record a few samples
        for i in 0..5 {
            agg.record_histogram(desc, labels.clone(), i as f64);
        }

        let mem = agg.estimate_memory_bytes();
        // 5 samples * sizeof(RawSamplePoint)
        assert_eq!(mem, 5 * mem::size_of::<RawSamplePoint>() as u64);

        agg.disable_raw_samples();
        assert_eq!(agg.estimate_memory_bytes(), 0);
    }

    #[test]
    fn test_counter_entry_rollover() {
        let desc = MetricDescriptor::counter("test_cnt", "test", &[]);
        let mut entry = CounterEntry::new(desc, vec![]);

        // Record at minute 0
        entry.record(0, 10);
        assert_eq!(entry.minutes[0].value, 10);
        assert_eq!(entry.hours[0].value, 10);

        // Record at minute 60 (same minute slot index 0, same hour slot index 1)
        entry.record(60, 20);
        assert_eq!(entry.minutes[0].value, 20); // Reset at minute 0 index
        assert_eq!(entry.hours[1].value, 20);
        assert_eq!(entry.hours[0].value, 10);

        // Snapshot last minute
        let snap = entry.snapshot(WindowRange::LastMinute, 60);
        assert_eq!(snap.total, 20);

        // Snapshot last 5 min (including minute 60)
        let snap = entry.snapshot(WindowRange::LastFiveMinutes, 60);
        assert_eq!(snap.total, 20); // Only minute 60 has value
    }

    #[test]
    fn test_histogram_entry_buckets() {
        let desc = MetricDescriptor::histogram("test_hist", "test", &[], &[10.0, 100.0]);
        let mut entry = HistogramEntry::new(
            desc,
            &[10.0, 100.0],
            vec![],
            HistogramWindowOptions::from_config(HistogramWindowConfig::default()),
        );

        entry.record(0, 5.0); // Bucket 0
        entry.record(0, 50.0); // Bucket 1
        entry.record(0, 150.0); // Bucket 2

        let snap = entry.snapshot(WindowRange::LastMinute, 0, &[0.5]);
        assert_eq!(snap.count, 3);
        assert_eq!(snap.buckets[0].1, 1); // <= 10
        assert_eq!(snap.buckets[1].1, 1); // <= 100
        assert_eq!(snap.buckets[2].1, 1); // > 100
    }
}
