use std::{path::Path, sync::{atomic::{AtomicBool, Ordering}, Arc, Mutex}};

use super::{errors::{GitError, ErrorCategory}, service::{GitService, ProgressPayload}};
use crate::core::config::loader::load_or_init;
use crate::core::tls::util::{decide_sni_host_with_proxy, proxy_present};
use crate::core::git::transport::{ensure_registered, maybe_rewrite_https_to_custom};
use url::Url;

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

    #[inline]
    fn emit_sni_status<F: FnMut(ProgressPayload)>(
        kind: &str,
        maybe_repo_url: Option<&str>,
        on_progress: &mut F,
    ) {
        // 尝试加载配置并推断一次初始 SNI 选择（不包含 403 轮换后的结果，仅供观测）
        let cfg = match load_or_init() { Ok(c) => c, Err(_) => return };
        let host_opt = maybe_repo_url
            .and_then(|u| Url::parse(u).ok())
            .and_then(|u| u.host_str().map(|s| s.to_string()));
        let proxy = proxy_present();
        let (sni, used_fake) = match host_opt.as_deref() {
            Some(h) => decide_sni_host_with_proxy(&cfg, false, h, proxy),
            None => decide_sni_host_with_proxy(&cfg, false, "github.com", proxy), // host unknown: pick by policy, value not used for connection here
        };
        let phase = match host_opt {
            Some(h) => format!(
                "UsingSNI host={} sni={} fake={} rotate403={} proxy={}",
                h, sni, used_fake, cfg.http.sni_rotate_on_403, proxy
            ),
            None => format!(
                "UsingSNI sni={} fake={} rotate403={} proxy={} (host=unknown)",
                sni, used_fake, cfg.http.sni_rotate_on_403, proxy
            ),
        };
        (*on_progress)(ProgressPayload {
            task_id: uuid::Uuid::nil(),
            kind: kind.into(),
            phase,
            percent: 0,
            objects: None,
            bytes: None,
            total_hint: None,
        });
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

        // 记录一次 SNI 使用状况（基于当前配置与 repo URL 预估）
        Self::emit_sni_status("GitClone", Some(repo), &mut on_progress);

        // 注册自定义传输并尝试进行 URL 灰度改写
        let cfg = crate::core::config::loader::load_or_init()
            .map_err(|e| GitError::new(ErrorCategory::Internal, format!("load config: {}", e)))?;
        if let Err(e) = ensure_registered(&cfg) {
            return Err(GitError::new(ErrorCategory::Internal, format!("register custom transport: {}", e.message())));
        }
        let repo_url_final = maybe_rewrite_https_to_custom(&cfg, repo).unwrap_or_else(|| repo.to_string());

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

        match builder.clone(&repo_url_final, dest) {
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

        // 记录一次 SNI 使用状况（若提供了 repo_url 则尝试基于其预估）
        let repo_url_trimmed = repo_url.trim();
        if !repo_url_trimmed.is_empty() {
            Self::emit_sni_status("GitFetch", Some(repo_url_trimmed), &mut on_progress);
        } else {
            // 未提供 URL：不做主机推断，展示总体 SNI 策略
            Self::emit_sni_status("GitFetch", None, &mut on_progress);
        }

        // 注册自定义传输（幂等）并准备可能的 URL 改写
        let cfg = crate::core::config::loader::load_or_init()
            .map_err(|e| GitError::new(ErrorCategory::Internal, format!("load config: {}", e)))?;
        if let Err(e) = ensure_registered(&cfg) {
            return Err(GitError::new(ErrorCategory::Internal, format!("register custom transport: {}", e.message())));
        }

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
        // 选择远程：支持对 URL 进行 https+custom 灰度改写，必要时以匿名远程执行 fetch
        let mut remote = if repo_url_trimmed.is_empty() {
            // 未指定 URL：先找命名远程；若其 URL 可改写，则改用匿名远程以改写后的 URL
            let named = match repo.find_remote("origin") {
                Ok(r) => Some(r),
                Err(_) => {
                    // 取第一个远程
                    match repo.remotes() {
                        Ok(names) => {
                            if let Some(name) = names.iter().flatten().next() {
                                match repo.find_remote(name) { Ok(r) => Some(r), Err(_) => None }
                            } else { None }
                        }
                        Err(_) => None,
                    }
                }
            };
            if let Some(r) = named {
                let url_opt = r.url().map(|s| s.to_string());
                if let Some(u) = url_opt {
                    if let Some(new_url) = maybe_rewrite_https_to_custom(&cfg, u.as_str()) {
                        match repo.remote_anonymous(&new_url) {
                            Ok(r2) => r2,
                            Err(e) => return Err(GitError::new(Self::map_git2_error(&e), format!("remote anonymous with rewritten url: {}", e))),
                        }
                    } else {
                        r
                    }
                } else {
                    r
                }
            } else {
                return Err(GitError::new(ErrorCategory::Internal, "no remote configured"));
            }
        } else {
            // 指定了 name 或 URL：若是 name，优先找到远程；可改写时转为匿名远程
            match repo.find_remote(repo_url_trimmed) {
                Ok(r) => {
                    let url_opt = r.url().map(|s| s.to_string());
                    if let Some(u) = url_opt {
                        if let Some(new_url) = maybe_rewrite_https_to_custom(&cfg, u.as_str()) {
                            match repo.remote_anonymous(&new_url) { Ok(r2) => r2, Err(e) => return Err(GitError::new(Self::map_git2_error(&e), format!("remote anonymous with rewritten url: {}", e))) }
                        } else { r }
                    } else { r }
                }
                Err(_) => {
                    // 作为 URL 使用匿名远程（先尝试改写）
                    let final_url = maybe_rewrite_https_to_custom(&cfg, repo_url_trimmed).unwrap_or_else(|| repo_url_trimmed.to_string());
                    match repo.remote_anonymous(&final_url) { Ok(r) => r, Err(e) => return Err(GitError::new(Self::map_git2_error(&e), format!("remote lookup: {}", e))) }
                }
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
