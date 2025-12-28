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
    // Enhanced parsing for "Receiving objects:  12% (123/1000), 5.23 MiB | 1.00 MiB/s"
    // And "Resolving deltas: 100% (4/4), done."
    let line = line.trim();

    let kind = "GitClone".to_string();
    let task_id = uuid::Uuid::nil();

    if line.starts_with("Receiving objects:") {
        let percent = parse_percent(line).unwrap_or(0);
        let (objects, total_hint) = parse_object_count(line);
        let bytes = parse_bytes(line);

        return Some(ProgressPayload {
            task_id,
            kind,
            phase: "Receiving".to_string(),
            percent,
            objects,
            bytes,
            total_hint,
        });
    } else if line.starts_with("Resolving deltas:") {
        let percent = parse_percent(line).unwrap_or(0);
        let (objects, total_hint) = parse_object_count(line);

        return Some(ProgressPayload {
            task_id,
            kind,
            phase: "Resolving".to_string(),
            percent,
            objects,
            bytes: None,
            total_hint,
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

fn parse_object_count(line: &str) -> (Option<u64>, Option<u64>) {
    // Parse "(123/1000)" -> (Some(123), Some(1000))
    if let Some(start) = line.find('(') {
        if let Some(end) = line[start..].find(')') {
            let content = &line[start + 1..start + end];
            if let Some(slash_pos) = content.find('/') {
                let current = content[..slash_pos].trim().parse::<u64>().ok();
                let total = content[slash_pos + 1..].trim().parse::<u64>().ok();
                return (current, total);
            }
        }
    }
    (None, None)
}

fn parse_bytes(line: &str) -> Option<u64> {
    // Parse "5.23 MiB" or "1024 KiB" from lines like:
    // "Receiving objects: 50% (500/1000), 5.23 MiB | 1.00 MiB/s"
    // Look for pattern: number + space + unit (before "|" or ",")

    // Find the bytes value (before |)
    let parts: Vec<&str> = line.split('|').collect();
    let content = parts[0];

    // Look for size pattern
    let size_pattern = content.split(',').nth(1)?;
    let trimmed = size_pattern.trim();

    // Parse "5.23 MiB" format
    let tokens: Vec<&str> = trimmed.split_whitespace().collect();
    if tokens.len() >= 2 {
        if let Ok(value) = tokens[0].parse::<f64>() {
            let unit = tokens[1];
            let multiplier = match unit {
                "B" => 1,
                "KiB" => 1024,
                "MiB" => 1024 * 1024,
                "GiB" => 1024 * 1024 * 1024,
                _ => return None,
            };
            return Some((value * multiplier as f64) as u64);
        }
    }
    None
}
