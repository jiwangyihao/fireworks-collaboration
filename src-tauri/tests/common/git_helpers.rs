//! git_helpers: 测试中统一的 Git 错误分类/断言工具，避免各聚合文件重复定义 `cat` / `assert_*`。
use fireworks_collaboration_lib::core::git::errors::{GitError, ErrorCategory};

/// 提取错误分类（若不是 Categorized 则 panic，帮助暴露实现差异）。
pub fn error_category(err: GitError) -> ErrorCategory {
    match err { GitError::Categorized { category, .. } => category }
}

/// 断言错误分类匹配，输出带标签上下文。
pub fn assert_err_category(label: &str, err: GitError, want: ErrorCategory) {
    let got = error_category(err);
    assert_eq!(got as u32, want as u32, "[{label}] expect {:?} got {:?}", want, got);
}

/// 简化 Option<Result<..>> 场景：如果返回 Ok 则触发失败（期望出错）。
pub fn expect_err_category<T>(label: &str, r: Result<T, GitError>, want: ErrorCategory) {
    match r { Ok(_) => panic!("[{label}] expected error {:?} but got Ok", want), Err(e) => assert_err_category(label, e, want) }
}
