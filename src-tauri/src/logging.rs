// 简单的 tracing 初始化（P0.1）
use tracing_subscriber::{fmt, EnvFilter};

pub fn init_logging() {
    // 若已经初始化，避免重复 panic
    if tracing::dispatcher::has_been_set() {
        return;
    }
    let env_filter = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new("info"))
        .unwrap();

    let subscriber = fmt()
        .with_env_filter(env_filter)
        .with_target(false)
        .with_level(true)
        .compact()
        .finish();

    let _ = tracing::subscriber::set_global_default(subscriber);
    tracing::info!(target = "app", "tracing initialized");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init_logging_idempotent() {
        // 调用两次不应 panic
        init_logging();
        init_logging();
        // 发一条日志确保不会崩
        tracing::info!(target = "app", "test log after init");
    }
}
