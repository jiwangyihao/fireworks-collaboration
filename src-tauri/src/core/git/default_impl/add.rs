use std::{
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};

use super::super::{
    errors::{ErrorCategory, GitError},
    service::ProgressPayload,
};

fn normalize_component(p: &Path) -> PathBuf {
    p.components().collect()
}

pub fn git_add<F: FnMut(ProgressPayload)>(
    dest: &Path,
    paths: &[&str],
    should_interrupt: &AtomicBool,
    mut on_progress: F,
) -> Result<(), GitError> {
    if should_interrupt.load(Ordering::Relaxed) {
        return Err(GitError::new(ErrorCategory::Cancel, "user canceled"));
    }
    if !dest.join(".git").exists() {
        return Err(GitError::new(
            ErrorCategory::Protocol,
            "dest is not a git repository",
        ));
    }
    if paths.is_empty() {
        return Err(GitError::new(
            ErrorCategory::Protocol,
            "paths list is empty",
        ));
    }
    // De-duplicate keeping order
    let mut seen = std::collections::HashSet::<String>::new();
    let mut uniq: Vec<String> = Vec::new();
    for p in paths {
        let t = p.trim();
        if t.is_empty() {
            return Err(GitError::new(ErrorCategory::Protocol, "empty path entry"));
        }
        if Path::new(t).is_absolute() {
            return Err(GitError::new(
                ErrorCategory::Protocol,
                format!("absolute path not allowed: {t}"),
            ));
        }
        if seen.insert(t.to_string()) {
            uniq.push(t.to_string());
        }
    }

    let repo = git2::Repository::open(dest).map_err(|e| {
        GitError::new(
            ErrorCategory::Internal,
            format!("open repo: {}", e.message()),
        )
    })?;
    let workdir = repo
        .workdir()
        .ok_or_else(|| GitError::new(ErrorCategory::Internal, "workdir unavailable"))?;
    let workdir_canon = std::fs::canonicalize(workdir).map_err(|e| {
        GitError::new(
            ErrorCategory::Internal,
            format!("canonicalize workdir: {e}"),
        )
    })?;
    let total = uniq.len() as u32;
    let mut index = repo.index().map_err(|e| {
        GitError::new(
            ErrorCategory::Internal,
            format!("open index: {}", e.message()),
        )
    })?;
    for (i, raw) in uniq.iter().enumerate() {
        if should_interrupt.load(Ordering::Relaxed) {
            return Err(GitError::new(ErrorCategory::Cancel, "user canceled"));
        }
        let p_abs = normalize_component(&workdir.join(raw));
        if !p_abs.exists() {
            return Err(GitError::new(
                ErrorCategory::Protocol,
                format!("path does not exist: {raw}"),
            ));
        }
        let p_canon = std::fs::canonicalize(&p_abs).map_err(|e| {
            GitError::new(ErrorCategory::Internal, format!("canonicalize path: {e}"))
        })?;
        if !p_canon.starts_with(&workdir_canon) {
            return Err(GitError::new(
                ErrorCategory::Protocol,
                format!("path outside workdir: {raw}"),
            ));
        }
        let rel = match p_canon.strip_prefix(&workdir_canon) {
            Ok(r) => r,
            Err(_) => {
                return Err(GitError::new(
                    ErrorCategory::Protocol,
                    "path outside workdir",
                ))
            }
        };
        if p_canon.is_dir() {
            index
                .add_all(
                    [rel.to_string_lossy().as_ref()].iter(),
                    git2::IndexAddOption::DEFAULT,
                    None,
                )
                .map_err(|e| {
                    GitError::new(
                        ErrorCategory::Internal,
                        format!("add directory: {}", e.message()),
                    )
                })?;
        } else {
            index.add_path(rel).map_err(|e| {
                GitError::new(
                    ErrorCategory::Internal,
                    format!("add file: {}", e.message()),
                )
            })?;
        }
        let percent = (((i as f64 + 1.0) / total as f64) * 100.0) as u32;
        on_progress(ProgressPayload {
            task_id: uuid::Uuid::nil(),
            kind: "GitAdd".into(),
            phase: format!("Staging {}", raw),
            percent: percent.min(100),
            objects: Some((i + 1) as u64),
            bytes: None,
            total_hint: Some(total as u64),
        });
    }
    index.write().map_err(|e| {
        GitError::new(
            ErrorCategory::Internal,
            format!("write index: {}", e.message()),
        )
    })?;
    on_progress(ProgressPayload {
        task_id: uuid::Uuid::nil(),
        kind: "GitAdd".into(),
        phase: "Staged".into(),
        percent: 100,
        objects: Some(total as u64),
        bytes: None,
        total_hint: Some(total as u64),
    });
    Ok(())
}
