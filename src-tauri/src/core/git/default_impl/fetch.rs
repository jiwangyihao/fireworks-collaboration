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
    // Receiving objects:  12% (123/1000), 5.23 MiB | 1.00 MiB/s
    // Resolving deltas: 100% (4/4), done.
    // From https://github.com/user/repo

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
    if let Some(start) = line.find(": ") {
        let content = &line[start + 2..];
        if let Some(end) = content.find('%') {
            return content[..end].trim().parse::<u32>().ok();
        }
    }
    None
}

fn parse_object_count(line: &str) -> (Option<u64>, Option<u64>) {
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
    let parts: Vec<&str> = line.split('|').collect();
    let content = parts[0];
    let size_pattern = content.split(',').nth(1)?;
    let trimmed = size_pattern.trim();
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
