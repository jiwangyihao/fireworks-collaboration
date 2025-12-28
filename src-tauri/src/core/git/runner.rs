use super::errors::{ErrorCategory, GitError};
use std::path::Path;
use std::process::{Command, Output};

/// Abstract interface for running Git commands.
/// This allows mocking the git CLI for testing purposes.
pub trait GitRunner: Send + Sync {
    /// Run a git command with arguments in a specific directory.
    /// Run a git command with arguments in a specific directory.
    fn run(&self, args: &[&str], cwd: &Path) -> Result<Output, GitError>;

    /// Run a git command and stream the output (stdout and stderr) to a callback.
    /// The callback receives the line content and a boolean indicating if it's from stderr.
    fn run_with_progress(
        &self,
        args: &[&str],
        cwd: &Path,
        on_output: &mut dyn FnMut(&str, bool),
    ) -> Result<Output, GitError>;
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

    fn run_with_progress(
        &self,
        args: &[&str],
        cwd: &Path,
        on_output: &mut dyn FnMut(&str, bool),
    ) -> Result<Output, GitError> {
        use std::io::{BufRead, BufReader};
        use std::process::Stdio;
        use std::thread;

        let mut child = Command::new("git")
            .args(args)
            .current_dir(cwd)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| {
                GitError::new(
                    ErrorCategory::Internal,
                    format!("failed to spawn git command: {}", e),
                )
            })?;

        let stdout = child.stdout.take().unwrap();
        let stderr = child.stderr.take().unwrap();

        // simple synchronous read for now (since this function is blocking/sync per trait)
        // or usage of threads to read both streams concurrently to avoid deadlock?
        // Git often writes to stderr for progress.
        // We really should read them concurrently.

        // Shared callback is tricky with threads because `FnMut` is not `Sync`.
        // We can use channels to send lines back to the main thread.
        let (tx, rx) = std::sync::mpsc::channel();
        let tx_err = tx.clone();

        let t_out = thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines() {
                if let Ok(l) = line {
                    if tx.send((l, false)).is_err() {
                        break;
                    }
                }
            }
        });

        let t_err = thread::spawn(move || {
            let reader = BufReader::new(stderr);
            for line in reader.lines() {
                if let Ok(l) = line {
                    if tx_err.send((l, true)).is_err() {
                        break;
                    }
                }
            }
        });

        // Drop tx in main thread so iteration ends when threads finish
        // Wait, tx_err was moved. tx was moved. I need to make sure I don't hold a sender.
        // Actually, I cloned tx. original tx is moved into t_out? No, I need to clone for t_out too if I want to keep one?
        // Let's re-structure:
        // tx is created.
        // tx1 = tx.clone() -> t_out
        // tx2 = tx.clone() -> t_err
        // drop(tx)

        // Receving loop
        for (line, is_stderr) in rx {
            on_output(&line, is_stderr);
        }

        let _ = t_out.join();
        let _ = t_err.join();

        let output = child.wait_with_output().map_err(|e| {
            GitError::new(
                ErrorCategory::Internal,
                format!("failed to wait on git command: {}", e),
            )
        })?;

        Ok(output)
    }
}
