use std::{
    path::Path,
    sync::{Arc, Mutex},
};

use crate::core::git::transport::maybe_rewrite_https_to_custom;

use super::super::{
    errors::{ErrorCategory, GitError},
    service::ProgressPayload,
};
use super::helpers;

pub fn do_clone<F: FnMut(ProgressPayload)>(
    repo_url_final: &str,
    dest: &Path,
    depth: Option<u32>,
    should_interrupt: &std::sync::atomic::AtomicBool,
    on_progress: F,
) -> Result<(), GitError> {
    let cb = Arc::new(Mutex::new(on_progress));

    let mut callbacks = git2::RemoteCallbacks::new();
    let cb_for_transfer = Arc::clone(&cb);
    callbacks.transfer_progress(move |stats| {
        if should_interrupt.load(std::sync::atomic::Ordering::Relaxed) {
            return false;
        }
        let received = stats.received_objects() as u64;
        let total = stats.total_objects() as u64;
        let bytes = stats.received_bytes() as u64;
        let percent = helpers::percent(received, total).min(100);
        if let Ok(mut f) = cb_for_transfer.lock() {
            (*f)(ProgressPayload {
                task_id: uuid::Uuid::nil(),
                kind: "GitClone".into(),
                phase: "Receiving".into(),
                percent,
                objects: Some(received),
                bytes: Some(bytes),
                total_hint: helpers::total_hint(total),
            });
        }
        true
    });

    let mut fo = git2::FetchOptions::new();
    if let Some(d) = depth {
        fo.depth(d as i32);
    }
    fo.remote_callbacks(callbacks);
    fo.download_tags(git2::AutotagOption::Unspecified);
    fo.proxy_options(git2::ProxyOptions::new());
    fo.update_fetchhead(true);

    let mut co = git2::build::CheckoutBuilder::new();
    let cb_for_checkout = Arc::clone(&cb);
    co.progress(move |_p, completed, total| {
        let percent = if total > 0 {
            ((completed as f64 / total as f64) * 100.0) as u32
        } else {
            0
        };
        let mapped = 90u32
            .saturating_add(((percent.min(100) as f64) * 0.1) as u32)
            .min(100);
        if let Ok(mut f) = cb_for_checkout.lock() {
            (*f)(ProgressPayload {
                task_id: uuid::Uuid::nil(),
                kind: "GitClone".into(),
                phase: "Checkout".into(),
                percent: mapped,
                objects: None,
                bytes: None,
                total_hint: None,
            });
        }
    });

    let mut builder = git2::build::RepoBuilder::new();
    builder.fetch_options(fo);
    builder.with_checkout(co);

    match builder.clone(repo_url_final, dest) {
        Ok(_repo) => {
            if let Ok(mut f) = cb.lock() {
                (*f)(ProgressPayload {
                    task_id: uuid::Uuid::nil(),
                    kind: "GitClone".into(),
                    phase: "Completed".into(),
                    percent: 100,
                    objects: None,
                    bytes: None,
                    total_hint: None,
                });
            }
            Ok(())
        }
        Err(e) => {
            let cat = helpers::map_git2_error(&e);
            Err(GitError::new(cat, e.message().to_string()))
        }
    }
}

pub fn do_fetch<F: FnMut(ProgressPayload)>(
    repo_url: &str,
    dest: &Path,
    depth: Option<u32>,
    cfg: &crate::core::config::model::AppConfig,
    should_interrupt: &std::sync::atomic::AtomicBool,
    on_progress: F,
) -> Result<(), GitError> {
    let repo = match git2::Repository::open(dest) {
        Ok(r) => r,
        Err(e) => {
            return Err(GitError::new(
                helpers::map_git2_error(&e),
                format!("open repo: {e}"),
            ))
        }
    };
    let cb = Arc::new(Mutex::new(on_progress));

    let mut callbacks = git2::RemoteCallbacks::new();
    let cb_for_transfer = Arc::clone(&cb);
    callbacks.transfer_progress(move |stats| {
        if should_interrupt.load(std::sync::atomic::Ordering::Relaxed) {
            return false;
        }
        let received = stats.received_objects() as u64;
        let total = stats.total_objects() as u64;
        let bytes = stats.received_bytes() as u64;
        let percent = helpers::percent(received, total).min(100);
        if let Ok(mut f) = cb_for_transfer.lock() {
            (*f)(ProgressPayload {
                task_id: uuid::Uuid::nil(),
                kind: "GitFetch".into(),
                phase: "Receiving".into(),
                percent,
                objects: Some(received),
                bytes: Some(bytes),
                total_hint: helpers::total_hint(total),
            });
        }
        true
    });

    let mut fo = git2::FetchOptions::new();
    if let Some(d) = depth {
        fo.depth(d as i32);
    }
    fo.remote_callbacks(callbacks);
    fo.download_tags(git2::AutotagOption::Unspecified);
    fo.update_fetchhead(true);

    let (mut remote, is_anonymous) = resolve_remote_to_use_ex(&repo, repo_url, cfg)?;

    // 对于匿名远程（使用 URL 而非远程名称），需要显式指定 refspec
    // 以确保远程跟踪分支被正确更新
    let refspecs: Vec<&str> = if is_anonymous {
        // 使用标准 refspec 更新 origin 远程跟踪分支
        vec!["refs/heads/*:refs/remotes/origin/*"]
    } else {
        // 使用命名远程时，传空数组会使用远程的默认 refspecs
        vec![]
    };

    match remote.fetch(&refspecs, Some(&mut fo), None) {
        Ok(_) => {
            if let Ok(mut f) = cb.lock() {
                (*f)(ProgressPayload {
                    task_id: uuid::Uuid::nil(),
                    kind: "GitFetch".into(),
                    phase: "Completed".into(),
                    percent: 100,
                    objects: None,
                    bytes: None,
                    total_hint: None,
                });
            }
            Ok(())
        }
        Err(e) => {
            let cat = helpers::map_git2_error(&e);
            Err(GitError::new(cat, e.message().to_string()))
        }
    }
}

