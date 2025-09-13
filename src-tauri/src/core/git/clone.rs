use std::path::{Path, PathBuf};

/// 使用 gix 在阻塞环境中执行克隆。
///
/// 仅负责 Git 具体调用与取消协作（通过 `should_interrupt` 原子标志），
/// 不处理任务状态或事件发射，这部分交由上层 TaskRegistry 负责。
/// 进度回调：phase, percent, objects, bytes, total_hint
pub type ProgressCb<'a> = dyn FnMut(&str, u32, Option<u64>, Option<u64>, Option<u64>) + 'a;

/// 带进度回调的阻塞克隆。
pub fn clone_blocking_with_progress<'a>(
    repo: &str,
    dest: &Path,
    should_interrupt: &std::sync::atomic::AtomicBool,
    mut on_progress: Box<ProgressCb<'a>>,
) -> Result<(), String> {
    // 粗略判定：像路径则走 Path 分支，避免 Windows 下 "C:/..." 被当作 scheme 解析
    let looks_like_path = {
        let p = Path::new(repo);
        let bytes = repo.as_bytes();
        let win_drive = bytes.len() >= 3 && bytes[1] == b':' && (bytes[2] == b'/' || bytes[2] == b'\\');
        p.is_absolute()
            || win_drive
            || repo.starts_with("./")
            || repo.starts_with("../")
            || repo.contains('\\')
    };

    if looks_like_path {
        let repo_path: PathBuf = PathBuf::from(repo);
        let mut prep = gix::prepare_clone(repo_path.as_path(), dest)
            .map_err(|e| format!("prepare_clone(path): {}", e))?;
        // 进入 Negotiating 阶段
        on_progress("Negotiating", 0, None, None, None);
        let (mut checkout, _out) = prep
            .fetch_then_checkout(gix::progress::Discard, should_interrupt)
            .map_err(|e| format!("fetch_then_checkout(path): {}", e))?;
        // 进入 Checkout 阶段前的 Receiving
        on_progress("Receiving", 10, None, None, None);
        checkout
            .main_worktree(gix::progress::Discard, should_interrupt)
            .map_err(|e| format!("main_worktree(path): {}", e))
            .ok();
        // 检出阶段完成
        on_progress("Checkout", 90, None, None, None);
        Ok(())
    } else {
        let mut prep = gix::prepare_clone(repo, dest)
            .map_err(|e| format!("prepare_clone(url): {}", e))?;
        on_progress("Negotiating", 0, None, None, None);
        let (mut checkout, _out) = prep
            .fetch_then_checkout(gix::progress::Discard, should_interrupt)
            .map_err(|e| format!("fetch_then_checkout(url): {}", e))?;
        on_progress("Receiving", 10, None, None, None);
        checkout
            .main_worktree(gix::progress::Discard, should_interrupt)
            .map_err(|e| format!("main_worktree(url): {}", e))
            .ok();
        on_progress("Checkout", 90, None, None, None);
        Ok(())
    }
}

/// 兼容旧签名：无进度回调
pub fn clone_blocking(
    repo: &str,
    dest: &Path,
    should_interrupt: &std::sync::atomic::AtomicBool,
) -> Result<(), String> {
    clone_blocking_with_progress(repo, dest, should_interrupt, Box::new(|_, _, _, _, _| {}))
}
