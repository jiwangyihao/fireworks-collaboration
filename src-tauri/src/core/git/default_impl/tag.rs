use std::{path::Path, sync::atomic::AtomicBool};

use super::super::{errors::{GitError, ErrorCategory}, service::ProgressPayload};

pub fn git_tag<F: FnMut(ProgressPayload)>(
    _dest: &Path,
    _name: &str,
    _message: Option<&str>,
    _annotated: bool,
    _force: bool,
    _should_interrupt: &AtomicBool,
    _on_progress: F,
) -> Result<(), GitError> {
    Err(GitError::new(ErrorCategory::Protocol, "git_tag: not implemented in P2.0"))
}
