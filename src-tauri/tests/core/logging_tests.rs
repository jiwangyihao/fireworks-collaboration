// 从 src/logging.rs 迁移的测试
use fireworks_collaboration_lib::logging::init_logging;

#[test]
fn test_init_logging_idempotent() {
    // 调用两次不应 panic
    init_logging();
    init_logging();
    // 发一条日志确保不会崩
    tracing::info!(target = "app", "test log after init");
}
