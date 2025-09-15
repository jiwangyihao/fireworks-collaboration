use std::{path::Path, sync::{atomic::{AtomicBool, Ordering}, Arc, Mutex}};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;

use crate::core::git::transport::{ensure_registered, maybe_rewrite_https_to_custom, set_push_auth_header_value};
// (helpers module handles SNI decision and config loading when needed)

use super::{errors::{GitError, ErrorCategory}, service::{GitService, ProgressPayload}};

mod helpers;
mod ops;

pub struct DefaultGitService;

impl DefaultGitService { pub fn new() -> Self { Self } }

impl GitService for DefaultGitService {
    fn clone_blocking<F: FnMut(ProgressPayload)>(
        &self,
        repo: &str,
        dest: &Path,
        should_interrupt: &AtomicBool,
        mut on_progress: F,
    ) -> Result<(), GitError> {
        // 预发与取消检查
        helpers::preflight_generic("GitClone", should_interrupt, &mut on_progress)?;
        // SNI 状态
        helpers::emit_sni_status("GitClone", Some(repo), &mut on_progress);
        // 注册与改写
        let cfg = crate::core::config::loader::load_or_init()
            .map_err(|e| GitError::new(ErrorCategory::Internal, format!("load config: {}", e)))?;
        if let Err(e) = ensure_registered(&cfg) {
            return Err(GitError::new(ErrorCategory::Internal, format!("register custom transport: {}", e.message())));
        }
        let repo_url_final = maybe_rewrite_https_to_custom(&cfg, repo).unwrap_or_else(|| repo.to_string());
        ops::do_clone(repo_url_final.as_str(), dest, should_interrupt, on_progress)
    }

    fn fetch_blocking<F: FnMut(ProgressPayload)>(
        &self,
        repo_url: &str,
        dest: &Path,
        should_interrupt: &AtomicBool,
        mut on_progress: F,
    ) -> Result<(), GitError> {
        // 校验目标路径
        if !dest.join(".git").exists() {
            return Err(GitError::new(ErrorCategory::Internal, "dest is not a git repository (missing .git)"));
        }
        // 预发与取消检查
        helpers::preflight_generic("GitFetch", should_interrupt, &mut on_progress)?;
        // SNI 状态
        let repo_url_trimmed = repo_url.trim();
        if !repo_url_trimmed.is_empty() { helpers::emit_sni_status("GitFetch", Some(repo_url_trimmed), &mut on_progress); }
        else { helpers::emit_sni_status("GitFetch", None, &mut on_progress); }
        // 注册与改写准备
        let cfg = crate::core::config::loader::load_or_init()
            .map_err(|e| GitError::new(ErrorCategory::Internal, format!("load config: {}", e)))?;
        if let Err(e) = ensure_registered(&cfg) {
            return Err(GitError::new(ErrorCategory::Internal, format!("register custom transport: {}", e.message())));
        }
        ops::do_fetch(repo_url, dest, &cfg, should_interrupt, on_progress)
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
        if !dest.join(".git").exists() {
            return Err(GitError::new(ErrorCategory::Internal, "dest is not a git repository (missing .git)"));
        }
        helpers::preflight_generic("GitPush", should_interrupt, &mut on_progress)?;

        let repo = match git2::Repository::open(dest) {
            Ok(r) => r,
            Err(e) => return Err(GitError::new(helpers::map_git2_error(&e), format!("open repo: {}", e))),
        };

        let cfg = crate::core::config::loader::load_or_init()
            .map_err(|e| GitError::new(ErrorCategory::Internal, format!("load config: {}", e)))?;
        if let Err(e) = ensure_registered(&cfg) {
            return Err(GitError::new(ErrorCategory::Internal, format!("register custom transport: {}", e.message())));
        }

        let cb = Arc::new(Mutex::new(on_progress));
        let mut callbacks = git2::RemoteCallbacks::new();
        if let Some((user, pass)) = creds { let (u, p) = (user.to_string(), pass.to_string()); callbacks.credentials(move |_url,_u,_a| git2::Cred::userpass_plaintext(&u,&p)); }

        // 传输进度（协商）
        let cb_for_transfer = Arc::clone(&cb);
        callbacks.transfer_progress(move |stats| {
            if should_interrupt.load(Ordering::Relaxed) { return false; }
            let received = stats.received_objects() as u64;
            let total = stats.total_objects() as u64;
            let bytes = stats.received_bytes() as u64;
            let percent = helpers::percent(received, total).min(100);
            if let Ok(mut f) = cb_for_transfer.lock() { (*f)(ProgressPayload { task_id: uuid::Uuid::nil(), kind: "GitPush".into(), phase: "PreUpload".into(), percent, objects: Some(received), bytes: Some(bytes), total_hint: helpers::total_hint(total) }); }
            true
        });
        // 阶段事件
        let cb_for_phase = Arc::clone(&cb);
        callbacks.sideband_progress(move |_data| { helpers::push_phase_event(&cb_for_phase, "Upload", 50); true });

        let mut po = git2::PushOptions::new();
        po.remote_callbacks(callbacks);

        // 选择远程并发出 SNI 状态
        let remote_name = remote.unwrap_or("origin");
        let mut remote = match repo.find_remote(remote_name) {
            Ok(r) => {
                if let Some(u) = r.url() { helpers::emit_sni_status("GitPush", Some(u), &mut |p| { if let Ok(mut f) = cb.lock() { (*f)(p); } }); }
                else { helpers::emit_sni_status("GitPush", None, &mut |p| { if let Ok(mut f) = cb.lock() { (*f)(p); } }); }
                if let Some(u) = r.url() { if let Some(new_url) = maybe_rewrite_https_to_custom(&cfg, u) { match repo.remote_anonymous(&new_url) { Ok(r2)=>r2, Err(e)=> return Err(GitError::new(helpers::map_git2_error(&e), format!("remote anonymous with rewritten url: {}", e))) } } else { r } }
                else { r }
            }
            Err(e) => return Err(GitError::new(helpers::map_git2_error(&e), format!("find remote '{}': {}", remote_name, e))),
        };

        // 发出 PreUpload 开始
        helpers::push_phase_event(&cb, "PreUpload", 10);

        // 设置线程局部 Authorization 头
        if let Some((user, pass)) = creds { let token = format!("{}:{}", user, pass); let enc = BASE64.encode(token.as_bytes()); set_push_auth_header_value(Some(format!("Basic {}", enc))); } else { set_push_auth_header_value(None); }

        let specs: Vec<&str> = refspecs.map(|s| s.to_vec()).unwrap_or_else(|| Vec::new());
        let push_res = if specs.is_empty() { remote.push(&[] as &[&str], Some(&mut po)) } else { remote.push(&specs, Some(&mut po)) };
        set_push_auth_header_value(None);

        match push_res {
            Ok(()) => { helpers::push_phase_event(&cb, "PostReceive", 90); helpers::push_phase_event(&cb, "Completed", 100); Ok(()) }
            Err(e) => Err(GitError::new(helpers::map_git2_error(&e), e.message().to_string())),
        }
    }
}
