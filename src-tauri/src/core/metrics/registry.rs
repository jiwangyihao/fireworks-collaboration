use dashmap::DashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use super::error::MetricError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetricKind {
    Counter,
    Gauge,
    Histogram,
}

#[derive(Debug, Clone, Copy)]
pub struct MetricDescriptor {
    pub name: &'static str,
    pub help: &'static str,
    pub kind: MetricKind,
    pub labels: &'static [&'static str],
    pub buckets: Option<&'static [f64]>,
}

impl MetricDescriptor {
    pub const fn counter(
        name: &'static str,
        help: &'static str,
        labels: &'static [&'static str],
    ) -> Self {
        Self {
            name,
            help,
            kind: MetricKind::Counter,
            labels,
            buckets: None,
        }
    }

    pub const fn histogram(
        name: &'static str,
        help: &'static str,
        labels: &'static [&'static str],
        buckets: &'static [f64],
    ) -> Self {
        Self {
            name,
            help,
            kind: MetricKind::Histogram,
            labels,
            buckets: Some(buckets),
        }
    }
}

#[derive(Debug, Clone)]
pub struct HistogramSnapshot {
    pub sum: f64,
    pub count: u64,
    pub buckets: Vec<(f64, u64)>,
}

#[derive(Default)]
pub struct MetricRegistry {
    counters: DashMap<&'static str, CounterMetric>,
    histograms: DashMap<&'static str, HistogramMetric>,
}

