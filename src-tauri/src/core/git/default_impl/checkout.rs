use std::{path::Path, sync::atomic::AtomicBool};

use super::super::{errors::{GitError, ErrorCategory}, service::ProgressPayload};

pub fn git_checkout<F: FnMut(ProgressPayload)>(
    _dest: &Path,
    _reference: &str,
    _create: bool,
    _should_interrupt: &AtomicBool,
    _on_progress: F,
) -> Result<(), GitError> {
    Err(GitError::new(ErrorCategory::Protocol, "git_checkout: not implemented in P2.0"))
}
