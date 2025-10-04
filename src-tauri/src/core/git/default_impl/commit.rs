use std::{
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};

use super::super::{
    errors::{ErrorCategory, GitError},
    service::ProgressPayload,
};

pub struct Author<'a> {
    pub name: Option<&'a str>,
    pub email: Option<&'a str>,
}

/// Create a commit with staged changes.
/// Rules:
/// - dest must be a git repo (dest/.git exists) -> else Protocol.
/// - message trimmed non-empty else Protocol.
/// - If no staged changes compared to HEAD and `allow_empty==false` -> Protocol (empty commit rejected).
/// - If HEAD absent (first commit) and index empty -> same empty check applies.
/// - author defaults to repository signature (from config); if provided missing name or email -> Protocol.
///
/// Cancellation checked at key points (before heavy diff / before write).
pub fn git_commit<F: FnMut(ProgressPayload)>(
    dest: &Path,
    message: &str,
    author: Option<Author>,
    allow_empty: bool,
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
    let msg = message.trim();
    if msg.is_empty() {
        return Err(GitError::new(
            ErrorCategory::Protocol,
            "commit message is empty",
        ));
    }

    let repo = git2::Repository::open(dest).map_err(|e| {
        GitError::new(
            ErrorCategory::Internal,
            format!("open repo: {}", e.message()),
        )
    })?;
    let mut index = repo.index().map_err(|e| {
        GitError::new(
            ErrorCategory::Internal,
            format!("open index: {}", e.message()),
        )
    })?;

    // Write the index to get the tree (similar to git commit semantics)
    if should_interrupt.load(Ordering::Relaxed) {
        return Err(GitError::new(ErrorCategory::Cancel, "user canceled"));
    }
    index.write().map_err(|e| {
        GitError::new(
            ErrorCategory::Internal,
            format!("write index: {}", e.message()),
        )
    })?;
    let tree_id = index.write_tree().map_err(|e| {
        GitError::new(
            ErrorCategory::Internal,
            format!("write tree: {}", e.message()),
        )
    })?;
    let tree = repo.find_tree(tree_id).map_err(|e| {
        GitError::new(
            ErrorCategory::Internal,
            format!("find tree: {}", e.message()),
        )
    })?;

    // Detect empty commit (no changes) when not allowed
    let head_commit_opt = repo
        .head()
        .ok()
        .and_then(|h| h.target())
        .and_then(|oid| repo.find_commit(oid).ok());
    if !allow_empty {
        let is_empty = match &head_commit_opt {
            Some(head_commit) => {
                // Compare tree ids
                head_commit
                    .tree()
                    .map(|t| t.id() == tree.id())
                    .unwrap_or(false)
            }
            None => index.is_empty(), // first commit case: if index empty it's empty
        };
        if is_empty {
            return Err(GitError::new(
                ErrorCategory::Protocol,
                "empty commit (no changes)",
            ));
        }
    }

    if should_interrupt.load(Ordering::Relaxed) {
        return Err(GitError::new(ErrorCategory::Cancel, "user canceled"));
    }

    // Build signature
    let sig = if let Some(a) = author {
        match (a.name, a.email) {
            (Some(n), Some(e)) if !n.trim().is_empty() && !e.trim().is_empty() => {
                git2::Signature::now(n.trim(), e.trim()).map_err(|e| {
                    GitError::new(
                        ErrorCategory::Internal,
                        format!("signature: {}", e.message()),
                    )
                })?
            }
            _ => {
                return Err(GitError::new(
                    ErrorCategory::Protocol,
                    "author name/email required when author specified",
                ))
            }
        }
    } else {
        repo.signature().map_err(|e| {
            GitError::new(
                ErrorCategory::Internal,
                format!("signature: {}", e.message()),
            )
        })?
    };

    // Parents
    let parents: Vec<git2::Commit> = head_commit_opt.into_iter().collect();
    let parent_refs: Vec<&git2::Commit> = parents.iter().collect();

    if should_interrupt.load(Ordering::Relaxed) {
        return Err(GitError::new(ErrorCategory::Cancel, "user canceled"));
    }

    let commit_id = repo
        .commit(Some("HEAD"), &sig, &sig, msg, &tree, &parent_refs)
        .map_err(|e| GitError::new(ErrorCategory::Internal, format!("commit: {}", e.message())))?;

    // Emit final progress (phase Running percent 100 for consistency)
    on_progress(ProgressPayload {
        task_id: uuid::Uuid::nil(),
        kind: "GitCommit".into(),
        phase: "Committed".into(),
        percent: 100,
        objects: None,
        bytes: None,
        total_hint: None,
    });
    tracing::debug!(target = "git", "commit created: {}", commit_id);
    Ok(())
}
