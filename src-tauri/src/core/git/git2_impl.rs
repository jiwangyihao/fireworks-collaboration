use std::{path::Path, sync::{atomic::{AtomicBool, Ordering}, Arc, Mutex}};

use super::{errors::{GitError, ErrorCategory}, service::{GitService, ProgressPayload}};

/// git2-rs 实现（MP0.2）：实现 clone，桥接进度，支持取消，错误分类。
pub struct Git2Service;

impl Git2Service {
    pub fn new() -> Self { Self }

    fn map_git2_error(e: &git2::Error) -> ErrorCategory {
        use git2::ErrorClass as C;
        // 用户取消：transfer_progress/checkout 回调返回 false 会产生 User 错误
        if e.code() == git2::ErrorCode::User {
            return ErrorCategory::Cancel;
        }
        let msg = e.message().to_ascii_lowercase();
        if msg.contains("timed out") || msg.contains("timeout") || msg.contains("connection") || matches!(e.class(), C::Net) {
            return ErrorCategory::Network;
        }
        if msg.contains("ssl") || msg.contains("tls") {
            return ErrorCategory::Tls;
        }
        if msg.contains("certificate") || msg.contains("x509") {
            return ErrorCategory::Verify;
        }
        if msg.contains("401") || msg.contains("403") || msg.contains("auth") {
            return ErrorCategory::Auth;
        }
        if matches!(e.class(), C::Http) {
            return ErrorCategory::Protocol;
        }
        ErrorCategory::Internal
    }
}

impl GitService for Git2Service {
    fn clone_blocking<F: FnMut(ProgressPayload)>(
        &self,
        repo: &str,
        dest: &Path,
        should_interrupt: &AtomicBool,
        mut on_progress: F,
    ) -> Result<(), GitError> {
        // 立即上报 Negotiating 起始
        on_progress(ProgressPayload {
            task_id: uuid::Uuid::nil(),
            kind: "GitClone".into(),
            phase: "Negotiating".into(),
            percent: 0,
            objects: None,
            bytes: None,
            total_hint: None,
        });

        // 提前响应取消
        if should_interrupt.load(Ordering::Relaxed) {
            return Err(GitError::new(ErrorCategory::Cancel, "user canceled"));
        }

    let mut callbacks = git2::RemoteCallbacks::new();
    // 共享回调封装，便于在多个 libgit2 回调中复用并避免可变借用冲突
    let cb = Arc::new(Mutex::new(on_progress));
        // 侧带文本进度可选：目前不透出，保留调试可能
        // callbacks.sideband_progress(|_data| true);

        // 数据传输进度
        let cb_for_transfer = Arc::clone(&cb);
        callbacks.transfer_progress(move |stats| {
            // 取消检查：返回 false 以中止 libgit2 操作
            if should_interrupt.load(Ordering::Relaxed) {
                return false;
            }

            let received = stats.received_objects() as u64;
            let total = stats.total_objects() as u64;
            let bytes = stats.received_bytes() as u64;
            let percent = if total > 0 { ((received as f64 / total as f64) * 100.0) as u32 } else { 0 };

            if let Ok(mut f) = cb_for_transfer.lock() {
                (*f)(ProgressPayload {
                task_id: uuid::Uuid::nil(),
                kind: "GitClone".into(),
                phase: "Receiving".into(),
                percent: percent.min(100),
                objects: Some(received),
                bytes: Some(bytes),
                total_hint: if total > 0 { Some(total) } else { None },
                });
            }
            true
        });

    let mut fo = git2::FetchOptions::new();
        fo.remote_callbacks(callbacks);
        fo.download_tags(git2::AutotagOption::Unspecified);
    // 降低网络超时以提升取消/错误反馈速度（秒）
    fo.proxy_options(git2::ProxyOptions::new());
    fo.update_fetchhead(true);

        // Checkout 进度（通常发生在传输完成之后）
        let mut co = git2::build::CheckoutBuilder::new();
        let cb_for_checkout = Arc::clone(&cb);
        co.progress(move |_p, completed, total| {
            // checkout 阶段不支持回调中止，这里仅上报进度；取消已通过传输阶段处理
            let percent = if total > 0 { ((completed as f64 / total as f64) * 100.0) as u32 } else { 0 };
            // 将 Checkout 阶段映射为 90%~100% 的细分，以与现有 UI 习惯保持接近
            let mapped = 90u32.saturating_add((percent.min(100) as f64 * 0.1) as u32).min(100);
            if let Ok(mut f) = cb_for_checkout.lock() {
                (*f)(ProgressPayload {
                task_id: uuid::Uuid::nil(),
                kind: "GitClone".into(),
                phase: "Checkout".into(),
                percent: mapped,
                objects: None,
                bytes: None,
                total_hint: None,
                });
            }
        });

        let mut builder = git2::build::RepoBuilder::new();
        builder.fetch_options(fo);
        builder.with_checkout(co);

        match builder.clone(repo, dest) {
            Ok(_repo) => {
                // 结束事件（用于对齐前端习惯）
                if let Ok(mut f) = cb.lock() {
                    (*f)(ProgressPayload {
                        task_id: uuid::Uuid::nil(),
                        kind: "GitClone".into(),
                        phase: "Completed".into(),
                        percent: 100,
                        objects: None,
                        bytes: None,
                        total_hint: None,
                    });
                }
                Ok(())
            },
            Err(e) => {
                let cat = Self::map_git2_error(&e);
                Err(GitError::new(cat, e.message().to_string()))
            }
        }
    }

    fn fetch_blocking<F: FnMut(ProgressPayload)>(
        &self,
        _repo_url: &str,
        dest: &Path,
        _should_interrupt: &std::sync::atomic::AtomicBool,
        mut on_progress: F,
    ) -> Result<(), GitError> {
        // 仍为占位，MP0.3 实现。此处保留最小行为以保证编译通过。
        if !dest.join(".git").exists() {
            return Err(GitError::new(ErrorCategory::Internal, "dest is not a git repository (missing .git)"));
        }
        on_progress(ProgressPayload {
            task_id: uuid::Uuid::nil(),
            kind: "GitFetch".into(),
            phase: "Init".into(),
            percent: 0,
            objects: None,
            bytes: None,
            total_hint: None,
        });
        Ok(())
    }
}
