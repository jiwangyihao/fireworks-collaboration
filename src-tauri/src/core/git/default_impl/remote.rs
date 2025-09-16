use std::{path::Path, sync::atomic::AtomicBool};

use super::super::{errors::{GitError, ErrorCategory}, service::ProgressPayload};

pub fn git_remote_set<F: FnMut(ProgressPayload)>(
    _dest: &Path,
    _name: &str,
    _url: &str,
    _should_interrupt: &AtomicBool,
    _on_progress: F,
) -> Result<(), GitError> {
    Err(GitError::new(ErrorCategory::Protocol, "git_remote_set: not implemented in P2.0"))
}

pub fn git_remote_add<F: FnMut(ProgressPayload)>(
    _dest: &Path,
    _name: &str,
    _url: &str,
    _should_interrupt: &AtomicBool,
    _on_progress: F,
) -> Result<(), GitError> {
    Err(GitError::new(ErrorCategory::Protocol, "git_remote_add: not implemented in P2.0"))
}

pub fn git_remote_remove<F: FnMut(ProgressPayload)>(
    _dest: &Path,
    _name: &str,
    _should_interrupt: &AtomicBool,
    _on_progress: F,
) -> Result<(), GitError> {
    Err(GitError::new(ErrorCategory::Protocol, "git_remote_remove: not implemented in P2.0"))
}
