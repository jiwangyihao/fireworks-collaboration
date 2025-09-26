//! git_helpers: Git 错误分类与断言公共工具。
//! 提供能力：
//!  * error_category / assert_err_category / expect_err_category  (原有：精确匹配)
//!  * assert_err_in / expect_err_in (新增：多类别容忍集合)
//!  * map_err_category  (Result -> Option<ErrorCategory>)
//! 用途：减少各聚合测试文件中匹配错误分类的重复代码；统一 panic 信息格式，便于快速定位。
//! 设计：保持完全向后兼容——旧 API 不修改签名。

use fireworks_collaboration_lib::core::git::errors::{ErrorCategory, GitError};

/// 提取错误分类（若不是 Categorized 则 panic，帮助暴露实现差异）。
pub fn error_category(err: GitError) -> ErrorCategory {
    match err {
        GitError::Categorized { category, .. } => category,
    }
}
fn error_category_ref(err: &GitError) -> ErrorCategory {
    match err {
        GitError::Categorized { category, .. } => *category,
    }
}

/// 断言错误分类匹配，输出带标签上下文。
pub fn assert_err_category(label: &str, err: GitError, want: ErrorCategory) {
    let got = error_category(err);
    assert_eq!(
        got as u32, want as u32,
        "[{label}] expect {:?} got {:?}",
        want, got
    );
}

/// 简化 Option<Result<..>> 场景：如果返回 Ok 则触发失败（期望出错）。
pub fn expect_err_category<T>(label: &str, r: Result<T, GitError>, want: ErrorCategory) {
    match r {
        Ok(_) => panic!("[{label}] expected error {:?} but got Ok", want),
        Err(e) => assert_err_category(label, e, want),
    }
}

/// 将 Result 转换为分类（Ok -> None, Err -> Some(category)）。非 Categorized 错误会 panic，保持与 error_category 一致策略。
pub fn map_err_category<T>(r: &Result<T, GitError>) -> Option<ErrorCategory> {
    match r {
        Ok(_) => None,
        Err(e) => Some(error_category_ref(e)),
    }
}

/// 多类别容忍断言：期望 err 的分类属于 want 列表之一。
pub fn assert_err_in(label: &str, err: GitError, want: &[ErrorCategory]) {
    let got = error_category(err);
    if !want.iter().any(|c| *c as u32 == got as u32) {
        panic!("[{label}] expect one of {:?} got {:?}", want, got);
    }
}

/// 组合 expect + 多类别；如果是 Ok 则 panic。
pub fn expect_err_in<T>(label: &str, r: Result<T, GitError>, want: &[ErrorCategory]) {
    match r {
        Ok(_) => panic!("[{label}] expected error in {:?} but got Ok", want),
        Err(e) => assert_err_in(label, e, want),
    }
}

/// 断言进度百分比序列单调不下降（允许相等）。
/// 提供统一的失败信息格式，减少各测试文件内重复循环代码。
#[allow(dead_code)]
pub fn assert_progress_monotonic(label: &str, percents: &[u32]) {
    assert!(
        percents.len() >= 2,
        "[{label}] expect >=2 progress events, got {:?}",
        percents
    );
    for w in percents.windows(2) {
        if w[1] < w[0] {
            panic!("[{label}] progress not monotonic: {:?}", percents);
        }
    }
}

#[cfg(test)]
mod tests_git_helpers {
    use super::*;
    // 构造两个分类错误（通过简单匹配 existing 枚举变体)——这里依赖生产枚举 Variants 已在 crate 中。
    fn fake_err(cat: ErrorCategory) -> GitError {
        GitError::Categorized {
            category: cat,
            message: format!("cat={:?}", cat),
        }
    }

    #[test]
    fn map_err_ok_none() {
        let r: Result<(), GitError> = Ok(());
        assert!(map_err_category(&r).is_none());
    }

    #[test]
    fn map_err_some() {
        let r: Result<(), GitError> = Err(fake_err(ErrorCategory::Protocol));
        assert_eq!(map_err_category(&r), Some(ErrorCategory::Protocol));
    }

    #[test]
    fn assert_in_accepts_any() {
        assert_err_in(
            "multi",
            fake_err(ErrorCategory::Protocol),
            &[ErrorCategory::Protocol, ErrorCategory::Cancel],
        );
    }

    #[test]
    #[should_panic]
    fn assert_in_panics_on_miss() {
        assert_err_in(
            "multi-miss",
            fake_err(ErrorCategory::Protocol),
            &[ErrorCategory::Cancel],
        );
    }

    #[test]
    fn expect_in_err_branch() {
        let r: Result<(), GitError> = Err(fake_err(ErrorCategory::Cancel));
        expect_err_in(
            "multi-exp",
            r,
            &[ErrorCategory::Protocol, ErrorCategory::Cancel],
        );
    }

    #[test]
    fn assert_and_expect_category_basic() {
        let e1 = fake_err(ErrorCategory::Protocol);
        assert_err_category("proto", e1, ErrorCategory::Protocol);
        let r: Result<(), GitError> = Err(fake_err(ErrorCategory::Cancel));
        expect_err_category("cancel", r, ErrorCategory::Cancel);
    }
}
