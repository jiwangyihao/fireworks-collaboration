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

#[tauri::command(rename_all = "camelCase")]
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
    values.dedup_by(|a, b| (*a - *b).abs() < f64::EPSILON);
    values
}

#[cfg(test)]
mod tests {
    use super::*;

    // -------------------------------------------------------------------------
    // parse_range_token tests
    // -------------------------------------------------------------------------
    #[test]
    fn test_parse_range_token_1m() {
        assert!(matches!(
            parse_range_token("1m"),
            Ok(WindowRange::LastMinute)
        ));
        assert!(matches!(
            parse_range_token("1minute"),
            Ok(WindowRange::LastMinute)
        ));
    }

    #[test]
    fn test_parse_range_token_5m() {
        assert!(matches!(
            parse_range_token("5m"),
            Ok(WindowRange::LastFiveMinutes)
        ));
        assert!(matches!(
            parse_range_token("5minutes"),
            Ok(WindowRange::LastFiveMinutes)
        ));
    }

    #[test]
    fn test_parse_range_token_1h() {
        assert!(matches!(parse_range_token("1h"), Ok(WindowRange::LastHour)));
        assert!(matches!(
            parse_range_token("1hour"),
            Ok(WindowRange::LastHour)
        ));
    }

    #[test]
    fn test_parse_range_token_24h() {
        assert!(matches!(parse_range_token("24h"), Ok(WindowRange::LastDay)));
        assert!(matches!(parse_range_token("1d"), Ok(WindowRange::LastDay)));
        assert!(matches!(
            parse_range_token("1day"),
            Ok(WindowRange::LastDay)
        ));
    }

    #[test]
    fn test_parse_range_token_invalid() {
        let result = parse_range_token("invalid");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("unsupported range"));
    }

    // -------------------------------------------------------------------------
    // sanitize_quantiles tests
    // -------------------------------------------------------------------------
    #[test]
    fn test_sanitize_quantiles_valid() {
        let result = sanitize_quantiles(vec![0.5, 0.9, 0.99]);
        assert_eq!(result, vec![0.5, 0.9, 0.99]);
    }

    #[test]
    fn test_sanitize_quantiles_filters_invalid() {
        // Filters out 0.0, 1.0, negative, NaN, Infinity
        let result = sanitize_quantiles(vec![0.0, 0.5, 1.0, -0.1, f64::NAN, f64::INFINITY, 0.9]);
        assert_eq!(result, vec![0.5, 0.9]);
    }

    #[test]
    fn test_sanitize_quantiles_sorts_and_dedupes() {
        let result = sanitize_quantiles(vec![0.9, 0.5, 0.9, 0.5, 0.75]);
        assert_eq!(result, vec![0.5, 0.75, 0.9]);
    }

    #[test]
    fn test_sanitize_quantiles_empty() {
        let result = sanitize_quantiles(vec![]);
        assert!(result.is_empty());
    }
}
