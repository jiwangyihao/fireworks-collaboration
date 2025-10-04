use std::{
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};

use super::super::{
    errors::{ErrorCategory, GitError},
    service::ProgressPayload,
};
use super::refname::validate_tag_name;

/// Create (or update with force) a tag pointing to the current HEAD commit.
/// Supports lightweight and annotated tags.
/// Rules:
/// - dest must be a git repo (.git present) else Protocol.
/// - tag name validated via `validate_tag_name`.
/// - annotated=true requires non-empty trimmed message; lightweight ignores message.
/// - repository must have a HEAD commit (no unborn HEAD) else Protocol.
/// - existing tag:
///     * force=false -> Protocol (already exists)
///     * force=true  -> overwrite
///         - lightweight: 更新 refs/tags/<name> 指向新的 HEAD 提交（不创建 tag 对象）
///         - annotated: 创建新的 tag 对象（新 OID），并更新引用指向新对象（旧对象可被 GC 回收）
/// - cancellation checked early and right before mutation.
///
/// Progress: single final progress event phase = "Tagged" or "`AnnotatedTagged`" (both for create / force overwrite).
pub fn git_tag<F: FnMut(ProgressPayload)>(
    dest: &Path,
    name: &str,
    message: Option<&str>,
    annotated: bool,
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
    let tag_name = name.trim();
    validate_tag_name(tag_name)?;
    let msg_trimmed = message.map(|m| m.trim().to_string());
    if annotated {
        let valid = msg_trimmed.as_ref().map(|s| !s.is_empty()).unwrap_or(false);
        if !valid {
            return Err(GitError::new(
                ErrorCategory::Protocol,
                "annotated tag requires non-empty message",
            ));
        }
    }
    let repo = git2::Repository::open(dest).map_err(|e| {
        GitError::new(
            ErrorCategory::Internal,
            format!("open repo: {}", e.message()),
        )
    })?;
    // HEAD commit required
    let head_oid = repo
        .head()
        .ok()
        .and_then(|h| h.target())
        .ok_or_else(|| GitError::new(ErrorCategory::Protocol, "repository has no commits"))?;
    if should_interrupt.load(Ordering::Relaxed) {
        return Err(GitError::new(ErrorCategory::Cancel, "user canceled"));
    }

    let existing_ref = repo.find_reference(&format!("refs/tags/{tag_name}")).ok();
    if existing_ref.is_some() && !force {
        return Err(GitError::new(ErrorCategory::Protocol, "tag already exists"));
    }

    // Resolve target commit
    let target_commit = repo.find_commit(head_oid).map_err(|e| {
        GitError::new(
            ErrorCategory::Internal,
            format!("find commit: {}", e.message()),
        )
    })?;

    if should_interrupt.load(Ordering::Relaxed) {
        return Err(GitError::new(ErrorCategory::Cancel, "user canceled"));
    }

    if annotated {
        // Build signature (repository signature – same for tagger)
        let sig = repo.signature().map_err(|e| {
            GitError::new(
                ErrorCategory::Internal,
                format!("signature: {}", e.message()),
            )
        })?;
        // CRLF 归一化：统一将 \r\n -> \n，遗留的单独 \r 也替换为 \n，保持 git 常见风格
        let raw = msg_trimmed.unwrap();
        let mut msg = raw.replace("\r\n", "\n");
        if msg.contains('\r') {
            msg = msg.replace('\r', "\n");
        }
        // 统一尾部空行：裁剪末尾空白后，若原本非空且不以换行结尾，补一个换行；若已多余换行，压缩为恰好一个结尾换行
        let trimmed_end = msg.trim_end();
        if !trimmed_end.is_empty() {
            msg = format!("{trimmed_end}\n");
        }
        // 若是 force 且已有引用，尝试复用：比较现有 tag 对象内容（目标 commit、消息、tagger）一致则不创建新对象
        let mut reused = false;
        if force {
            if let Some(existing) = existing_ref.as_ref() {
                if let Ok(obj) = existing.peel(git2::ObjectType::Tag) {
                    if let Ok(old_tag) = obj.into_tag() {
                        let same_target = old_tag.target_id() == target_commit.id();
                        let same_msg = old_tag.message().map(|m| m == msg).unwrap_or(false);
                        let same_tagger = old_tag.tagger().map(|t| t == sig).unwrap_or(false);
                        if same_target && same_msg && same_tagger {
                            // 复用：不创建新对象，只维持引用（引用已指向该对象）
                            reused = true;
                        }
                    }
                }
            }
        }
        if !reused {
            repo.tag(tag_name, target_commit.as_object(), &sig, &msg, force)
                .map_err(|e| {
                    GitError::new(
                        ErrorCategory::Internal,
                        format!("create annotated tag: {}", e.message()),
                    )
                })?;
        }
        let phase = if existing_ref.is_some() && force {
            "AnnotatedRetagged"
        } else {
            "AnnotatedTagged"
        };
        on_progress(ProgressPayload {
            task_id: uuid::Uuid::nil(),
            kind: "GitTag".into(),
            phase: phase.into(),
            percent: 100,
            objects: None,
            bytes: None,
            total_hint: None,
        });
    } else {
        // Lightweight: just create/update reference
        // If existing and force -> update ref target
        let had_existing = existing_ref.is_some();
        if let Some(mut rf) = existing_ref {
            // update
            rf.set_target(target_commit.id(), "force update tag")
                .map_err(|e| {
                    GitError::new(
                        ErrorCategory::Internal,
                        format!("update tag ref: {}", e.message()),
                    )
                })?;
        } else {
            repo.reference(
                &format!("refs/tags/{tag_name}"),
                target_commit.id(),
                force,
                "create tag",
            )
            .map_err(|e| {
                GitError::new(
                    ErrorCategory::Internal,
                    format!("create tag ref: {}", e.message()),
                )
            })?;
        }
        let phase = if had_existing && force {
            "Retagged"
        } else {
            "Tagged"
        };
        on_progress(ProgressPayload {
            task_id: uuid::Uuid::nil(),
            kind: "GitTag".into(),
            phase: phase.into(),
            percent: 100,
            objects: None,
            bytes: None,
            total_hint: None,
        });
    }
    Ok(())
}
