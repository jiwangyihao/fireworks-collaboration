use std::{
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};

use super::super::{
    errors::{ErrorCategory, GitError},
    service::ProgressPayload,
};
use super::refname::validate_branch_name;

/// Create (and optionally checkout) a branch.
/// Rules:
/// - dest must be a git repo; name trimmed non-empty.
/// - If branch exists:
///     - force=true => move branch ref to HEAD (current commit)
///     - force=false & checkout=true => just checkout existing branch
///     - force=false & checkout=false => Protocol (already exists)
/// - If branch not exists: create pointing to HEAD; if checkout flag set, checkout.
/// - If repository has no HEAD (no commits) and branch creation is requested -> create unborn branch (git allows creating a ref pointing to empty?)
///   We mimic git behavior: if HEAD target commit missing (initial repo) and no commit yet => Protocol unless force (still no commit to point) so we allow creating a symbolic ref using HEAD target? Simpler: require at least one commit; else Protocol.
/// - Cancellation at early points.
pub fn git_branch<F: FnMut(ProgressPayload)>(
    dest: &Path,
    name: &str,
    checkout: bool,
    force: bool,
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
    let branch_name = name.trim();
    validate_branch_name(branch_name)?;

    let repo = git2::Repository::open(dest).map_err(|e| {
        GitError::new(
            ErrorCategory::Internal,
            format!("open repo: {}", e.message()),
        )
    })?;
    // Resolve HEAD commit (may fail for empty repo)
    let head_oid = repo.head().ok().and_then(|h| h.target());

    // Attempt find existing branch
    let existing_branch = repo.find_branch(branch_name, git2::BranchType::Local).ok();
    if let Some(br) = existing_branch {
        // Branch exists
        if force {
            if head_oid.is_none() {
                return Err(GitError::new(
                    ErrorCategory::Protocol,
                    "no commit to move branch to",
                ));
            }
            if should_interrupt.load(Ordering::Relaxed) {
                return Err(GitError::new(ErrorCategory::Cancel, "user canceled"));
            }
            if let Some(oid) = head_oid {
                let commit = repo.find_commit(oid).map_err(|e| {
                    GitError::new(
                        ErrorCategory::Internal,
                        format!("find commit: {}", e.message()),
                    )
                })?;
                br.into_reference()
                    .set_target(commit.id(), "force branch move")
                    .map_err(|e| {
                        GitError::new(
                            ErrorCategory::Internal,
                            format!("move branch: {}", e.message()),
                        )
                    })?;
            }
        } else if !checkout {
            return Err(GitError::new(
                ErrorCategory::Protocol,
                "branch already exists",
            ));
        }
        if checkout {
            // perform checkout
            if should_interrupt.load(Ordering::Relaxed) {
                return Err(GitError::new(ErrorCategory::Cancel, "user canceled"));
            }
            let mut co = git2::build::CheckoutBuilder::new();
            co.safe();
            repo.set_head(&format!("refs/heads/{}", branch_name))
                .map_err(|e| {
                    GitError::new(
                        ErrorCategory::Internal,
                        format!("set head: {}", e.message()),
                    )
                })?;
            repo.checkout_head(Some(&mut co)).map_err(|e| {
                GitError::new(
                    ErrorCategory::Internal,
                    format!("checkout: {}", e.message()),
                )
            })?;
            on_progress(ProgressPayload {
                task_id: uuid::Uuid::nil(),
                kind: "GitBranch".into(),
                phase: "BranchedAndCheckedOut".into(),
                percent: 100,
                objects: None,
                bytes: None,
                total_hint: None,
            });
        } else {
            on_progress(ProgressPayload {
                task_id: uuid::Uuid::nil(),
                kind: "GitBranch".into(),
                phase: "Branched".into(),
                percent: 100,
                objects: None,
                bytes: None,
                total_hint: None,
            });
        }
        return Ok(());
    }

    // Branch does not exist -> need a HEAD commit to point at
    let head_oid = match head_oid {
        Some(oid) => oid,
        None => {
            return Err(GitError::new(
                ErrorCategory::Protocol,
                "repository has no commits (cannot create branch)",
            ))
        }
    };
    if should_interrupt.load(Ordering::Relaxed) {
        return Err(GitError::new(ErrorCategory::Cancel, "user canceled"));
    }
    let commit = repo.find_commit(head_oid).map_err(|e| {
        GitError::new(
            ErrorCategory::Internal,
            format!("find commit: {}", e.message()),
        )
    })?;
    repo.branch(branch_name, &commit, false).map_err(|e| {
        GitError::new(
            ErrorCategory::Internal,
            format!("create branch: {}", e.message()),
        )
    })?;
    if checkout {
        if should_interrupt.load(Ordering::Relaxed) {
            return Err(GitError::new(ErrorCategory::Cancel, "user canceled"));
        }
        let mut co = git2::build::CheckoutBuilder::new();
        co.safe();
        repo.set_head(&format!("refs/heads/{}", branch_name))
            .map_err(|e| {
                GitError::new(
                    ErrorCategory::Internal,
                    format!("set head: {}", e.message()),
                )
            })?;
        repo.checkout_head(Some(&mut co)).map_err(|e| {
            GitError::new(
                ErrorCategory::Internal,
                format!("checkout: {}", e.message()),
            )
        })?;
        on_progress(ProgressPayload {
            task_id: uuid::Uuid::nil(),
            kind: "GitBranch".into(),
            phase: "BranchedAndCheckedOut".into(),
            percent: 100,
            objects: None,
            bytes: None,
            total_hint: None,
        });
    } else {
        on_progress(ProgressPayload {
            task_id: uuid::Uuid::nil(),
            kind: "GitBranch".into(),
            phase: "Branched".into(),
            percent: 100,
            objects: None,
            bytes: None,
            total_hint: None,
        });
    }
    Ok(())
}
