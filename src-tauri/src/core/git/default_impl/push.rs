use crate::core::git::errors::{ErrorCategory, GitError};
use crate::core::git::runner::GitRunner;
use crate::core::git::service::ProgressPayload;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

pub fn do_push<F: FnMut(ProgressPayload)>(
    runner: &dyn GitRunner,
    dest: &Path,
    remote: Option<&str>,
    refspecs: Option<&[&str]>,
    creds: Option<(&str, &str)>,
    should_interrupt: &AtomicBool,
    mut on_progress: F,
) -> Result<(), GitError> {
    if !dest.join(".git").exists() {
        return Err(GitError::new(
            ErrorCategory::Internal,
            "dest is not a git repository (missing .git)",
        ));
    }

    if should_interrupt.load(Ordering::Relaxed) {
        return Err(GitError::new(ErrorCategory::Cancel, "Push cancelled"));
    }

    let mut args = vec!["push", "--progress", "--porcelain"];

    let remote_name = remote.unwrap_or("origin");
    args.push(remote_name);

    // Add refspecs if provided, otherwise git push will use default behavior
    let refspec_strings: Vec<String>;
    if let Some(rs) = refspecs {
        refspec_strings = rs.iter().map(|s| s.to_string()).collect();
        for spec in &refspec_strings {
            args.push(spec.as_str());
        }
    }

    // For authentication, we need to use credential helper or GIT_ASKPASS
    // Since we have creds, we can set environment variables or use git credential store
    // However, git CLI needs interactive auth or credential helper setup
    // For now, we'll rely on existing git config or assume SSH keys
    // This is a limitation compared to git2's programmatic credential callback

    let progress_closure = |line: &str, is_stderr: bool| {
        if should_interrupt.load(Ordering::Relaxed) {
            // Cannot easily kill process from callback with current trait design
        }

        // Git push progress goes to stderr
        if is_stderr {
            if let Some(payload) = parse_push_progress_line(line) {
                on_progress(payload);
            }
        }
    };

    let mut cb = progress_closure;

    runner
        .run_with_progress(&args, dest, &mut cb)
        .map(|output| {
            // Check exit status for errors
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(GitError::new(
                    ErrorCategory::Network,
                    format!("git push failed: {}", stderr),
                ));
            }
            Ok(())
        })
        .and_then(|r| r)
}

fn parse_push_progress_line(line: &str) -> Option<ProgressPayload> {
    let line = line.trim();
    let kind = "GitPush".to_string();
    let task_id = uuid::Uuid::nil();

    // Push progress patterns:
    // "Enumerating objects: 5, done."
    // "Counting objects: 100% (5/5), done."
    // "Compressing objects: 100% (4/4), done."
    // "Writing objects: 100% (3/3), 256 bytes | 256.00 KiB/s, done."
    // "Total 3 (delta 0), reused 0 (delta 0), pack-reused 0"

    if line.starts_with("Enumerating objects:") {
        // "Enumerating objects: 5, done." or "Enumerating objects: 100, done."
        let count = parse_enumerating_count(line);
        return Some(ProgressPayload {
            task_id,
            kind,
            phase: "Enumerating".to_string(),
            percent: 10,
            objects: count,
            bytes: None,
            total_hint: count,
        });
    } else if line.starts_with("Counting objects:") {
        let percent = parse_percent(line).unwrap_or(20);
        let (objects, total_hint) = parse_object_count(line);

        return Some(ProgressPayload {
            task_id,
            kind,
            phase: "Counting".to_string(),
            percent: 20 + (percent / 5), // Maps 0-100% to 20-40%
            objects,
            bytes: None,
            total_hint,
        });
    } else if line.starts_with("Compressing objects:") {
        let percent = parse_percent(line).unwrap_or(40);
        let (objects, total_hint) = parse_object_count(line);

        return Some(ProgressPayload {
            task_id,
            kind,
            phase: "Compressing".to_string(),
            percent: 40 + (percent / 5), // Maps 0-100% to 40-60%
            objects,
            bytes: None,
            total_hint,
        });
    } else if line.starts_with("Writing objects:") {
        let percent = parse_percent(line).unwrap_or(60);
        let (objects, total_hint) = parse_object_count(line);
        let bytes = parse_bytes(line);

        return Some(ProgressPayload {
            task_id,
            kind,
            phase: "Writing".to_string(),
            percent: 60 + (percent / 3), // Maps 0-100% to 60-93%
            objects,
            bytes,
            total_hint,
        });
    } else if line.contains("done.") && line.contains("Total") {
        return Some(ProgressPayload {
            task_id,
            kind,
            phase: "Completed".to_string(),
            percent: 100,
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

fn parse_enumerating_count(line: &str) -> Option<u64> {
    // "Enumerating objects: 5, done." -> Some(5)
    if let Some(start) = line.find(": ") {
        let content = &line[start + 2..];
        if let Some(comma) = content.find(',') {
            return content[..comma].trim().parse::<u64>().ok();
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
    // "Writing objects: 100% (3/3), 256 bytes | 256.00 KiB/s, done."
    let parts: Vec<&str> = line.split('|').collect();
    let content = parts[0];
    let size_pattern = content.split(',').nth(1)?;
    let trimmed = size_pattern.trim();
    let tokens: Vec<&str> = trimmed.split_whitespace().collect();

    if tokens.len() >= 2 {
        if let Ok(value) = tokens[0].parse::<f64>() {
            let unit = tokens[1];
            let multiplier = match unit {
                "B" | "bytes" => 1,
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
