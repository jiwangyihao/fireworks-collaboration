use std::{path::Path, sync::atomic::AtomicBool};

use super::super::errors::GitError;
use super::super::service::ProgressPayload;

// P2.0 bridge: delegate to legacy ops.rs implementation to preserve behavior
pub fn do_fetch<F: FnMut(ProgressPayload)>(
    repo_url: &str,
    dest: &Path,
    depth: Option<u32>,
    cfg: &crate::core::config::model::AppConfig,
    should_interrupt: &AtomicBool,
    on_progress: F,
) -> Result<(), GitError> {
    super::ops::do_fetch(repo_url, dest, depth, cfg, should_interrupt, on_progress)
}
