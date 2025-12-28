use crate::core::git::errors::{ErrorCategory, GitError};
use crate::core::git::runner::GitRunner;
use crate::core::git::service::ProgressPayload;
use std::path::Path;
use std::sync::atomic::AtomicBool;

pub fn do_clone<F: FnMut(ProgressPayload)>(
    runner: &dyn GitRunner,
    repo_url: &str,
    dest: &Path,
    depth: Option<u32>,
    should_interrupt: &AtomicBool,
    mut on_progress: F,
) -> Result<(), GitError> {
    let mut args = vec!["clone", "--progress"]; // --progress forces progress even if not TTY

    let depth_str;
    if let Some(d) = depth {
        depth_str = d.to_string();
        args.push("--depth");
        args.push(&depth_str);
    }

    args.push(repo_url);

    // Convert dest path to string, handle potential unicode issues
    let dest_str = dest.to_str().ok_or_else(|| {
        GitError::new(
            ErrorCategory::Internal,
            "Invalid destination path encoding (non-UTF8)",
        )
    })?;
    args.push(dest_str);

    let progress_closure = |line: &str, is_stderr: bool| {
        if should_interrupt.load(std::sync::atomic::Ordering::Relaxed) {
            // We can't easily kill the process from here via this callback structure
            // without changing GitRunner trait to return a handle or accept a cancellation token.
            // But GitRunner::run_with_progress implementation usually waits.
            // For now, we rely on the fact that if this closure returns or sets state,
            // we can't stop the inner loop unless the trait allows returning false to stop.
            // The current trait `run_with_progress` returns Result<Output>.
            // It spawns a thread to read output.
            // If we want to cancel, we might need a way.
            // But existing code uses `AtomicBool` passed to callbacks.
            // The trait I added `on_output: &mut dyn FnMut(&str, bool)` does not return control.
        }

        // Git progress typically goes to stderr
        if is_stderr {
            if let Some(payload) = parse_progress_line(line) {
                on_progress(payload);
            }
        }
    };

    // Need minimal wrapper to match types
    let mut cb = progress_closure;

    // We check interrupt before starting
    if should_interrupt.load(std::sync::atomic::Ordering::Relaxed) {
        return Err(GitError::new(ErrorCategory::Cancel, "Clone cancelled"));
    }

    runner
        .run_with_progress(&args, Path::new("."), &mut cb)
        .map(|_| ())
        .map_err(|e| e)
}

fn parse_progress_line(line: &str) -> Option<ProgressPayload> {
    // Basic parsing for "Receiving objects:  12% (123/1000)"
    // And "Resolving deltas: 100% (4/4)"
    let line = line.trim();

    let kind = "GitClone".to_string();
    let task_id = uuid::Uuid::nil();

    if line.starts_with("Receiving objects:") {
        let percent = parse_percent(line).unwrap_or(0);
        return Some(ProgressPayload {
            task_id,
            kind,
            phase: "Receiving".to_string(),
            percent,
            objects: None, // could parse "123/1000"
            bytes: None,
            total_hint: None,
        });
    } else if line.starts_with("Resolving deltas:") {
        let percent = parse_percent(line).unwrap_or(0);
        return Some(ProgressPayload {
            task_id,
            kind,
            phase: "Resolving".to_string(),
            percent,
            objects: None,
            bytes: None,
            total_hint: None,
        });
    }

    None
}

fn parse_percent(line: &str) -> Option<u32> {
    // Find "X%"
    if let Some(start) = line.find(": ") {
        let content = &line[start + 2..];
        if let Some(end) = content.find('%') {
            return content[..end].trim().parse::<u32>().ok();
        }
    }
    None
}