/// Resolves which remote to use for a fetch operation.
/// If `url` is empty, tries to find "origin" or any existing remote.
/// Rewrites HTTPS URLs to custom transport if configured.
pub fn resolve_remote_to_use<'a>(
    repo: &'a git2::Repository,
    url: &str,
    cfg: &crate::core::config::model::AppConfig,
) -> Result<git2::Remote<'a>, GitError> {
    resolve_remote_to_use_ex(repo, url, cfg).map(|(r, _)| r)
}

/// Extended version of resolve_remote_to_use that also returns whether
/// an anonymous remote was created (true) or a named remote was used (false).
/// This is important for determining whether explicit refspecs are needed for fetch.
pub fn resolve_remote_to_use_ex<'a>(
    repo: &'a git2::Repository,
    url: &str,
    cfg: &crate::core::config::model::AppConfig,
) -> Result<(git2::Remote<'a>, bool), GitError> {
    let url_trimmed = url.trim();
    if url_trimmed.is_empty() {
        let named = match repo.find_remote("origin") {
            Ok(r) => Some(r),
            Err(_) => match repo.remotes() {
                Ok(names) => {
                    if let Some(name) = names.iter().flatten().next() {
                        repo.find_remote(name).ok()
                    } else {
                        None
                    }
                }
                Err(_) => None,
            },
        };
        if let Some(r) = named {
            let url_opt = r.url().map(|s| s.to_string());
            if let Some(u) = url_opt {
                if let Some(new_url) = maybe_rewrite_https_to_custom(cfg, u.as_str()) {
                    repo.remote_anonymous(&new_url)
                        .map(|remote| (remote, true))
                        .map_err(|e| {
                            GitError::new(
                                helpers::map_git2_error(&e),
                                format!("remote anonymous with rewritten url: {e}"),
                            )
                        })
                } else {
                    Ok((r, false))
                }
            } else {
                Ok((r, false))
            }
        } else {
            Err(GitError::new(
                ErrorCategory::Internal,
                "no remote configured",
            ))
        }
    } else {
        match repo.find_remote(url_trimmed) {
            Ok(r) => {
                let url_opt = r.url().map(|s| s.to_string());
                if let Some(u) = url_opt {
                    if let Some(new_url) = maybe_rewrite_https_to_custom(cfg, u.as_str()) {
                        repo.remote_anonymous(&new_url)
                            .map(|remote| (remote, true))
                            .map_err(|e| {
                                GitError::new(
                                    helpers::map_git2_error(&e),
                                    format!("remote anonymous with rewritten url: {e}"),
                                )
                            })
                    } else {
                        Ok((r, false))
                    }
                } else {
                    Ok((r, false))
                }
            }
            Err(_) => {
                let final_url = maybe_rewrite_https_to_custom(cfg, url_trimmed)
                    .unwrap_or_else(|| url_trimmed.to_string());
                repo.remote_anonymous(&final_url)
                    .map(|remote| (remote, true))
                    .map_err(|e| {
                        GitError::new(helpers::map_git2_error(&e), format!("remote lookup: {e}"))
                    })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::model::AppConfig;

    #[test]
    fn test_resolve_remote_to_use_invalid_repo() {
        let temp = tempfile::tempdir().unwrap();
        // Repository::init(...) is needed to get a real repo for find_remote to run
        let repo = git2::Repository::init(temp.path()).unwrap();
        let cfg = AppConfig::default();

        // No remotes yet
        let res = resolve_remote_to_use(&repo, "", &cfg);
        match res {
            Err(e) => {
                assert!(
                    e.category() == ErrorCategory::Internal
                        || e.to_string().contains("no remote configured")
                );
            }
            Ok(_) => panic!("should fail"),
        }

        // Direct URL (anonymous)
        // Note: github.com is whitelisted for fake SNI rewrite. Disable it for stable assertion.
        let mut cfg_no_rewrite = AppConfig::default();
        cfg_no_rewrite.http.fake_sni_enabled = false;
        let res = resolve_remote_to_use(&repo, "https://github.com/foo", &cfg_no_rewrite);
        if let Ok(rem) = res {
            assert_eq!(rem.url().unwrap(), "https://github.com/foo");
        } else {
            panic!("should succeed");
        }
    }
}
