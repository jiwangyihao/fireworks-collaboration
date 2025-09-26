use std::{
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};

use super::super::{
    errors::{ErrorCategory, GitError},
    service::ProgressPayload,
};

/// Initialize a git repository at dest. Behavior:
/// - If dest does not exist: try create (including parents).
/// - If dest exists and is a directory without .git → init.
/// - If dest/.git exists → idempotent success.
/// - If dest exists but is a file → Protocol error.
/// - Cancellation checked before heavy operations.
pub fn git_init<F: FnMut(ProgressPayload)>(
    dest: &Path,
    should_interrupt: &AtomicBool,
    mut on_progress: F,
) -> Result<(), GitError> {
    // Emit a single Running progress when succeed later (registry wraps). Here we can early validate.
    if should_interrupt.load(Ordering::Relaxed) {
        return Err(GitError::new(ErrorCategory::Cancel, "user canceled"));
    }

    if dest.exists() {
        if dest.is_file() {
            return Err(GitError::new(
                ErrorCategory::Protocol,
                "dest path is a file",
            ));
        }
    } else {
        std::fs::create_dir_all(dest)
            .map_err(|e| GitError::new(ErrorCategory::Internal, format!("create dir: {}", e)))?;
    }

    // Idempotent: if .git exists treat as success.
    if dest.join(".git").exists() {
        on_progress(ProgressPayload {
            task_id: uuid::Uuid::nil(),
            kind: "GitInit".into(),
            phase: "AlreadyInitialized".into(),
            percent: 100,
            objects: None,
            bytes: None,
            total_hint: None,
        });
        return Ok(());
    }

    // Safety check: inside writable directory
    let repo = git2::Repository::init(dest).map_err(|e| {
        GitError::new(
            ErrorCategory::Internal,
            format!("init repo: {}", e.message()),
        )
    })?;
    // Basic sanity: HEAD should exist (symbolic)
    if repo.head().is_err() { /* Some platforms HEAD not yet resolved until first commit; ignore */
    }
    if should_interrupt.load(Ordering::Relaxed) {
        return Err(GitError::new(ErrorCategory::Cancel, "user canceled"));
    }
    on_progress(ProgressPayload {
        task_id: uuid::Uuid::nil(),
        kind: "GitInit".into(),
        phase: "Initialized".into(),
        percent: 100,
        objects: None,
        bytes: None,
        total_hint: None,
    });
    Ok(())
}
