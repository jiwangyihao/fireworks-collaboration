use std::{path::Path, sync::atomic::AtomicBool};

use super::super::{errors::{GitError, ErrorCategory}, service::ProgressPayload};

pub fn git_branch<F: FnMut(ProgressPayload)>(
    _dest: &Path,
    _name: &str,
    _checkout: bool,
    _force: bool,
    _should_interrupt: &AtomicBool,
    _on_progress: F,
) -> Result<(), GitError> {
    Err(GitError::new(ErrorCategory::Protocol, "git_branch: not implemented in P2.0"))
}
