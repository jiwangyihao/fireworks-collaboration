use std::{path::Path, sync::{Arc, Mutex}};

use crate::core::git::transport::maybe_rewrite_https_to_custom;

use super::super::{errors::{GitError, ErrorCategory}, service::ProgressPayload};
use super::helpers;

pub fn do_clone<F: FnMut(ProgressPayload)>(repo_url_final: &str, dest: &Path, should_interrupt: &std::sync::atomic::AtomicBool, on_progress: F) -> Result<(), GitError> {
    let cb = Arc::new(Mutex::new(on_progress));

    let mut callbacks = git2::RemoteCallbacks::new();
    let cb_for_transfer = Arc::clone(&cb);
    callbacks.transfer_progress(move |stats| {
        if should_interrupt.load(std::sync::atomic::Ordering::Relaxed) { return false; }
        let received = stats.received_objects() as u64;
        let total = stats.total_objects() as u64;
        let bytes = stats.received_bytes() as u64;
        let percent = helpers::percent(received, total).min(100);
        if let Ok(mut f) = cb_for_transfer.lock() { (*f)(ProgressPayload { task_id: uuid::Uuid::nil(), kind: "GitClone".into(), phase: "Receiving".into(), percent, objects: Some(received), bytes: Some(bytes), total_hint: helpers::total_hint(total) }); }
        true
    });

    let mut fo = git2::FetchOptions::new();
    fo.remote_callbacks(callbacks);
    fo.download_tags(git2::AutotagOption::Unspecified);
    fo.proxy_options(git2::ProxyOptions::new());
    fo.update_fetchhead(true);

    let mut co = git2::build::CheckoutBuilder::new();
    let cb_for_checkout = Arc::clone(&cb);
    co.progress(move |_p, completed, total| {
        let percent = if total > 0 { ((completed as f64 / total as f64) * 100.0) as u32 } else { 0 };
        let mapped = 90u32.saturating_add(((percent.min(100) as f64) * 0.1) as u32).min(100);
        if let Ok(mut f) = cb_for_checkout.lock() { (*f)(ProgressPayload { task_id: uuid::Uuid::nil(), kind: "GitClone".into(), phase: "Checkout".into(), percent: mapped, objects: None, bytes: None, total_hint: None }); }
    });

    let mut builder = git2::build::RepoBuilder::new();
    builder.fetch_options(fo);
    builder.with_checkout(co);

    match builder.clone(repo_url_final, dest) {
        Ok(_repo) => { if let Ok(mut f) = cb.lock() { (*f)(ProgressPayload { task_id: uuid::Uuid::nil(), kind: "GitClone".into(), phase: "Completed".into(), percent: 100, objects: None, bytes: None, total_hint: None }); } Ok(()) }
        Err(e) => { let cat = helpers::map_git2_error(&e); Err(GitError::new(cat, e.message().to_string())) }
    }
}

pub fn do_fetch<F: FnMut(ProgressPayload)>(repo_url:&str, dest:&Path, cfg:&crate::core::config::model::AppConfig, should_interrupt:&std::sync::atomic::AtomicBool, on_progress:F) -> Result<(), GitError> {
    let repo = match git2::Repository::open(dest) { Ok(r)=>r, Err(e)=> return Err(GitError::new(helpers::map_git2_error(&e), format!("open repo: {}", e))) };
    let cb = Arc::new(Mutex::new(on_progress));

    let mut callbacks = git2::RemoteCallbacks::new();
    let cb_for_transfer = Arc::clone(&cb);
    callbacks.transfer_progress(move |stats| {
        if should_interrupt.load(std::sync::atomic::Ordering::Relaxed) { return false; }
        let received = stats.received_objects() as u64;
        let total = stats.total_objects() as u64;
        let bytes = stats.received_bytes() as u64;
        let percent = helpers::percent(received, total).min(100);
        if let Ok(mut f) = cb_for_transfer.lock() { (*f)(ProgressPayload { task_id: uuid::Uuid::nil(), kind: "GitFetch".into(), phase: "Receiving".into(), percent, objects: Some(received), bytes: Some(bytes), total_hint: helpers::total_hint(total) }); }
        true
    });

    let mut fo = git2::FetchOptions::new();
    fo.remote_callbacks(callbacks);
    fo.download_tags(git2::AutotagOption::Unspecified);
    fo.update_fetchhead(true);

    let repo_url_trimmed = repo_url.trim();
    let mut remote = if repo_url_trimmed.is_empty() {
        let named = match repo.find_remote("origin") { Ok(r)=>Some(r), Err(_)=>{
            match repo.remotes() { Ok(names)=>{ if let Some(name) = names.iter().flatten().next() { match repo.find_remote(name) { Ok(r)=>Some(r), Err(_)=>None } } else { None } }, Err(_)=>None }
        }};
        if let Some(r) = named {
            let url_opt = r.url().map(|s| s.to_string());
            if let Some(u) = url_opt { if let Some(new_url) = maybe_rewrite_https_to_custom(cfg, u.as_str()) { match repo.remote_anonymous(&new_url) { Ok(r2)=>r2, Err(e)=> return Err(GitError::new(helpers::map_git2_error(&e), format!("remote anonymous with rewritten url: {}", e))) } } else { r } }
            else { r }
        } else { return Err(GitError::new(ErrorCategory::Internal, "no remote configured")); }
    } else {
        match repo.find_remote(repo_url_trimmed) {
            Ok(r) => {
                let url_opt = r.url().map(|s| s.to_string());
                if let Some(u) = url_opt { if let Some(new_url) = maybe_rewrite_https_to_custom(cfg, u.as_str()) { match repo.remote_anonymous(&new_url) { Ok(r2)=>r2, Err(e)=> return Err(GitError::new(helpers::map_git2_error(&e), format!("remote anonymous with rewritten url: {}", e))) } } else { r } }
                else { r }
            }
            Err(_) => {
                let final_url = maybe_rewrite_https_to_custom(cfg, repo_url_trimmed).unwrap_or_else(|| repo_url_trimmed.to_string());
                match repo.remote_anonymous(&final_url) { Ok(r)=>r, Err(e)=> return Err(GitError::new(helpers::map_git2_error(&e), format!("remote lookup: {}", e))) }
            }
        }
    };

    match remote.fetch(&[] as &[&str], Some(&mut fo), None) {
        Ok(_) => { if let Ok(mut f) = cb.lock() { (*f)(ProgressPayload { task_id: uuid::Uuid::nil(), kind: "GitFetch".into(), phase: "Completed".into(), percent: 100, objects: None, bytes: None, total_hint: None }); } Ok(()) }
        Err(e) => { let cat = helpers::map_git2_error(&e); Err(GitError::new(cat, e.message().to_string())) }
    }
}
