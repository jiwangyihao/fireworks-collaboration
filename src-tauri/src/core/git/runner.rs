use super::errors::{ErrorCategory, GitError};
use std::path::Path;
use std::process::{Command, Output};

/// Abstract interface for running Git commands.
/// This allows mocking the git CLI for testing purposes.
pub trait GitRunner: Send + Sync {
    /// Run a git command with arguments in a specific directory.
    fn run(&self, args: &[&str], cwd: &Path) -> Result<Output, GitError>;
}

/// Default implementation that runs the actual git CLI.
#[derive(Clone, Copy)]
pub struct CliGitRunner;

impl CliGitRunner {
    pub fn new() -> Self {
        Self
    }
}

impl Default for CliGitRunner {
    fn default() -> Self {
        Self::new()
    }
}

impl GitRunner for CliGitRunner {
    fn run(&self, args: &[&str], cwd: &Path) -> Result<Output, GitError> {
        let output = Command::new("git")
            .args(args)
            .current_dir(cwd)
            .output()
            .map_err(|e| {
                GitError::new(
                    ErrorCategory::Internal,
                    format!("failed to execute git command: {}", e),
                )
            })?;

        Ok(output)
    }
}