impl MetricRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&self, desc: MetricDescriptor) -> Result<(), MetricError> {
        match desc.kind {
            MetricKind::Counter => {
                if self.counters.contains_key(desc.name) {
                    return Err(MetricError::AlreadyRegistered(desc.name));
                }
                self.counters.insert(
                    desc.name,
                    CounterMetric {
                        _desc: desc,
                        series: DashMap::new(),
                    },
                );
                Ok(())
            }
            MetricKind::Histogram => {
                if self.histograms.contains_key(desc.name) {
                    return Err(MetricError::AlreadyRegistered(desc.name));
                }
                let bucket_count = desc
                    .buckets
                    .ok_or(MetricError::MissingBuckets(desc.name))?
                    .len()
                    + 1; // include +Inf bucket
                if bucket_count <= 1 {
                    return Err(MetricError::MissingBuckets(desc.name));
                }
                self.histograms.insert(
                    desc.name,
                    HistogramMetric {
                        _desc: desc,
                        series: DashMap::new(),
                        bucket_count,
                    },
                );
                Ok(())
            }
            MetricKind::Gauge => Err(MetricError::UnsupportedKind(desc.name)),
        }
    }

    pub fn incr_counter(
        &self,
        desc: MetricDescriptor,
        labels: &[(&'static str, &str)],
        value: u64,
    ) -> Result<(), MetricError> {
        let metric = self
            .counters
            .get(desc.name)
            .ok_or(MetricError::NotRegistered(desc.name))?;
        let key = normalize_labels(desc, labels)?;
        let entry = metric
            .series
            .entry(key)
            .or_insert_with(|| Arc::new(AtomicU64::new(0)));
        entry.fetch_add(value, Ordering::Relaxed);
        Ok(())
    }

    pub fn observe_histogram(
        &self,
        desc: MetricDescriptor,
        labels: &[(&'static str, &str)],
        value: f64,
    ) -> Result<(), MetricError> {
        let metric = self
            .histograms
            .get(desc.name)
            .ok_or(MetricError::NotRegistered(desc.name))?;
        let boundaries = desc.buckets.ok_or(MetricError::MissingBuckets(desc.name))?;
        let index = locate_bucket(boundaries, value);
        let key = normalize_labels(desc, labels)?;
        let series = metric
            .series
            .entry(key)
            .or_insert_with(|| Arc::new(HistogramSeries::new(metric.bucket_count)));
        series.observe(index, value);
        Ok(())
    }

    pub fn get_counter(
        &self,
        desc: MetricDescriptor,
        labels: &[(&'static str, &str)],
    ) -> Option<u64> {
        let metric = self.counters.get(desc.name)?;
        let key = normalize_labels(desc, labels).ok()?;
        metric
            .series
            .get(&key)
            .map(|entry| entry.load(Ordering::Relaxed))
    }

    pub fn get_histogram(
        &self,
        desc: MetricDescriptor,
        labels: &[(&'static str, &str)],
    ) -> Option<HistogramSnapshot> {
        let metric = self.histograms.get(desc.name)?;
        let boundaries = desc.buckets?;
        let key = normalize_labels(desc, labels).ok()?;
        metric
            .series
            .get(&key)
            .map(|series| series.snapshot(boundaries))
    }
}

struct CounterMetric {
    _desc: MetricDescriptor,
    series: DashMap<LabelKey, Arc<AtomicU64>>,
}

struct HistogramMetric {
    _desc: MetricDescriptor,
    series: DashMap<LabelKey, Arc<HistogramSeries>>,
    bucket_count: usize,
}

struct HistogramSeries {
    buckets: Vec<AtomicU64>,
    sum_bits: AtomicU64,
    count: AtomicU64,
}

impl HistogramSeries {
    fn new(bucket_count: usize) -> Self {
        let mut buckets = Vec::with_capacity(bucket_count);
        for _ in 0..bucket_count {
            buckets.push(AtomicU64::new(0));
        }
        Self {
            buckets,
            sum_bits: AtomicU64::new(0f64.to_bits()),
            count: AtomicU64::new(0),
        }
    }

    fn observe(&self, index: usize, value: f64) {
        if let Some(bucket) = self.buckets.get(index) {
            bucket.fetch_add(1, Ordering::Relaxed);
        }
        self.count.fetch_add(1, Ordering::Relaxed);
        add_f64(&self.sum_bits, value);
    }

    fn snapshot(&self, boundaries: &[f64]) -> HistogramSnapshot {
        let sum = f64::from_bits(self.sum_bits.load(Ordering::Relaxed));
        let count = self.count.load(Ordering::Relaxed);
        let mut buckets_snapshot = Vec::with_capacity(self.buckets.len());
        for (idx, counter) in self.buckets.iter().enumerate() {
            let boundary = if idx < boundaries.len() {
                boundaries[idx]
            } else {
                f64::INFINITY
            };
            buckets_snapshot.push((boundary, counter.load(Ordering::Relaxed)));
        }
        HistogramSnapshot {
            sum,
            count,
            buckets: buckets_snapshot,
        }
    }
}

#[derive(Clone, Debug, Eq)]
struct LabelKey {
    values: Vec<String>,
    hash: u64,
}

impl PartialEq for LabelKey {
    fn eq(&self, other: &Self) -> bool {
        self.values == other.values
    }
}

impl Hash for LabelKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u64(self.hash);
    }
}

impl LabelKey {
    fn new(values: Vec<String>) -> Self {
        let mut hasher = DefaultHasher::new();
        values.hash(&mut hasher);
        let hash = hasher.finish();
        Self { values, hash }
    }
}

fn normalize_labels(
    desc: MetricDescriptor,
    labels: &[(&'static str, &str)],
) -> Result<LabelKey, MetricError> {
    if labels.len() != desc.labels.len() {
        if labels.len() < desc.labels.len() {
            let missing = desc
                .labels
                .iter()
                .find(|expected| labels.iter().all(|(name, _)| name != *expected))
                .copied()
                .unwrap_or("");
            return Err(MetricError::MissingLabel {
                metric: desc.name,
                label: missing,
            });
        } else {
            let unexpected = labels
                .iter()
                .find(|(name, _)| !desc.labels.iter().any(|expected| expected == name))
                .map(|(name, _)| *name)
                .unwrap_or("");
            return Err(MetricError::UnexpectedLabel {
                metric: desc.name,
                label: unexpected,
            });
        }
    }
    let mut ordered = Vec::with_capacity(desc.labels.len());
    for expected in desc.labels {
        let value = labels
            .iter()
            .find_map(|(name, value)| {
                if *name == *expected {
                    Some(*value)
                } else {
                    None
                }
            })
            .ok_or(MetricError::MissingLabel {
                metric: desc.name,
                label: expected,
            })?;
        ordered.push(value.to_string());
    }
    Ok(LabelKey::new(ordered))
}

fn locate_bucket(boundaries: &[f64], value: f64) -> usize {
    for (idx, boundary) in boundaries.iter().enumerate() {
        if value <= *boundary {
            return idx;
        }
    }
    boundaries.len()
}

fn add_f64(target: &AtomicU64, value: f64) {
    let mut current = target.load(Ordering::Relaxed);
    loop {
        let current_val = f64::from_bits(current);
        let new_val = current_val + value;
        let new_bits = new_val.to_bits();
        match target.compare_exchange_weak(current, new_bits, Ordering::Relaxed, Ordering::Relaxed)
        {
            Ok(_) => break,
            Err(observed) => current = observed,
        }
    }
}
