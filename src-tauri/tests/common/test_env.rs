use std::{fs, path::PathBuf, sync::Once};

static INIT: Once = Once::new();

/// 统一测试环境初始化：
/// - 日志（tracing）
/// - 必要 Git 环境变量（防止提交操作缺失身份信息）
pub fn init_test_env() {
    INIT.call_once(|| {
        // 简化：若用户未设置 RUST_LOG，则提供一个默认级别。
        if std::env::var("RUST_LOG").is_err() {
            std::env::set_var("RUST_LOG", "info");
        }
        let _ = tracing_subscriber::fmt()
            .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
            .try_init();

        // Git 身份（某些实现可能需要）。
        std::env::set_var("GIT_AUTHOR_NAME", "fwc-test");
        std::env::set_var("GIT_AUTHOR_EMAIL", "fwc-test@example.com");
        std::env::set_var("GIT_COMMITTER_NAME", "fwc-test");
        std::env::set_var("GIT_COMMITTER_EMAIL", "fwc-test@example.com");

        // Git 全局配置：禁用 CRLF 转换，避免测试运行时产生警告。
        std::env::set_var("GIT_CONFIG_NOSYSTEM", "1");
        let cfg_path = git_test_config_path();
        fs::write(
            &cfg_path,
            "[core]\n\tautocrlf = false\n\tsafecrlf = false\n",
        )
        .expect("write git test config");
        std::env::set_var("GIT_CONFIG_GLOBAL", cfg_path.to_string_lossy().to_string());
    });
}

fn git_test_config_path() -> PathBuf {
    std::env::temp_dir().join("fwc-test-gitconfig")
}

#[cfg(test)]
mod tests_env_smoke {
    #[test]
    fn init_env_smoke() {
        // 多次调用应只初始化一次（幂等）
        super::init_test_env();
        super::init_test_env();
    }
}
