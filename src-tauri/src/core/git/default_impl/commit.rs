use std::{path::Path, sync::atomic::AtomicBool};

use super::super::{errors::{GitError, ErrorCategory}, service::ProgressPayload};

pub struct Author<'a> { pub name: Option<&'a str>, pub email: Option<&'a str> }

pub fn git_commit<F: FnMut(ProgressPayload)>(
    _dest: &Path,
    _message: &str,
    _author: Option<Author>,
    _allow_empty: bool,
    _should_interrupt: &AtomicBool,
    _on_progress: F,
) -> Result<(), GitError> {
    Err(GitError::new(ErrorCategory::Protocol, "git_commit: not implemented in P2.0"))
}
