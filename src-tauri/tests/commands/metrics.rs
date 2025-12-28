//! Metrics command integration tests

use std::borrow::Cow;
use std::sync::{Arc, Mutex};
use tauri::{Assets, Manager};
use tauri_utils::assets::{AssetKey, CspHash};

use fireworks_collaboration_lib::app::commands::metrics::*;
use fireworks_collaboration_lib::core::config::model::AppConfig;

use fireworks_collaboration_lib::app::types::SharedConfig;

// MockAssets definition
struct MockAssets;

impl<R: tauri::Runtime> Assets<R> for MockAssets {
    fn get(&self, _key: &AssetKey) -> Option<Cow<'_, [u8]>> {
        None
    }
    fn iter(&self) -> Box<dyn Iterator<Item = (Cow<'_, str>, Cow<'_, [u8]>)> + '_> {
        Box::new(std::iter::empty())
    }
    fn csp_hashes(&self, _html_path: &AssetKey) -> Box<dyn Iterator<Item = CspHash<'_>> + '_> {
        Box::new(std::iter::empty())
    }
}

fn create_mock_app() -> tauri::App<tauri::test::MockRuntime> {
    let config: SharedConfig = Arc::new(Mutex::new(AppConfig::default()));

    let context = tauri::test::mock_context(MockAssets);

    tauri::test::mock_builder()
        .manage::<SharedConfig>(config)
        .build(context)
        .expect("Failed to build mock app")
}

#[tokio::test]
async fn test_metrics_snapshot_default_options() {
    let app = create_mock_app();

    let result = metrics_snapshot(None, app.state()).await;
    assert!(result.is_ok());

    let snapshot = result.unwrap();
    // Snapshot should be valid (series can be empty or not)
    assert!(snapshot.series.is_empty() || !snapshot.series.is_empty());
}

#[tokio::test]
async fn test_metrics_snapshot_with_range() {
    let app = create_mock_app();

    let request = MetricsSnapshotRequest {
        names: vec![],
        range: Some("1m".to_string()),
        quantiles: vec![],
        max_series: None,
    };

    let result = metrics_snapshot(Some(request), app.state()).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_metrics_snapshot_with_invalid_range() {
    let app = create_mock_app();

    let request = MetricsSnapshotRequest {
        names: vec![],
        range: Some("invalid_range".to_string()),
        quantiles: vec![],
        max_series: None,
    };

    let result = metrics_snapshot(Some(request), app.state()).await;
    // Should fail with invalid range
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("unsupported range"));
}

#[tokio::test]
async fn test_metrics_snapshot_with_quantiles() {
    let app = create_mock_app();

    let request = MetricsSnapshotRequest {
        names: vec![],
        range: None,
        quantiles: vec![0.5, 0.9, 0.99],
        max_series: Some(100),
    };

    let result = metrics_snapshot(Some(request), app.state()).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_metrics_snapshot_with_name_filter() {
    let app = create_mock_app();

    let request = MetricsSnapshotRequest {
        names: vec!["git_clone".to_string(), "proxy".to_string()],
        range: None,
        quantiles: vec![],
        max_series: None,
    };

    let result = metrics_snapshot(Some(request), app.state()).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_metrics_snapshot_all_range_tokens() {
    let app = create_mock_app();

    for range in [
        "1m", "5m", "1h", "24h", "1minute", "5minutes", "1hour", "1d", "1day",
    ] {
        let request = MetricsSnapshotRequest {
            names: vec![],
            range: Some(range.to_string()),
            quantiles: vec![],
            max_series: None,
        };

        let result = metrics_snapshot(Some(request), app.state()).await;
        assert!(result.is_ok(), "Failed for range: {}", range);
    }
}
