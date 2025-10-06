use thiserror::Error;

#[derive(Debug, Error)]
pub enum MetricError {
    #[error("metric '{0}' already registered")]
    AlreadyRegistered(&'static str),
    #[error("metric '{0}' is not registered")]
    NotRegistered(&'static str),
    #[error("metric '{0}' is missing histogram buckets")]
    MissingBuckets(&'static str),
    #[error("metric '{metric}' missing required label '{label}'")]
    MissingLabel {
        metric: &'static str,
        label: &'static str,
    },
    #[error("metric '{metric}' received unexpected label '{label}'")]
    UnexpectedLabel {
        metric: &'static str,
        label: &'static str,
    },
    #[error("metric '{0}' kind not supported yet")]
    UnsupportedKind(&'static str),
    #[error("metric aggregator not initialized")]
    AggregatorDisabled,
    #[error("metric '{metric}' window series not found")]
    SeriesNotFound { metric: &'static str },
    #[error("invalid quantile '{0}' supplied")]
    InvalidQuantile(f64),
}

#[derive(Debug, Error)]
pub enum MetricInitError {
    #[error(transparent)]
    Metric(#[from] MetricError),
    #[error("event bus unavailable: {0}")]
    EventBus(&'static str),
    #[error("metrics export initialization failed: {0}")]
    Export(String),
    #[error("metrics alerts initialization failed: {0}")]
    Alerts(String),
}
