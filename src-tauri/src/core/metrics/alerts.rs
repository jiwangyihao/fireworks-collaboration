use std::collections::{hash_map::DefaultHasher, HashMap, HashSet};
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use dashmap::{mapref::entry::Entry, DashMap};
use serde::Deserialize;
use thiserror::Error;
use tracing::{debug, warn};

use crate::core::config::model::ObservabilityAlertsConfig;
use crate::events::structured::{publish_global, Event, MetricAlertState, StrategyEvent};

use super::descriptors::find_descriptor;
use super::error::MetricError;
use super::registry::MetricRegistry;
use super::MetricDescriptor;
use super::{MetricKind, WindowRange};

const EPSILON: f64 = 1e-9;

#[derive(Debug, Error)]
pub enum AlertError {
    #[error("alert rule parse error: {0}")]
    Parse(String),
    #[error("alert rules io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("metric error: {0}")]
    Metric(#[from] MetricError),
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
enum AlertSeverity {
    Info,
    Warn,
    Critical,
}

impl AlertSeverity {
    fn as_str(self) -> &'static str {
        match self {
            AlertSeverity::Info => "info",
            AlertSeverity::Warn => "warn",
            AlertSeverity::Critical => "critical",
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum Comparator {
    Greater,
    GreaterEqual,
    Less,
    LessEqual,
}

impl Comparator {
    fn as_str(self) -> &'static str {
        match self {
            Comparator::Greater => ">",
            Comparator::GreaterEqual => ">=",
            Comparator::Less => "<",
            Comparator::LessEqual => "<=",
        }
    }

    fn evaluate(self, lhs: f64, rhs: f64) -> bool {
        match self {
            Comparator::Greater => lhs > rhs,
            Comparator::GreaterEqual => lhs >= rhs,
            Comparator::Less => lhs < rhs,
            Comparator::LessEqual => lhs <= rhs,
        }
    }
}

#[derive(Debug, Clone)]
struct AlertComparison {
    lhs: AlertExpr,
    rhs: AlertExpr,
    comparator: Comparator,
}

#[derive(Debug, Clone)]
enum AlertExpr {
    Number(f64),
    Metric(CompiledMetricRef),
    Divide(Box<AlertExpr>, Box<AlertExpr>),
}

#[derive(Debug, Clone)]
struct CompiledMetricRef {
    descriptor: MetricDescriptor,
    filters: Vec<FilterSpec>,
    quantile: Option<f64>,
    average: bool,
}

impl CompiledMetricRef {
    fn matches(&self, labels: &[String]) -> bool {
        self.filters.iter().all(|filter| {
            labels
                .get(filter.index)
                .map(|value| value == &filter.value)
                .unwrap_or(false)
        })
    }
}

#[derive(Debug, Clone)]
struct FilterSpec {
    index: usize,
    value: String,
}

#[derive(Debug, Clone)]
struct CompiledRule {
    id: String,
    severity: AlertSeverity,
    window: WindowRange,
    comparison: AlertComparison,
    description: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AlertRuleDefinition {
    id: String,
    expr: String,
    severity: AlertSeverity,
    #[serde(default)]
    window: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default = "default_true")]
    enabled: bool,
}

fn default_true() -> bool {
    true
}

struct RuleStatus {
    active: bool,
    last_emit: Option<Instant>,
}

impl RuleStatus {
    fn new() -> Self {
        Self {
            active: false,
            last_emit: None,
        }
    }
}

pub struct AlertEngine {
    registry: Arc<MetricRegistry>,
    rules_path: PathBuf,
    rules: RwLock<Arc<Vec<CompiledRule>>>,
    rules_hash: Mutex<Option<u64>>,
    states: DashMap<String, RuleStatus>,
    eval_interval: Duration,
    min_repeat: Duration,
}

impl AlertEngine {
    pub fn new(
        registry: Arc<MetricRegistry>,
        cfg: ObservabilityAlertsConfig,
    ) -> Result<Self, AlertError> {
        let rules_path = PathBuf::from(&cfg.rules_path);
        let eval_interval = if cfg.eval_interval_secs == 0 {
            Duration::from_secs(0)
        } else {
            Duration::from_secs(cfg.eval_interval_secs as u64)
        };
        let min_repeat = Duration::from_secs(cfg.min_repeat_interval_secs as u64);
        let engine = Self {
            registry,
            rules_path,
            rules: RwLock::new(Arc::new(Vec::new())),
            rules_hash: Mutex::new(None),
            states: DashMap::new(),
            eval_interval,
            min_repeat,
        };
        engine.reload_rules_if_needed()?;
        Ok(engine)
    }

    pub fn spawn(self: &Arc<Self>) {
        if self.eval_interval.is_zero() {
            return;
        }
        let interval = self.eval_interval;
        let engine = Arc::clone(self);
        let builder = thread::Builder::new().name("metrics-alerts".into());
        let result = builder.spawn(move || loop {
            engine.evaluate();
            thread::sleep(interval);
        });
        if let Err(err) = result {
            warn!(
                target = "metrics",
                ?err,
                "failed to spawn alert evaluation thread"
            );
        }
    }

    pub fn evaluate(&self) {
        if let Err(err) = self.reload_rules_if_needed() {
            warn!(target = "metrics", ?err, "failed to reload alert rules");
        }
        let rules = {
            let guard = self.rules.read().expect("rules lock poisoned");
            guard.clone()
        };
        if rules.is_empty() {
            return;
        }
        for rule in rules.iter() {
            match self.evaluate_rule(rule) {
                Ok(RuleEvaluation::Triggered { value, threshold }) => {
                    self.handle_trigger(rule, value, threshold)
                }
                Ok(RuleEvaluation::Cleared { value, threshold }) => {
                    self.handle_clear(rule, value, threshold)
                }
                Ok(RuleEvaluation::NoData) => {}
                Err(err) => warn!(
                    target = "metrics",
                    rule = rule.id,
                    ?err,
                    "alert evaluation failed"
                ),
            }
        }
    }

    fn reload_rules_if_needed(&self) -> Result<(), AlertError> {
        let path = self.rules_path.clone();
        let source = match fs::read_to_string(&path) {
            Ok(content) => Some(content),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => None,
            Err(err) => return Err(AlertError::Io(err)),
        };
        let hash = source
            .as_ref()
            .map(|content| hash_source(content))
            .or(Some(0));
        let mut guard = self.rules_hash.lock().expect("rules_hash poisoned");
        if guard.as_ref() == hash.as_ref() {
            return Ok(());
        }
        let compiled = self.build_rules(source.as_deref())?;
        let ids: HashSet<String> = compiled.iter().map(|rule| rule.id.clone()).collect();
        {
            let mut rules_guard = self.rules.write().expect("rules lock poisoned");
            *rules_guard = Arc::new(compiled);
        }
        self.prune_states(&ids);
        *guard = hash;
        Ok(())
    }

    fn build_rules(&self, source: Option<&str>) -> Result<Vec<CompiledRule>, AlertError> {
        let mut all: HashMap<String, AlertRuleDefinition> = HashMap::new();
        for rule in builtin_rule_definitions() {
            all.insert(rule.id.clone(), rule);
        }
        if let Some(raw) = source {
            if !raw.trim().is_empty() {
                let user_rules: Vec<AlertRuleDefinition> =
                    serde_json::from_str(raw).map_err(|err| {
                        AlertError::Parse(format!("failed to parse alert rules json: {err}"))
                    })?;
                for rule in user_rules {
                    all.insert(rule.id.clone(), rule);
                }
            }
        }
        let mut merged: Vec<AlertRuleDefinition> = all.into_values().collect();
        merged.sort_by(|a, b| a.id.cmp(&b.id));
        self.compile_definitions(merged)
    }

    fn compile_definitions(
        &self,
        definitions: Vec<AlertRuleDefinition>,
    ) -> Result<Vec<CompiledRule>, AlertError> {
        let mut compiled = Vec::new();
        let mut seen = HashSet::new();
        for definition in definitions {
            if !seen.insert(definition.id.clone()) {
                return Err(AlertError::Parse(format!(
                    "duplicate alert rule id '{}' detected",
                    definition.id
                )));
            }
            if !definition.enabled {
                continue;
            }
            compiled.push(self.compile_rule(&definition)?);
        }
        Ok(compiled)
    }

    fn compile_rule(&self, rule: &AlertRuleDefinition) -> Result<CompiledRule, AlertError> {
        if rule.id.trim().is_empty() {
            return Err(AlertError::Parse(
                "alert rule 'id' must not be empty".to_string(),
            ));
        }
        if rule.expr.trim().is_empty() {
            return Err(AlertError::Parse(format!(
                "alert rule '{}' expression must not be empty",
                rule.id
            )));
        }
        let window = parse_window(rule.window.as_deref())
            .map_err(|err| AlertError::Parse(format!("rule '{}': {err}", rule.id)))?;
        let comparison = self
            .parse_comparison(rule.expr.trim())
            .map_err(|err| AlertError::Parse(format!("rule '{}': {err}", rule.id)))?;
        Ok(CompiledRule {
            id: rule.id.clone(),
            severity: rule.severity,
            window,
            comparison,
            description: rule.description.clone(),
        })
    }

    fn parse_comparison(&self, input: &str) -> Result<AlertComparison, String> {
        let mut brace_depth: usize = 0;
        let mut bracket_depth: usize = 0;
        let mut comparator_pos: Option<(usize, usize, Comparator)> = None;
        let chars: Vec<char> = input.chars().collect();
        let mut idx = 0;
        while idx < chars.len() {
            match chars[idx] {
                '{' => brace_depth += 1,
                '}' => brace_depth = brace_depth.saturating_sub(1),
                '[' => bracket_depth += 1,
                ']' => bracket_depth = bracket_depth.saturating_sub(1),
                '>' | '<' if brace_depth == 0 && bracket_depth == 0 => {
                    let start = idx;
                    let (length, comparator) = if chars[idx] == '>' {
                        if chars.get(idx + 1) == Some(&'=') {
                            (2, Comparator::GreaterEqual)
                        } else {
                            (1, Comparator::Greater)
                        }
                    } else if chars.get(idx + 1) == Some(&'=') {
                        (2, Comparator::LessEqual)
                    } else {
                        (1, Comparator::Less)
                    };
                    comparator_pos = Some((start, length, comparator));
                    break;
                }
                _ => {}
            }
            idx += 1;
        }
        let (pos, comp_len, comparator) = comparator_pos.ok_or_else(|| {
            format!("missing comparator ('<', '>', '<=', '>=') in expression '{input}'")
        })?;
        let lhs = input[..pos].trim();
        let rhs = input[pos + comp_len..].trim();
        if lhs.is_empty() || rhs.is_empty() {
            return Err(format!("invalid comparison expression '{input}'"));
        }
        let lhs_expr = self.parse_expr(lhs)?;
        let rhs_expr = self.parse_expr(rhs)?;
        Ok(AlertComparison {
            lhs: lhs_expr,
            rhs: rhs_expr,
            comparator,
        })
    }

    fn parse_expr(&self, input: &str) -> Result<AlertExpr, String> {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Err("empty expression".to_string());
        }
        let mut brace_depth: usize = 0;
        let mut bracket_depth: usize = 0;
        let chars: Vec<char> = trimmed.chars().collect();
        for (idx, ch) in chars.iter().enumerate() {
            match ch {
                '{' => brace_depth += 1,
                '}' => brace_depth = brace_depth.saturating_sub(1),
                '[' => bracket_depth += 1,
                ']' => bracket_depth = bracket_depth.saturating_sub(1),
                '/' if brace_depth == 0 && bracket_depth == 0 => {
                    let left = &trimmed[..idx];
                    let right = &trimmed[idx + 1..];
                    return Ok(AlertExpr::Divide(
                        Box::new(self.parse_expr(left)?),
                        Box::new(self.parse_expr(right)?),
                    ));
                }
                _ => {}
            }
        }
        self.parse_term(trimmed)
    }

    fn parse_term(&self, input: &str) -> Result<AlertExpr, String> {
        if let Ok(number) = parse_number(input) {
            return Ok(AlertExpr::Number(number));
        }
        let metric = self.parse_metric(input)?;
        Ok(AlertExpr::Metric(metric))
    }

    fn parse_metric(&self, input: &str) -> Result<CompiledMetricRef, String> {
        let mut name_end = input.len();
        let mut idx = 0;
        let chars: Vec<char> = input.chars().collect();
        let mut quantile: Option<f64> = None;
        let mut labels_part: Option<&str> = None;
        while idx < chars.len() {
            match chars[idx] {
                '[' => {
                    name_end = idx;
                    let close = find_matching(&chars, idx, '[', ']')
                        .ok_or_else(|| format!("unbalanced '[' in metric '{input}'"))?;
                    let inside = input[idx + 1..close].trim();
                    quantile = Some(parse_quantile(inside)?);
                    idx = close + 1;
                }
                '{' => {
                    if name_end == input.len() {
                        name_end = idx;
                    }
                    let close = find_matching(&chars, idx, '{', '}')
                        .ok_or_else(|| format!("unbalanced '{{' in metric '{input}'"))?;
                    labels_part = Some(&input[idx + 1..close]);
                    break;
                }
                _ => idx += 1,
            }
        }
        if name_end == input.len() {
            name_end = input.len();
        }
        let metric_name = input[..name_end].trim();
        if metric_name.is_empty() {
            return Err(format!("invalid metric reference '{input}'"));
        }
        let descriptor = find_descriptor(metric_name).ok_or_else(|| {
            format!("unknown metric '{metric_name}' referenced in alert expression")
        })?;
        if quantile.is_some() && descriptor.kind != MetricKind::Histogram {
            return Err(format!("metric '{metric_name}' does not support quantiles"));
        }
        let filters = parse_filters(descriptor, labels_part)?;
        let average = descriptor.kind == MetricKind::Histogram && quantile.is_none();
        Ok(CompiledMetricRef {
            descriptor,
            filters,
            quantile,
            average,
        })
    }

    fn evaluate_rule(&self, rule: &CompiledRule) -> Result<RuleEvaluation, AlertError> {
        let lhs = self.evaluate_expr(&rule.comparison.lhs, rule.window)?;
        let rhs = self.evaluate_expr(&rule.comparison.rhs, rule.window)?;
        match (lhs, rhs) {
            (Some(lhs_val), Some(rhs_val)) => {
                let triggered = rule.comparison.comparator.evaluate(lhs_val, rhs_val);
                if triggered {
                    Ok(RuleEvaluation::Triggered {
                        value: lhs_val,
                        threshold: rhs_val,
                    })
                } else {
                    Ok(RuleEvaluation::Cleared {
                        value: lhs_val,
                        threshold: rhs_val,
                    })
                }
            }
            _ => Ok(RuleEvaluation::NoData),
        }
    }

    fn evaluate_expr(
        &self,
        expr: &AlertExpr,
        window: WindowRange,
    ) -> Result<Option<f64>, AlertError> {
        match expr {
            AlertExpr::Number(value) => Ok(Some(*value)),
            AlertExpr::Metric(metric) => self.evaluate_metric(metric, window),
            AlertExpr::Divide(lhs, rhs) => {
                let numerator = self.evaluate_expr(lhs, window)?;
                let denominator = self.evaluate_expr(rhs, window)?;
                match (numerator, denominator) {
                    (Some(num), Some(den)) if den.abs() > EPSILON => Ok(Some(num / den)),
                    _ => Ok(None),
                }
            }
        }
    }

    fn evaluate_metric(
        &self,
        metric: &CompiledMetricRef,
        window: WindowRange,
    ) -> Result<Option<f64>, AlertError> {
        match metric.descriptor.kind {
            MetricKind::Counter => self.evaluate_counter(metric, window),
            MetricKind::Histogram => self.evaluate_histogram(metric, window),
            MetricKind::Gauge => Err(AlertError::Parse(format!(
                "metric '{}' of kind Gauge is not supported",
                metric.descriptor.name
            ))),
        }
    }

    fn evaluate_counter(
        &self,
        metric: &CompiledMetricRef,
        window: WindowRange,
    ) -> Result<Option<f64>, AlertError> {
        let series = self.registry.list_counter_series(metric.descriptor)?;
        if series.is_empty() {
            return Ok(None);
        }
        let mut total = 0.0;
        let mut matched = false;
        for entry in series {
            let labels = entry.labels;
            if metric.matches(&labels) {
                matched = true;
                let pairs = labels_as_pairs(metric.descriptor, &labels);
                let snapshot =
                    self.registry
                        .snapshot_counter_window(metric.descriptor, &pairs, window)?;
                total += snapshot.total as f64;
            }
        }
        if matched {
            Ok(Some(total))
        } else {
            Ok(None)
        }
    }

    fn evaluate_histogram(
        &self,
        metric: &CompiledMetricRef,
        window: WindowRange,
    ) -> Result<Option<f64>, AlertError> {
        let series = self.registry.list_histogram_series(metric.descriptor)?;
        if series.is_empty() {
            return Ok(None);
        }
        let mut matched_labels = Vec::new();
        for entry in series {
            if metric.matches(&entry.labels) {
                matched_labels.push(entry.labels);
            }
        }
        if matched_labels.is_empty() {
            return Ok(None);
        }
        if metric.quantile.is_some() && matched_labels.len() > 1 {
            warn!(
                target = "metrics",
                metric = metric.descriptor.name,
                "quantile evaluation skipped: multiple label series matched"
            );
            return Ok(None);
        }
        if let Some(target_quantile) = metric.quantile {
            let labels = matched_labels.into_iter().next().unwrap();
            let pairs = labels_as_pairs(metric.descriptor, &labels);
            let snapshot = self.registry.snapshot_histogram_window(
                metric.descriptor,
                &pairs,
                window,
                &[target_quantile],
            )?;
            if snapshot.count == 0 {
                return Ok(None);
            }
            let value = snapshot
                .quantiles
                .iter()
                .find(|(q, _)| (*q - target_quantile).abs() < 1e-6)
                .map(|(_, value)| *value)
                .unwrap_or(0.0);
            return Ok(Some(value));
        }
        let mut sum = 0.0;
        let mut count = 0.0;
        for labels in matched_labels {
            let pairs = labels_as_pairs(metric.descriptor, &labels);
            let snapshot =
                self.registry
                    .snapshot_histogram_window(metric.descriptor, &pairs, window, &[])?;
            sum += snapshot.sum;
            count += snapshot.count as f64;
        }
        if count > 0.0 {
            Ok(Some(sum / count))
        } else {
            Ok(None)
        }
    }

    fn handle_trigger(&self, rule: &CompiledRule, value: f64, threshold: f64) {
        let now = Instant::now();
        match self.states.entry(rule.id.clone()) {
            Entry::Vacant(vacant) => {
                let mut status = RuleStatus::new();
                status.active = true;
                status.last_emit = Some(now);
                vacant.insert(status);
                self.emit_event(rule, MetricAlertState::Firing, value, threshold);
            }
            Entry::Occupied(mut occ) => {
                let status = occ.get_mut();
                if !status.active {
                    status.active = true;
                    status.last_emit = Some(now);
                    self.emit_event(rule, MetricAlertState::Firing, value, threshold);
                } else if self.min_repeat.is_zero()
                    || status
                        .last_emit
                        .map(|last| now.duration_since(last) >= self.min_repeat)
                        .unwrap_or(true)
                {
                    status.last_emit = Some(now);
                    self.emit_event(rule, MetricAlertState::Active, value, threshold);
                }
            }
        }
    }

    fn handle_clear(&self, rule: &CompiledRule, value: f64, threshold: f64) {
        if let Some(mut entry) = self.states.get_mut(rule.id.as_str()) {
            let status = entry.value_mut();
            if status.active {
                status.active = false;
                status.last_emit = Some(Instant::now());
                drop(entry);
                self.emit_event(rule, MetricAlertState::Resolved, value, threshold);
            }
        }
    }

    fn emit_event(&self, rule: &CompiledRule, state: MetricAlertState, value: f64, threshold: f64) {
        let timestamp_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .ok()
            .map(|dur| dur.as_millis() as u64);
        let severity = rule.severity.as_str().to_string();
        let comparator = rule.comparison.comparator.as_str().to_string();
        debug!(
            target = "metrics",
            rule = rule.id,
            %severity,
            ?state,
            value,
            threshold,
            comparator,
            "metric alert state transition",
        );
        publish_global(Event::Strategy(StrategyEvent::MetricAlert {
            rule_id: rule.id.clone(),
            severity,
            state,
            value,
            threshold,
            comparator,
            timestamp_ms,
        }));
    }

    fn prune_states(&self, valid_ids: &HashSet<String>) {
        self.states.retain(|rule_id, _| valid_ids.contains(rule_id));
    }
}

fn builtin_rule_definitions() -> Vec<AlertRuleDefinition> {
    vec![
        AlertRuleDefinition {
            id: "git_fail_rate".to_string(),
            expr: "git_tasks_total{state=failed}/git_tasks_total > 0.05".to_string(),
            severity: AlertSeverity::Warn,
            window: Some("5m".to_string()),
            description: Some("Git 任务失败率超过 5%".to_string()),
            enabled: true,
        },
        AlertRuleDefinition {
            id: "tls_latency_p95".to_string(),
            expr: "tls_handshake_ms[p95] > 800".to_string(),
            severity: AlertSeverity::Warn,
            window: Some("5m".to_string()),
            description: Some("TLS 握手 P95 超过 800ms".to_string()),
            enabled: true,
        },
        AlertRuleDefinition {
            id: "ip_refresh_success_low".to_string(),
            expr: "ip_pool_refresh_total{success=true}/ip_pool_refresh_total < 0.85".to_string(),
            severity: AlertSeverity::Critical,
            window: Some("5m".to_string()),
            description: Some("IP 池刷新成功率低于 85%".to_string()),
            enabled: true,
        },
    ]
}

enum RuleEvaluation {
    Triggered { value: f64, threshold: f64 },
    Cleared { value: f64, threshold: f64 },
    NoData,
}

fn labels_as_pairs<'a>(
    descriptor: MetricDescriptor,
    values: &'a [String],
) -> Vec<(&'static str, &'a str)> {
    descriptor
        .labels
        .iter()
        .zip(values.iter())
        .map(|(name, value)| (*name, value.as_str()))
        .collect()
}

