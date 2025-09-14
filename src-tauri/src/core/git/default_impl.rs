use std::{path::Path, sync::{atomic::{AtomicBool, Ordering}, Arc, Mutex}};

use super::{errors::{GitError, ErrorCategory}, service::{GitService, ProgressPayload}};

/// 默认 Git 实现（基于 git2-rs），实现 clone/fetch、桥接进度、支持取消、错误分类。
pub struct DefaultGitService;

impl DefaultGitService {
    pub fn new() -> Self { Self }

    #[inline]
    fn map_checkout_percent_to_overall(p: u32) -> u32 {
        // 将 Checkout 阶段映射为 90%~100%
        90u32.saturating_add(((p.min(100) as f64) * 0.1) as u32).min(100)
    }

    #[inline]
    fn preflight_generic<F: FnMut(ProgressPayload)>(kind: &str, should_interrupt: &AtomicBool, on_progress: &mut F) -> Result<(), GitError> {
        // 起始阶段：Negotiating
        on_progress(ProgressPayload {
            task_id: uuid::Uuid::nil(),
            kind: kind.into(),
            phase: "Negotiating".into(),
            percent: 0,
            objects: None,
            bytes: None,
            total_hint: None,
        });
        if should_interrupt.load(Ordering::Relaxed) {
            return Err(GitError::new(ErrorCategory::Cancel, "user canceled"));
        }
        Ok(())
    }

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

    #[inline]
    fn percent(received: u64, total: u64) -> u32 {
        if total > 0 { ((received as f64 / total as f64) * 100.0) as u32 } else { 0 }
    }

    #[inline]
    fn total_hint(total: u64) -> Option<u64> {
        if total > 0 { Some(total) } else { None }
    }

    #[inline]
    fn push_phase_event<F: FnMut(ProgressPayload)>(cb: &Arc<Mutex<F>>, phase: &str, percent: u32) {
        if let Ok(mut f) = cb.lock() {
            (*f)(ProgressPayload {
                task_id: uuid::Uuid::nil(),
                kind: "GitPush".into(),
                phase: phase.into(),
                percent,
                objects: None,
                bytes: None,
                total_hint: None,
            });
        }
    }
}

