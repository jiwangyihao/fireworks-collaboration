use std::{
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};

use super::super::{
    errors::{ErrorCategory, GitError},
    service::ProgressPayload,
};

/// Reset current branch to a specific reference (for pull/sync operations).
/// This performs a hard reset, updating HEAD, index, and working tree.
///
/// # Parameters
/// - `dest`: Repository path
/// - `reference`: Target reference (can be branch name like "main", remote tracking
///   branch like "origin/main", or full ref like "refs/remotes/origin/main")
/// - `hard`: If true, perform a hard reset (update working tree). If false, perform
///   a soft reset (only update HEAD).
pub fn git_reset<F: FnMut(ProgressPayload)>(
    dest: &Path,
    reference: &str,
    hard: bool,
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

    let reference_trimmed = reference.trim();
    if reference_trimmed.is_empty() {
        return Err(GitError::new(
            ErrorCategory::Protocol,
            "reference cannot be empty",
        ));
    }

    let repo = git2::Repository::open(dest).map_err(|e| {
        GitError::new(
            ErrorCategory::Internal,
            format!("open repo: {}", e.message()),
        )
    })?;

    if should_interrupt.load(Ordering::Relaxed) {
        return Err(GitError::new(ErrorCategory::Cancel, "user canceled"));
    }

    on_progress(ProgressPayload {
        task_id: uuid::Uuid::nil(),
        kind: "GitReset".into(),
        phase: "Resolving".into(),
        percent: 10,
        objects: None,
        bytes: None,
        total_hint: None,
    });

    // Try to resolve the reference to a commit
    let target_oid = resolve_reference_to_oid(&repo, reference_trimmed)?;

    if should_interrupt.load(Ordering::Relaxed) {
        return Err(GitError::new(ErrorCategory::Cancel, "user canceled"));
    }

    let target_commit = repo.find_commit(target_oid).map_err(|e| {
        GitError::new(
            ErrorCategory::Internal,
            format!("find commit: {}", e.message()),
        )
    })?;

    on_progress(ProgressPayload {
        task_id: uuid::Uuid::nil(),
        kind: "GitReset".into(),
        phase: "Resetting".into(),
        percent: 50,
        objects: None,
        bytes: None,
        total_hint: None,
    });

    // Perform the reset
    let reset_type = if hard {
        git2::ResetType::Hard
    } else {
        git2::ResetType::Soft
    };

    repo.reset(target_commit.as_object(), reset_type, None)
        .map_err(|e| {
            GitError::new(
                ErrorCategory::Internal,
                format!("reset failed: {}", e.message()),
            )
        })?;

    on_progress(ProgressPayload {
        task_id: uuid::Uuid::nil(),
        kind: "GitReset".into(),
        phase: "Completed".into(),
        percent: 100,
        objects: None,
        bytes: None,
        total_hint: None,
    });

    Ok(())
}

/// Resolve a reference string to an OID.
/// Supports:
/// - Full refs like "refs/remotes/origin/main"
/// - Short remote tracking refs like "origin/main"
/// - Local branch names like "main"
/// - Commit hashes
fn resolve_reference_to_oid(
    repo: &git2::Repository,
    reference: &str,
) -> Result<git2::Oid, GitError> {
    // Try as full reference first
    if let Ok(r) = repo.find_reference(reference) {
        if let Some(oid) = r.target() {
            return Ok(oid);
        }
        // Might be a symbolic reference, resolve it
        if let Ok(resolved) = r.resolve() {
            if let Some(oid) = resolved.target() {
                return Ok(oid);
            }
        }
    }

    // Try as refs/remotes/<reference>
    let remote_ref = format!("refs/remotes/{}", reference);
    if let Ok(r) = repo.find_reference(&remote_ref) {
        if let Some(oid) = r.target() {
            return Ok(oid);
        }
    }

    // Try as refs/heads/<reference> (local branch)
    let local_ref = format!("refs/heads/{}", reference);
    if let Ok(r) = repo.find_reference(&local_ref) {
        if let Some(oid) = r.target() {
            return Ok(oid);
        }
    }

    // Try as a commit hash (full or abbreviated)
    if let Ok(oid) = git2::Oid::from_str(reference) {
        if repo.find_commit(oid).is_ok() {
            return Ok(oid);
        }
    }

    // Try revparse as last resort
    if let Ok(obj) = repo.revparse_single(reference) {
        if let Some(commit) = obj.as_commit() {
            return Ok(commit.id());
        }
        // If it's a tag or something else, peel to commit
        if let Ok(commit) = obj.peel_to_commit() {
            return Ok(commit.id());
        }
    }

    Err(GitError::new(
        ErrorCategory::Protocol,
        format!("cannot resolve reference: {}", reference),
    ))
}
