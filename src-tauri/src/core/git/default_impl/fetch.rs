use crate::core::git::errors::{ErrorCategory, GitError};
use crate::core::git::runner::GitRunner;
use crate::core::git::service::ProgressPayload;
use std::path::Path;
use std::sync::atomic::AtomicBool;

pub fn do_fetch<F: FnMut(ProgressPayload)>(
    runner: &dyn GitRunner,
    repo_url: &str,
    dest: &Path,
    depth: Option<u32>,
    _cfg: &crate::core::config::model::AppConfig,
    should_interrupt: &AtomicBool,
    mut on_progress: F,
) -> Result<(), GitError> {
    let mut args = vec!["fetch", "--progress"];

    let depth_str;
    if let Some(d) = depth {
        depth_str = d.to_string();
        args.push("--depth");
        args.push(&depth_str);
    }

    if !repo_url.is_empty() {
        args.push(repo_url);
    }

    let progress_closure = |line: &str, is_stderr: bool| {
        if should_interrupt.load(std::sync::atomic::Ordering::Relaxed) {
            // Cancellation logic handled by AtomicBool check before call, but hard to interrupt mid-process without kill
        }

        if is_stderr {
            if let Some(payload) = parse_progress_line(line) {
                on_progress(payload);
            }
        }
    };

    let mut cb = progress_closure;

    if should_interrupt.load(std::sync::atomic::Ordering::Relaxed) {
        return Err(GitError::new(ErrorCategory::Cancel, "Fetch cancelled"));
    }

    runner
        .run_with_progress(&args, dest, &mut cb)
        .map(|_| ())
        .map_err(|e| e)
}

fn parse_progress_line(line: &str) -> Option<ProgressPayload> {
    let line = line.trim();
    let kind = "GitFetch".to_string();
    let task_id = uuid::Uuid::nil();

    // Fetch progress looks like:
    // Receiving objects:  12% (123/1000)
    // Resolving deltas: 100% (4/4)
    // From https://github.com/user/repo

    if line.starts_with("Receiving objects:") {
        let percent = parse_percent(line).unwrap_or(0);
        return Some(ProgressPayload {
            task_id,
            kind,
            phase: "Receiving".to_string(),
            percent,
            objects: None,
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
    if let Some(start) = line.find(": ") {
        let content = &line[start + 2..];
        if let Some(end) = content.find('%') {
            return content[..end].trim().parse::<u32>().ok();
        }
    }
    None
}