impl GitService for DefaultGitService {
    fn clone_blocking<F: FnMut(ProgressPayload)>(
        &self,
        repo: &str,
        dest: &Path,
        should_interrupt: &AtomicBool,
        mut on_progress: F,
    ) -> Result<(), GitError> {
        // 预发与取消检查
        Self::preflight_generic("GitClone", should_interrupt, &mut on_progress)?;

        // 共享回调（保持与原实现一致）
        let cb = Arc::new(Mutex::new(on_progress));

        // 传输进度
        let mut callbacks = git2::RemoteCallbacks::new();
        let cb_for_transfer = Arc::clone(&cb);
        callbacks.transfer_progress(move |stats| {
            if should_interrupt.load(Ordering::Relaxed) { return false; }
            let received = stats.received_objects() as u64;
            let total = stats.total_objects() as u64;
            let bytes = stats.received_bytes() as u64;
            let percent = Self::percent(received, total).min(100);
            if let Ok(mut f) = cb_for_transfer.lock() {
                (*f)(ProgressPayload {
                    task_id: uuid::Uuid::nil(),
                    kind: "GitClone".into(),
                    phase: "Receiving".into(),
                    percent,
                    objects: Some(received),
                    bytes: Some(bytes),
                    total_hint: Self::total_hint(total),
                });
            }
            true
        });

        // Fetch 选项
        let mut fo = git2::FetchOptions::new();
        fo.remote_callbacks(callbacks);
        fo.download_tags(git2::AutotagOption::Unspecified);
        fo.proxy_options(git2::ProxyOptions::new());
        fo.update_fetchhead(true);

        // Checkout 进度
        let mut co = git2::build::CheckoutBuilder::new();
        let cb_for_checkout = Arc::clone(&cb);
        co.progress(move |_p, completed, total| {
            let percent = if total > 0 { ((completed as f64 / total as f64) * 100.0) as u32 } else { 0 };
            let mapped = Self::map_checkout_percent_to_overall(percent);
            if let Ok(mut f) = cb_for_checkout.lock() {
                (*f)(ProgressPayload { task_id: uuid::Uuid::nil(), kind: "GitClone".into(), phase: "Checkout".into(), percent: mapped, objects: None, bytes: None, total_hint: None });
            }
        });

        let mut builder = git2::build::RepoBuilder::new();
        builder.fetch_options(fo);
        builder.with_checkout(co);

        match builder.clone(repo, dest) {
            Ok(_repo) => {
                // 结束事件（用于对齐前端习惯）
                if let Ok(mut f) = cb.lock() {
                    (*f)(ProgressPayload { task_id: uuid::Uuid::nil(), kind: "GitClone".into(), phase: "Completed".into(), percent: 100, objects: None, bytes: None, total_hint: None });
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

        // 预发与取消检查
        Self::preflight_generic("GitFetch", should_interrupt, &mut on_progress)?;

        // 打开仓库
        let repo = match git2::Repository::open(dest) {
            Ok(r) => r,
            Err(e) => return Err(GitError::new(Self::map_git2_error(&e), format!("open repo: {}", e))),
        };

        // 共享回调
        let cb = Arc::new(Mutex::new(on_progress));

        // Fetch 选项与回调
        let mut callbacks = git2::RemoteCallbacks::new();
        let cb_for_transfer = Arc::clone(&cb);
        callbacks.transfer_progress(move |stats| {
            if should_interrupt.load(Ordering::Relaxed) { return false; }
            let received = stats.received_objects() as u64;
            let total = stats.total_objects() as u64;
            let bytes = stats.received_bytes() as u64;
            let percent = Self::percent(received, total).min(100);
            if let Ok(mut f) = cb_for_transfer.lock() {
                (*f)(ProgressPayload { task_id: uuid::Uuid::nil(), kind: "GitFetch".into(), phase: "Receiving".into(), percent, objects: Some(received), bytes: Some(bytes), total_hint: Self::total_hint(total) });
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
                    (*f)(ProgressPayload { task_id: uuid::Uuid::nil(), kind: "GitFetch".into(), phase: "Completed".into(), percent: 100, objects: None, bytes: None, total_hint: None });
                }
                Ok(())
            }
            Err(e) => {
                let cat = Self::map_git2_error(&e);
                Err(GitError::new(cat, e.message().to_string()))
            }
        }
    }

    fn push_blocking<F: FnMut(ProgressPayload)>(
        &self,
        dest: &Path,
        remote: Option<&str>,
        refspecs: Option<&[&str]>,
        creds: Option<(&str, &str)>,
        should_interrupt: &AtomicBool,
        mut on_progress: F,
    ) -> Result<(), GitError> {
        // 校验并打开仓库
        if !dest.join(".git").exists() {
            return Err(GitError::new(ErrorCategory::Internal, "dest is not a git repository (missing .git)"));
        }

        Self::preflight_generic("GitPush", should_interrupt, &mut on_progress)?;

        let repo = match git2::Repository::open(dest) {
            Ok(r) => r,
            Err(e) => return Err(GitError::new(Self::map_git2_error(&e), format!("open repo: {}", e))),
        };

        // callbacks：凭证、传输进度、push 阶段
        let cb = Arc::new(Mutex::new(on_progress));
        let mut callbacks = git2::RemoteCallbacks::new();

        // 凭证回调（HTTPS Basic / Token）
        if creds.is_some() {
            let (user, pass) = creds.unwrap();
            callbacks.credentials(move |_url, _username_from_url, _allowed| {
                git2::Cred::userpass_plaintext(user, pass)
            });
        }

        // 传输进度（pack 上传前的协商阶段）
        let cb_for_transfer = Arc::clone(&cb);
        callbacks.transfer_progress(move |stats| {
            if should_interrupt.load(Ordering::Relaxed) { return false; }
            let received = stats.received_objects() as u64;
            let total = stats.total_objects() as u64;
            let bytes = stats.received_bytes() as u64;
            let percent = Self::percent(received, total).min(100);
            if let Ok(mut f) = cb_for_transfer.lock() {
                (*f)(ProgressPayload { task_id: uuid::Uuid::nil(), kind: "GitPush".into(), phase: "PreUpload".into(), percent, objects: Some(received), bytes: Some(bytes), total_hint: Self::total_hint(total) });
            }
            true
        });

        // push 阶段进度（libgit2 暂无 bytesSent 回调；这里仅阶段事件）
        let cb_for_phase = Arc::clone(&cb);
        callbacks.sideband_progress(move |_data| {
            // 服务器 side-band 信息，作为 Upload 阶段信号（不统计 percent）
            Self::push_phase_event(&cb_for_phase, "Upload", 50);
            true
        });

        let mut po = git2::PushOptions::new();
        po.remote_callbacks(callbacks);

        // 选择远程
        let remote_name = remote.unwrap_or("origin");
        let mut remote = match repo.find_remote(remote_name) {
            Ok(r) => r,
            Err(e) => return Err(GitError::new(Self::map_git2_error(&e), format!("find remote '{}': {}", remote_name, e))),
        };

        // 发出 PreUpload 阶段开始
        Self::push_phase_event(&cb, "PreUpload", 10);

        // 执行 push
        let specs: Vec<&str> = refspecs.map(|s| s.to_vec()).unwrap_or_else(|| Vec::new());
        let push_res = if specs.is_empty() { remote.push(&[] as &[&str], Some(&mut po)) } else { remote.push(&specs, Some(&mut po)) };

        match push_res {
            Ok(()) => {
                // 完成阶段
                Self::push_phase_event(&cb, "PostReceive", 90);
                Self::push_phase_event(&cb, "Completed", 100);
                Ok(())
            }
            Err(e) => Err(GitError::new(Self::map_git2_error(&e), e.message().to_string())),
        }
    }
}
