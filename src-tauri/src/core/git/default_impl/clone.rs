use std::{path::Path, sync::atomic::AtomicBool};

use super::super::{errors::GitError, service::ProgressPayload};

// P2.0 bridge: delegate to legacy ops.rs implementation to preserve behavior
pub fn do_clone<F: FnMut(ProgressPayload)>(
    repo_url_final: &str,
    dest: &Path,
    should_interrupt: &AtomicBool,
    on_progress: F,
) -> Result<(), GitError> {
    super::ops::do_clone(repo_url_final, dest, should_interrupt, on_progress)
}
