use std::{
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};

use super::super::{
    errors::{ErrorCategory, GitError},
    service::ProgressPayload,
};
use super::refname::validate_remote_name;

fn validate_remote_url(url: &str) -> Result<(), GitError> {
    // 首先在原始字符串上检测任何空白，包含结尾换行/制表等，避免 trim 吞掉再放过
    if url.chars().any(|c| matches!(c, ' ' | '\n' | '\r' | '\t')) {
        return Err(GitError::new(
            ErrorCategory::Protocol,
            "remote url contains whitespace",
        ));
    }
    let u = url.trim();
    if u.is_empty() {
        return Err(GitError::new(
            ErrorCategory::Protocol,
            "remote url is empty",
        ));
    }
    // allow http/https schemes, scp-like (user@host:path), or local path (existing or with path separators)
    if u.contains("://") {
        if let Ok(parsed) = url::Url::parse(u) {
            if !matches!(parsed.scheme(), "http" | "https") {
                return Err(GitError::new(
                    ErrorCategory::Protocol,
                    "unsupported remote url scheme",
                ));
            }
        } else {
            return Err(GitError::new(ErrorCategory::Protocol, "invalid remote url"));
        }
    } else if u.contains('@') && u.contains(':') {
        // scp-like accepted
    } else {
        // treat as path; basic sanity: must not contain spaces
        if u.contains(' ') {
            return Err(GitError::new(
                ErrorCategory::Protocol,
                "remote path contains space",
            ));
        }
    }
    Ok(())
}

pub fn git_remote_set<F: FnMut(ProgressPayload)>(
    dest: &Path,
    name: &str,
    url: &str,
    should_interrupt: &AtomicBool,
    mut on_progress: F,
) -> Result<(), GitError> {
    if should_interrupt.load(Ordering::Relaxed) {
        return Err(GitError::new(ErrorCategory::Cancel, "user canceled"));
    }
    if !dest.join(".git").exists() {
        return Err(GitError::new(
            ErrorCategory::Protocol,
            "dest is not a git repository",
        ));
    }
    let remote_name = name.trim();
    validate_remote_name(remote_name)?;
    validate_remote_url(url)?;
    let repo = git2::Repository::open(dest).map_err(|e| {
        GitError::new(
            ErrorCategory::Internal,
            format!("open repo: {}", e.message()),
        )
    })?;
    // remote must exist
    if repo.find_remote(remote_name).is_err() {
        return Err(GitError::new(
            ErrorCategory::Protocol,
            "remote does not exist",
        ));
    }
    if should_interrupt.load(Ordering::Relaxed) {
        return Err(GitError::new(ErrorCategory::Cancel, "user canceled"));
    }
    repo.remote_set_url(remote_name, url).map_err(|e| {
        GitError::new(
            ErrorCategory::Internal,
            format!("set remote url: {}", e.message()),
        )
    })?;
    on_progress(ProgressPayload {
        task_id: uuid::Uuid::nil(),
        kind: "GitRemoteSet".into(),
        phase: "RemoteSet".into(),
        percent: 100,
        objects: None,
        bytes: None,
        total_hint: None,
    });
    Ok(())
}

pub fn git_remote_add<F: FnMut(ProgressPayload)>(
    dest: &Path,
    name: &str,
    url: &str,
    should_interrupt: &AtomicBool,
    mut on_progress: F,
) -> Result<(), GitError> {
    if should_interrupt.load(Ordering::Relaxed) {
        return Err(GitError::new(ErrorCategory::Cancel, "user canceled"));
    }
    if !dest.join(".git").exists() {
        return Err(GitError::new(
            ErrorCategory::Protocol,
            "dest is not a git repository",
        ));
    }
    let remote_name = name.trim();
    validate_remote_name(remote_name)?;
    validate_remote_url(url)?;
    let repo = git2::Repository::open(dest).map_err(|e| {
        GitError::new(
            ErrorCategory::Internal,
            format!("open repo: {}", e.message()),
        )
    })?;
    if repo.find_remote(remote_name).is_ok() {
        return Err(GitError::new(
            ErrorCategory::Protocol,
            "remote already exists",
        ));
    }
    if should_interrupt.load(Ordering::Relaxed) {
        return Err(GitError::new(ErrorCategory::Cancel, "user canceled"));
    }
    repo.remote(remote_name, url).map_err(|e| {
        GitError::new(
            ErrorCategory::Internal,
            format!("create remote: {}", e.message()),
        )
    })?;
    on_progress(ProgressPayload {
        task_id: uuid::Uuid::nil(),
        kind: "GitRemoteAdd".into(),
        phase: "RemoteAdded".into(),
        percent: 100,
        objects: None,
        bytes: None,
        total_hint: None,
    });
    Ok(())
}

pub fn git_remote_remove<F: FnMut(ProgressPayload)>(
    dest: &Path,
    name: &str,
    should_interrupt: &AtomicBool,
    mut on_progress: F,
) -> Result<(), GitError> {
    if should_interrupt.load(Ordering::Relaxed) {
        return Err(GitError::new(ErrorCategory::Cancel, "user canceled"));
    }
    if !dest.join(".git").exists() {
        return Err(GitError::new(
            ErrorCategory::Protocol,
            "dest is not a git repository",
        ));
    }
    let remote_name = name.trim();
    validate_remote_name(remote_name)?; // reuse same naming rules
    let repo = git2::Repository::open(dest).map_err(|e| {
        GitError::new(
            ErrorCategory::Internal,
            format!("open repo: {}", e.message()),
        )
    })?;
    if repo.find_remote(remote_name).is_err() {
        return Err(GitError::new(
            ErrorCategory::Protocol,
            "remote does not exist",
        ));
    }
    if should_interrupt.load(Ordering::Relaxed) {
        return Err(GitError::new(ErrorCategory::Cancel, "user canceled"));
    }
    repo.remote_delete(remote_name).map_err(|e| {
        GitError::new(
            ErrorCategory::Internal,
            format!("remove remote: {}", e.message()),
        )
    })?;
    on_progress(ProgressPayload {
        task_id: uuid::Uuid::nil(),
        kind: "GitRemoteRemove".into(),
        phase: "RemoteRemoved".into(),
        percent: 100,
        objects: None,
        bytes: None,
        total_hint: None,
    });
    Ok(())
}
