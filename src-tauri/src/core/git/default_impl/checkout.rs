use std::{path::Path, sync::atomic::{AtomicBool, Ordering}};

use super::super::{errors::{GitError, ErrorCategory}, service::ProgressPayload};

/// Checkout an existing branch or create (if flag set) then checkout.
/// Rules:
/// - dest must be repo; reference trimmed non-empty
/// - if branch exists: set HEAD and checkout (worktree update)
/// - if not exists and create=true and HEAD has commit: create branch at HEAD then checkout
/// - if not exists and create=false -> Protocol
/// - For simplicity we only support branch names (no commit hash / tags) in P2.1c
pub fn git_checkout<F: FnMut(ProgressPayload)>(
    dest: &Path,
    reference: &str,
    create: bool,
    should_interrupt: &AtomicBool,
    mut on_progress: F,
) -> Result<(), GitError> {
    if should_interrupt.load(Ordering::Relaxed) { return Err(GitError::new(ErrorCategory::Cancel, "user canceled")); }
    if !dest.join(".git").exists() { return Err(GitError::new(ErrorCategory::Protocol, "dest is not a git repository")); }
    let name = reference.trim();
    if name.is_empty() { return Err(GitError::new(ErrorCategory::Protocol, "reference is empty")); }
    if name.contains(' ') { return Err(GitError::new(ErrorCategory::Protocol, "reference contains space")); }
    let repo = git2::Repository::open(dest).map_err(|e| GitError::new(ErrorCategory::Internal, format!("open repo: {}", e.message())))?;
    let existing = repo.find_branch(name, git2::BranchType::Local).ok();
    if let Some(_br) = existing {
        // just checkout
        if should_interrupt.load(Ordering::Relaxed) { return Err(GitError::new(ErrorCategory::Cancel, "user canceled")); }
        let mut co = git2::build::CheckoutBuilder::new(); co.safe();
        if should_interrupt.load(Ordering::Relaxed) { return Err(GitError::new(ErrorCategory::Cancel, "user canceled")); }
        repo.set_head(&format!("refs/heads/{}", name)).map_err(|e| GitError::new(ErrorCategory::Internal, format!("set head: {}", e.message())))?;
        if should_interrupt.load(Ordering::Relaxed) { return Err(GitError::new(ErrorCategory::Cancel, "user canceled")); }
        repo.checkout_head(Some(&mut co)).map_err(|e| GitError::new(ErrorCategory::Internal, format!("checkout: {}", e.message())))?;
        on_progress(ProgressPayload { task_id: uuid::Uuid::nil(), kind: "GitCheckout".into(), phase: "CheckedOut".into(), percent: 100, objects: None, bytes: None, total_hint: None });
        return Ok(());
    }
    if !create { return Err(GitError::new(ErrorCategory::Protocol, "branch does not exist")); }
    // create new branch at HEAD
    let head_oid = repo.head().ok().and_then(|h| h.target()).ok_or_else(|| GitError::new(ErrorCategory::Protocol, "repository has no commits"))?;
    if should_interrupt.load(Ordering::Relaxed) { return Err(GitError::new(ErrorCategory::Cancel, "user canceled")); }
    let commit = repo.find_commit(head_oid).map_err(|e| GitError::new(ErrorCategory::Internal, format!("find commit: {}", e.message())))?;
    repo.branch(name, &commit, false).map_err(|e| GitError::new(ErrorCategory::Internal, format!("create branch: {}", e.message())))?;
    let mut co = git2::build::CheckoutBuilder::new(); co.safe();
    if should_interrupt.load(Ordering::Relaxed) { return Err(GitError::new(ErrorCategory::Cancel, "user canceled")); }
    repo.set_head(&format!("refs/heads/{}", name)).map_err(|e| GitError::new(ErrorCategory::Internal, format!("set head: {}", e.message())))?;
    if should_interrupt.load(Ordering::Relaxed) { return Err(GitError::new(ErrorCategory::Cancel, "user canceled")); }
    repo.checkout_head(Some(&mut co)).map_err(|e| GitError::new(ErrorCategory::Internal, format!("checkout: {}", e.message())))?;
    on_progress(ProgressPayload { task_id: uuid::Uuid::nil(), kind: "GitCheckout".into(), phase: "CreatedAndCheckedOut".into(), percent: 100, objects: None, bytes: None, total_hint: None });
    Ok(())
}