fn parse_number(input: &str) -> Result<f64, std::num::ParseFloatError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return "".parse();
    }
    let mut scale = 1.0;
    let mut without_suffix = trimmed;
    if let Some(stripped) = trimmed.strip_suffix('%') {
        scale = 0.01;
        without_suffix = stripped.trim();
    }
    if let Some(stripped) = without_suffix.strip_suffix("ms") {
        without_suffix = stripped.trim();
    }
    let value: f64 = without_suffix.parse()?;
    Ok(value * scale)
}

fn parse_quantile(input: &str) -> Result<f64, String> {
    if input.is_empty() {
        return Err("quantile token must not be empty".to_string());
    }
    let normalized = input.trim().to_ascii_lowercase();
    let value = if let Some(stripped) = normalized.strip_prefix('p') {
        stripped
            .parse::<f64>()
            .map_err(|err| format!("invalid quantile '{input}': {err}"))?
            / 100.0
    } else {
        normalized
            .parse::<f64>()
            .map_err(|err| format!("invalid quantile '{input}': {err}"))?
    };
    if !(0.0..=1.0).contains(&value) {
        return Err(format!("quantile '{input}' is out of range [0, 1]"));
    }
    Ok(value)
}

fn parse_filters(
    descriptor: MetricDescriptor,
    labels_part: Option<&str>,
) -> Result<Vec<FilterSpec>, String> {
    let mut filters = Vec::new();
    let Some(part) = labels_part else {
        return Ok(filters);
    };
    for item in part.split(',') {
        let trimmed = item.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Some((name, value)) = trimmed.split_once('=') else {
            return Err(format!("invalid label selector '{trimmed}'"));
        };
        let label_name = name.trim();
        let label_value = value.trim().trim_matches('"').trim_matches('\'');
        if label_value == "*" {
            continue;
        }
        let index = descriptor
            .labels
            .iter()
            .position(|candidate| *candidate == label_name)
            .ok_or_else(|| {
                format!(
                    "unknown label '{label_name}' for metric '{}'",
                    descriptor.name
                )
            })?;
        if filters.iter().any(|filter| filter.index == index) {
            return Err(format!("label '{label_name}' referenced multiple times"));
        }
        filters.push(FilterSpec {
            index,
            value: label_value.to_string(),
        });
    }
    Ok(filters)
}

fn find_matching(chars: &[char], start: usize, open: char, close: char) -> Option<usize> {
    let mut depth = 0;
    for (idx, ch) in chars.iter().enumerate().skip(start) {
        if *ch == open {
            depth += 1;
        } else if *ch == close {
            depth -= 1;
            if depth == 0 {
                return Some(idx);
            }
        }
    }
    None
}

fn parse_window(value: Option<&str>) -> Result<WindowRange, String> {
    match value {
        None => Ok(WindowRange::LastFiveMinutes),
        Some(raw) => {
            let normalized = raw.trim().to_ascii_lowercase();
            if normalized.is_empty() || normalized == "5m" {
                Ok(WindowRange::LastFiveMinutes)
            } else {
                match normalized.as_str() {
                    "1m" => Ok(WindowRange::LastMinute),
                    "1h" => Ok(WindowRange::LastHour),
                    "24h" => Ok(WindowRange::LastDay),
                    other => Err(format!("unsupported window '{other}'")),
                }
            }
        }
    }
}

fn hash_source<S: AsRef<str>>(source: S) -> u64 {
    let mut hasher = DefaultHasher::new();
    source.as_ref().hash(&mut hasher);
    hasher.finish()
}
