use std::{path::Path, sync::atomic::AtomicBool};

use crate::core::git::transport::{ensure_registered, maybe_rewrite_https_to_custom};
// (helpers module handles SNI decision and config loading when needed)

use super::{errors::{GitError, ErrorCategory}, service::{GitService, ProgressPayload}};

mod helpers;
mod ops;
pub mod clone;
pub mod fetch;
pub mod push;
pub mod init;
pub mod add;
pub mod commit;
pub mod branch;
pub mod checkout;
pub mod tag;
pub mod remote;
pub mod refname;
pub mod opts; // P2.2a: depth/filter/strategyOverride parsing placeholder

pub struct DefaultGitService;

impl DefaultGitService { pub fn new() -> Self { Self } }

impl GitService for DefaultGitService {
    fn clone_blocking<F: FnMut(ProgressPayload)>(
        &self,
        repo: &str,
        dest: &Path,
        depth: Option<u32>,
        should_interrupt: &AtomicBool,
        mut on_progress: F,
    ) -> Result<(), GitError> {
        // 预发与取消检查
        helpers::preflight_generic("GitClone", should_interrupt, &mut on_progress)?;
        // 若 repo 看起来是本地路径，则进行快速存在性校验，避免阻塞在底层 clone 过程
        // 判定规则：绝对路径，或以 ./ ../ 开头，或包含反斜杠（Windows 常见），统一按路径处理
        let repo_trim = repo.trim();
        let looks_like_path = {
            let p = std::path::Path::new(repo_trim);
            p.is_absolute()
                || repo_trim.starts_with("./")
                || repo_trim.starts_with("../")
                || repo_trim.contains('\\')
        };
        if looks_like_path {
            let p = std::path::Path::new(repo_trim);
            if !p.exists() {
                return Err(GitError::new(
                    ErrorCategory::Internal,
                    format!("source path does not exist: {}", repo_trim),
                ));
            }
        }
        // 基础 URL 形态快速校验：允许 http/https 以及常见的 scp-like 语法（user@host:path），否则直接判定为无效输入
        if !looks_like_path {
            let looks_like_http = repo_trim.contains("://");
            let looks_like_scp = repo_trim.contains('@') && repo_trim.contains(':');
            if looks_like_http {
                if let Ok(parsed) = url::Url::parse(repo_trim) {
                    let scheme_ok = matches!(parsed.scheme(), "http" | "https");
                    if !scheme_ok {
                        return Err(GitError::new(ErrorCategory::Internal, format!("unsupported url scheme: {}", parsed.scheme())));
                    }
                } else {
                    return Err(GitError::new(ErrorCategory::Internal, "invalid repository url format"));
                }
            } else if !looks_like_scp {
                // 既不像本地路径，也不像 http(s) 或 scp-like，视为明显无效，快速失败
                return Err(GitError::new(ErrorCategory::Internal, "invalid repository path or url"));
            }
        }
        // SNI 状态
        helpers::emit_sni_status("GitClone", Some(repo), &mut on_progress);
        // 注册与改写
        let cfg = crate::core::config::loader::load_or_init()
            .map_err(|e| GitError::new(ErrorCategory::Internal, format!("load config: {}", e)))?;
        if let Err(e) = ensure_registered(&cfg) {
            return Err(GitError::new(ErrorCategory::Internal, format!("register custom transport: {}", e.message())));
        }
        let repo_url_final = maybe_rewrite_https_to_custom(&cfg, repo).unwrap_or_else(|| repo.to_string());
        // 若是本地路径克隆，git2/libgit2 不支持 depth 参数；忽略之以保持兼容（后续可发回退事件）。
        let effective_depth = if looks_like_path { None } else { depth };
        // Bridge to dedicated module (P2.0). Internals currently delegate to ops.rs.
        clone::do_clone(repo_url_final.as_str(), dest, effective_depth, should_interrupt, on_progress)
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
        // Bridge to dedicated module (P2.0). Internals currently delegate to ops.rs.
        fetch::do_fetch(repo_url, dest, &cfg, should_interrupt, on_progress)
    }

    fn push_blocking<F: FnMut(ProgressPayload)>(
        &self,
        dest: &Path,
        remote: Option<&str>,
        refspecs: Option<&[&str]>,
        creds: Option<(&str, &str)>,
        should_interrupt: &AtomicBool,
        on_progress: F,
    ) -> Result<(), GitError> {
        // Bridge to dedicated module (P2.0). Implementation migrated into push.rs.
        push::do_push(dest, remote, refspecs, creds, should_interrupt, on_progress)
    }
}
