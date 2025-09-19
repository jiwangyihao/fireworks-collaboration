use std::sync::{atomic::{AtomicBool, Ordering}, Arc, Mutex};
use url::Url;

use crate::core::config::loader::load_or_init;
use crate::core::tls::util::{decide_sni_host_with_proxy, proxy_present};

use super::super::{errors::{GitError, ErrorCategory}, service::ProgressPayload};

#[inline]
pub fn preflight_generic<F: FnMut(ProgressPayload)>(kind: &str, should_interrupt: &AtomicBool, on_progress: &mut F) -> Result<(), GitError> {
    (*on_progress)(ProgressPayload { task_id: uuid::Uuid::nil(), kind: kind.into(), phase: "Negotiating".into(), percent: 0, objects: None, bytes: None, total_hint: None });
    if should_interrupt.load(Ordering::Relaxed) { return Err(GitError::new(ErrorCategory::Cancel, "user canceled")); }
    Ok(())
}

#[inline]
pub fn emit_sni_status<F: FnMut(ProgressPayload)>(kind:&str, maybe_repo_url: Option<&str>, on_progress: &mut F) {
    let cfg = match load_or_init() { Ok(c)=>c, Err(_)=>return };
    let host_opt = maybe_repo_url.and_then(|u| Url::parse(u).ok()).and_then(|u| u.host_str().map(|s| s.to_string()));
    let proxy = proxy_present();
    let (sni, used_fake) = match host_opt.as_deref() { Some(h)=>decide_sni_host_with_proxy(&cfg,false,h,proxy), None=>decide_sni_host_with_proxy(&cfg,false, "github.com", proxy) };
    let phase = match host_opt { Some(h)=> format!("UsingSNI host={} sni={} fake={} rotate403={} proxy={}", h, sni, used_fake, cfg.http.sni_rotate_on_403, proxy), None=> format!("UsingSNI sni={} fake={} rotate403={} proxy={} (host=unknown)", sni, used_fake, cfg.http.sni_rotate_on_403, proxy) };
    (*on_progress)(ProgressPayload { task_id: uuid::Uuid::nil(), kind: kind.into(), phase, percent: 0, objects: None, bytes: None, total_hint: None });
}

pub fn map_git2_error(e: &git2::Error) -> ErrorCategory {
    use git2::ErrorClass as C;
    if e.code() == git2::ErrorCode::User { return ErrorCategory::Cancel; }
    let msg = e.message().to_ascii_lowercase();
    if msg.contains("timed out") || msg.contains("timeout") || msg.contains("connection") || msg.contains("connect") || matches!(e.class(), C::Net) { return ErrorCategory::Network; }
    if msg.contains("ssl") || msg.contains("tls") { return ErrorCategory::Tls; }
    if msg.contains("certificate") || msg.contains("x509") { return ErrorCategory::Verify; }
    if msg.contains("401") || msg.contains("403") || msg.contains("auth")
        || msg.contains("unauthorized") || msg.contains("permission denied")
        || msg.contains("www-authenticate") || msg.contains("basic realm")
    { return ErrorCategory::Auth; }
    if matches!(e.class(), C::Http) { return ErrorCategory::Protocol; }
    ErrorCategory::Internal
}

#[inline]
pub fn percent(received:u64, total:u64) -> u32 { if total>0 { ((received as f64 / total as f64) * 100.0) as u32 } else { 0 } }
#[inline]
pub fn total_hint(total:u64) -> Option<u64> { if total>0 { Some(total) } else { None } }

#[inline]
pub fn push_phase_event<F: FnMut(ProgressPayload)>(cb: &Arc<Mutex<F>>, phase: &str, percent: u32) {
    if let Ok(mut f) = cb.lock() { (*f)(ProgressPayload { task_id: uuid::Uuid::nil(), kind: "GitPush".into(), phase: phase.into(), percent, objects: None, bytes: None, total_hint: None }); }
}

/// 统一判定输入字符串是否“看起来像本地路径”（而不是远端 URL）。
/// 规则：
/// - 绝对路径
/// - 以 ./ 或 ../ 开头
/// - 包含反斜杠（Windows 常见）
/// - 不包含 :// 但存在文件系统实体（存在的目录或文件）
pub fn is_local_path_candidate(s: &str) -> bool {
    if s.is_empty() { return false; }
    let p = std::path::Path::new(s);
    if p.is_absolute() || s.starts_with("./") || s.starts_with("../") || s.contains('\\') { return true; }
    if !s.contains("://") && p.exists() { return true; }
    false
}
