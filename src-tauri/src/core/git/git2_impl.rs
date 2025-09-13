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
        repo_url: &str,
        dest: &Path,
        should_interrupt: &std::sync::atomic::AtomicBool,
        mut on_progress: F,
    ) -> Result<(), GitError> {
        // 校验目标路径是一个 Git 仓库
        if !dest.join(".git").exists() {
            return Err(GitError::new(ErrorCategory::Internal, "dest is not a git repository (missing .git)"));
        }

        // 进入 Negotiating 阶段
        on_progress(ProgressPayload {
            task_id: uuid::Uuid::nil(),
            kind: "GitFetch".into(),
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

        // 打开仓库
        let repo = match git2::Repository::open(dest) {
            Ok(r) => r,
            Err(e) => return Err(GitError::new(Self::map_git2_error(&e), format!("open repo: {}", e))),
        };

        // 构建进度回调
        let cb = Arc::new(Mutex::new(on_progress));
        let mut callbacks = git2::RemoteCallbacks::new();
        let cb_for_transfer = Arc::clone(&cb);
        callbacks.transfer_progress(move |stats| {
            if should_interrupt.load(Ordering::Relaxed) { return false; }
            let received = stats.received_objects() as u64;
            let total = stats.total_objects() as u64;
            let bytes = stats.received_bytes() as u64;
            let percent = if total > 0 { ((received as f64 / total as f64) * 100.0) as u32 } else { 0 };
            if let Ok(mut f) = cb_for_transfer.lock() {
                (*f)(ProgressPayload {
                    task_id: uuid::Uuid::nil(),
                    kind: "GitFetch".into(),
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
        fo.update_fetchhead(true);

        // 选择远程：优先 repo_url 指定；为空时尝试 origin；否则取第一个远程；若均不存在则使用匿名远程（需传入 URL）。
        let repo_url_trimmed = repo_url.trim();
        let mut remote = if repo_url_trimmed.is_empty() {
            match repo.find_remote("origin") {
                Ok(r) => r,
                Err(_) => {
                    // 尝试取第一个远程名
                    match repo.remotes() {
                        Ok(names) => {
                            if let Some(name) = names.iter().flatten().next() {
                                repo.find_remote(name)
                                    .map_err(|e| GitError::new(Self::map_git2_error(&e), format!("find first remote: {}", e)))?
                            } else {
                                return Err(GitError::new(ErrorCategory::Internal, "no remote configured"));
                            }
                        }
                        Err(e) => return Err(GitError::new(Self::map_git2_error(&e), format!("list remotes: {}", e))),
                    }
                }
            }
        } else {
            // 若能按名称找到则使用；否则作为 URL 使用匿名远程
            match repo.find_remote(repo_url_trimmed) {
                Ok(r) => r,
                Err(_) => match repo.remote_anonymous(repo_url_trimmed) {
                    Ok(r) => r,
                    Err(e) => return Err(GitError::new(Self::map_git2_error(&e), format!("remote lookup: {}", e))),
                },
            }
        };

        // 执行 fetch（使用远程自身配置的 refspecs）
        match remote.fetch(&[] as &[&str], Some(&mut fo), None) {
            Ok(_) => {
                if let Ok(mut f) = cb.lock() {
                    (*f)(ProgressPayload {
                        task_id: uuid::Uuid::nil(),
                        kind: "GitFetch".into(),
                        phase: "Completed".into(),
                        percent: 100,
                        objects: None,
                        bytes: None,
                        total_hint: None,
                    });
                }
                Ok(())
            }
            Err(e) => {
                let cat = Self::map_git2_error(&e);
                Err(GitError::new(cat, e.message().to_string()))
            }
        }
    }
}
