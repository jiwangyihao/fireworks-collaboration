use super::errors::{ErrorCategory, GitError};
use super::service::ProgressPayload;
use std::path::Path;
use std::sync::atomic::AtomicBool;

/// Abstract interface for Git operations.
/// This allows dependency injection and mocking for testing.
///
/// Note: The actual implementation uses git2 (libgit2), not external git CLI.
pub trait GitRunner: Send + Sync {
    /// Clone a repository using git2.
    fn clone_repo(
        &self,
        url: &str,
        dest: &Path,
        depth: Option<u32>,
        should_interrupt: &AtomicBool,
        on_progress: &mut dyn FnMut(ProgressPayload),
    ) -> Result<(), GitError>;

    /// Fetch from a remote using git2.
    fn fetch_repo(
        &self,
        repo_path: &Path,
        remote_url: &str,
        depth: Option<u32>,
        should_interrupt: &AtomicBool,
        on_progress: &mut dyn FnMut(ProgressPayload),
    ) -> Result<(), GitError>;

    /// Push to a remote using git2.
    fn push_repo(
        &self,
        repo_path: &Path,
        remote: Option<&str>,
        refspecs: Option<&[&str]>,
        creds: Option<(&str, &str)>,
        should_interrupt: &AtomicBool,
        on_progress: &mut dyn FnMut(ProgressPayload),
    ) -> Result<(), GitError>;
}

/// Git2-based implementation using libgit2.
/// This is the production implementation that does NOT depend on external git CLI.
///
/// The actual git operations are implemented in:
/// - ops.rs: do_clone, do_fetch (using git2::build::RepoBuilder, git2::Remote)
/// - push.rs: do_push_internal (using git2::Remote::push)
#[derive(Clone, Copy)]
pub struct Git2Runner;

impl Git2Runner {
    pub fn new() -> Self {
        Self
    }
}

impl Default for Git2Runner {
    fn default() -> Self {
        Self::new()
    }
}

impl GitRunner for Git2Runner {
    fn clone_repo(
        &self,
        url: &str,
        dest: &Path,
        depth: Option<u32>,
        should_interrupt: &AtomicBool,
        on_progress: &mut dyn FnMut(ProgressPayload),
    ) -> Result<(), GitError> {
        // Delegate to ops::do_clone which contains the actual git2 implementation
        super::default_impl::ops::do_clone(url, dest, depth, should_interrupt, on_progress)
    }

    fn fetch_repo(
        &self,
        repo_path: &Path,
        remote_url: &str,
        depth: Option<u32>,
        should_interrupt: &AtomicBool,
        on_progress: &mut dyn FnMut(ProgressPayload),
    ) -> Result<(), GitError> {
        // Load config for transport customization
        let cfg = crate::core::config::loader::load_or_init()
            .map_err(|e| GitError::new(ErrorCategory::Internal, format!("load config: {e}")))?;

        // Delegate to ops::do_fetch which contains the actual git2 implementation
        super::default_impl::ops::do_fetch(
            remote_url,
            repo_path,
            depth,
            &cfg,
            should_interrupt,
            on_progress,
        )
    }

    fn push_repo(
        &self,
        repo_path: &Path,
        remote: Option<&str>,
        refspecs: Option<&[&str]>,
        creds: Option<(&str, &str)>,
        should_interrupt: &AtomicBool,
        on_progress: &mut dyn FnMut(ProgressPayload),
    ) -> Result<(), GitError> {
        // Delegate to push::do_push_internal which contains the actual git2 implementation
        super::default_impl::push::do_push_internal(
            repo_path,
            remote,
            refspecs,
            creds,
            should_interrupt,
            on_progress,
        )
    }
}
