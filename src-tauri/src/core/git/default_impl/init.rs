use std::{path::Path, sync::atomic::AtomicBool};

use super::super::{errors::{GitError, ErrorCategory}, service::ProgressPayload};

// Skeleton for P2.1 — currently NotImplemented placeholder
pub fn git_init<F: FnMut(ProgressPayload)>(
    _dest: &Path,
    _should_interrupt: &AtomicBool,
    _on_progress: F,
) -> Result<(), GitError> {
    Err(GitError::new(ErrorCategory::Protocol, "git_init: not implemented in P2.0"))
}
