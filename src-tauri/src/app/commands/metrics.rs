use std::cmp::Ordering;

use serde::Deserialize;
use tauri::State;

use crate::app::types::SharedConfig;
use crate::core::metrics::{
    build_snapshot, global_registry, MetricsSnapshot, SnapshotQuery, WindowRange,
};

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetricsSnapshotRequest {
    #[serde(default)]
    pub names: Vec<String>,
    pub range: Option<String>,
    #[serde(default)]
    pub quantiles: Vec<f64>,
    pub max_series: Option<usize>,
}

#[tauri::command]
pub async fn metrics_snapshot(
    options: Option<MetricsSnapshotRequest>,
    cfg: State<'_, SharedConfig>,
) -> Result<MetricsSnapshot, String> {
    let options = options.unwrap_or_default();

    let mut query = SnapshotQuery::default();
    let requested_max = options.max_series;
    if !options.names.is_empty() {
        query.names = options.names;
    }
    if let Some(range_token) = options.range {
        query.range = Some(parse_range_token(&range_token)?);
    }
    if !options.quantiles.is_empty() {
        query.quantiles = sanitize_quantiles(options.quantiles);
    }
    query.max_series = requested_max;

    let default_limit = cfg
        .lock()
        .map(|guard| guard.observability.export.max_series_per_snapshot as usize)
        .unwrap_or(1_000);

    let registry = global_registry();
    let snapshot = build_snapshot(&registry, &query, default_limit);
    Ok(snapshot)
}

fn parse_range_token(value: &str) -> Result<WindowRange, String> {
    match value {
        "1m" | "1minute" => Ok(WindowRange::LastMinute),
        "5m" | "5minutes" => Ok(WindowRange::LastFiveMinutes),
        "1h" | "1hour" => Ok(WindowRange::LastHour),
        "24h" | "1d" | "1day" => Ok(WindowRange::LastDay),
        other => Err(format!("unsupported range '{other}'")),
    }
}

fn sanitize_quantiles(mut values: Vec<f64>) -> Vec<f64> {
    values.retain(|q| q.is_finite() && *q > 0.0 && *q < 1.0);
    values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Equal));
    values.dedup_by(|a, b| (a - b).abs() < f64::EPSILON);
    values
}
