#![cfg(feature = "tauri-app")]
use std::{sync::{Arc, Mutex}, io::{Read, Write}, net::{TcpListener, TcpStream}, thread, path::PathBuf};
use serde::{Serialize, Deserialize};
use tauri::{Manager, State};
use crate::core::{
    config::{loader as cfg_loader, model::AppConfig},
    tasks::{TaskRegistry, SharedTaskRegistry, TaskSnapshot, TaskKind},
    http::{types::{HttpRequestInput, HttpResponseOutput, RedirectInfo}, client::HttpClient},
    tls::util::match_domain,
};
use crate::logging;

// ===== State Types =====
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct OAuthCallbackData { pub code: Option<String>, pub state: Option<String>, pub error: Option<String>, pub error_description: Option<String> }

type OAuthState = Arc<Mutex<Option<OAuthCallbackData>>>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemProxy { pub enabled: bool, pub host: String, pub port: u16, pub bypass: String }
impl Default for SystemProxy { fn default() -> Self { Self { enabled:false, host:"".into(), port:0, bypass:"".into() } } }

type SharedConfig = Arc<Mutex<AppConfig>>;
type ConfigBaseDir = PathBuf;
type TaskRegistryState = SharedTaskRegistry;

// ===== Commands =====
#[tauri::command]
fn greet(name:&str)->String{ format!("Hello, {}! You've been greeted from Rust!", name) }

#[tauri::command]
async fn get_config(cfg: State<'_, SharedConfig>) -> Result<AppConfig,String>{ cfg.lock().map(|c|c.clone()).map_err(|e|e.to_string()) }

#[tauri::command]
#[allow(non_snake_case)]
async fn set_config(newCfg: AppConfig, cfg: State<'_, SharedConfig>, base: State<'_, ConfigBaseDir>) -> Result<(),String>{ { let mut g = cfg.lock().map_err(|e|e.to_string())?; *g = newCfg.clone(); } cfg_loader::save_at(&newCfg, &*base).map_err(|e|e.to_string()) }

#[tauri::command]
async fn start_oauth_server(state: State<'_, OAuthState>) -> Result<String,String>{ let oauth_state = Arc::clone(&*state); thread::spawn(move||{ if let Ok(listener)=TcpListener::bind("127.0.0.1:3429"){ for stream in listener.incoming(){ if let Ok(s)=stream { let st=Arc::clone(&oauth_state); thread::spawn(move|| handle_oauth_request(s, st)); } } } }); Ok("OAuth server started".into()) }

fn handle_oauth_request(mut stream: TcpStream, oauth_state: OAuthState){ let mut buf=[0u8;4096]; if let Ok(n)=stream.read(&mut buf){ let req = String::from_utf8_lossy(&buf[..n]); if req.starts_with("GET /auth/callback") { let mut data=OAuthCallbackData{code:None,state:None,error:None,error_description:None}; if let Some(q_pos)=req.find('?'){ let tail=&req[q_pos+1..]; let end=tail.find(' ').unwrap_or(tail.len()); for kv in tail[..end].split('&'){ if let Some(eq)=kv.find('='){ let (k,v)=(&kv[..eq], &kv[eq+1..]); let v= urlencoding::decode(v).unwrap_or_default().to_string(); match k {"code"=>data.code=Some(v),"state"=>data.state=Some(v),"error"=>data.error=Some(v),"error_description"=>data.error_description=Some(v), _=>{} } } } } if let Ok(mut s)=oauth_state.lock(){ *s=Some(data); } let body="<html><body>OK</body></html>"; let resp=format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body); let _=stream.write_all(resp.as_bytes()); } else { let _=stream.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length:0\r\n\r\n"); } } }

#[tauri::command]
async fn get_oauth_callback_data(state: State<'_, OAuthState>) -> Result<Option<OAuthCallbackData>,String>{ if let Ok(mut s)=state.lock(){ Ok(s.take()) } else { Err("lock".into()) } }
#[tauri::command]
async fn clear_oauth_state(state: State<'_, OAuthState>) -> Result<(),String>{ if let Ok(mut s)=state.lock(){ *s=None; Ok(()) } else { Err("lock".into()) } }

#[cfg(windows)] fn inner_get_system_proxy()->SystemProxy{ SystemProxy::default() }
#[cfg(not(windows))] fn inner_get_system_proxy()->SystemProxy{ SystemProxy::default() }
#[tauri::command] fn get_system_proxy()->Result<SystemProxy,String>{ Ok(inner_get_system_proxy()) }

// Task commands
#[tauri::command]
async fn task_list(reg: State<'_, TaskRegistryState>) -> Result<Vec<TaskSnapshot>,String>{ Ok(reg.list()) }
#[tauri::command]
async fn task_snapshot(id:String, reg: State<'_, TaskRegistryState>) -> Result<Option<TaskSnapshot>,String>{ let uuid=uuid::Uuid::parse_str(&id).map_err(|e|e.to_string())?; Ok(reg.snapshot(&uuid)) }
#[tauri::command]
async fn task_cancel(id:String, reg: State<'_, TaskRegistryState>) -> Result<bool,String>{ let uuid=uuid::Uuid::parse_str(&id).map_err(|e|e.to_string())?; Ok(reg.cancel(&uuid)) }
#[tauri::command]
async fn task_start_sleep(ms:u64, reg: State<'_, TaskRegistryState>, app: tauri::AppHandle) -> Result<String,String>{ let (id, token)=reg.create(TaskKind::Sleep { ms }); reg.clone().spawn_sleep_task(Some(app), id, token, ms); Ok(id.to_string()) }

// Git 命令：启动克隆任务
#[tauri::command]
async fn git_clone(repo: String, dest: String, reg: State<'_, TaskRegistryState>, app: tauri::AppHandle) -> Result<String, String> {
    let (id, token) = reg.create(TaskKind::GitClone { repo: repo.clone(), dest: dest.clone() });
    reg.clone().spawn_git_clone_task(Some(app), id, token, repo, dest);
    Ok(id.to_string())
}

// ========== P0.5 http_fake_request ==========
fn redact_auth_in_headers(mut h: std::collections::HashMap<String, String>, mask: bool) -> std::collections::HashMap<String, String> {
    if !mask { return h; }
    // 大小写不敏感匹配 Authorization
    for (k, v) in h.clone().iter() {
        if k.eq_ignore_ascii_case("authorization") {
            let _ = v; // silence unused warning
            h.insert(k.clone(), "REDACTED".into());
        }
    }
    h
}

fn host_in_whitelist(host: &str, cfg: &AppConfig) -> bool {
    let wl = &cfg.tls.san_whitelist;
    if wl.is_empty() { return false; }
    wl.iter().any(|p| match_domain(p, host))
}

fn classify_error_msg(e: &str) -> (&'static str, String) {
    let msg = e.to_string();
    if msg.contains("SAN whitelist mismatch") { ("Verify", msg) }
    else if msg.contains("tls handshake") { ("Tls", msg) }
    else if msg.contains("connect timeout") || msg.contains("connect error") || msg.contains("read body") { ("Network", msg) }
    else if msg.contains("only https") || msg.contains("invalid URL") || msg.contains("url host missing") { ("Input", msg) }
    else { ("Internal", msg) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_redact_auth_in_headers_case_insensitive() {
        let mut h = std::collections::HashMap::new();
        h.insert("Authorization".to_string(), "Bearer abc".to_string());
        h.insert("x-other".to_string(), "1".to_string());
        let out = redact_auth_in_headers(h, true);
        assert_eq!(out.get("Authorization").unwrap(), "REDACTED");
        assert_eq!(out.get("x-other").unwrap(), "1");

        let mut h2 = std::collections::HashMap::new();
        h2.insert("aUtHoRiZaTiOn".to_string(), "token".to_string());
        let out2 = redact_auth_in_headers(h2, true);
        assert_eq!(out2.get("aUtHoRiZaTiOn").unwrap(), "REDACTED");
    }

    #[test]
    fn test_redact_auth_no_mask_keeps_original() {
        let mut h = std::collections::HashMap::new();
        h.insert("Authorization".to_string(), "Bearer xyz".to_string());
        let out = redact_auth_in_headers(h, false);
        assert_eq!(out.get("Authorization").unwrap(), "Bearer xyz");
    }

    #[test]
    fn test_host_in_whitelist_exact_and_wildcard() {
        let mut cfg = AppConfig::default();
        // default has github.com and *.github.com
        assert!(host_in_whitelist("github.com", &cfg));
        assert!(host_in_whitelist("api.github.com", &cfg));
        assert!(!host_in_whitelist("example.com", &cfg));

        // empty whitelist -> reject any
        cfg.tls.san_whitelist.clear();
        assert!(!host_in_whitelist("github.com", &cfg));
    }

    #[test]
    fn test_classify_error_msg_mapping() {
        let cases = vec![
            ("SAN whitelist mismatch", "Verify"),
            ("Tls: tls handshake", "Tls"),
            ("connect timeout", "Network"),
            ("connect error", "Network"),
            ("read body", "Network"),
            ("only https", "Input"),
            ("invalid URL", "Input"),
            ("url host missing", "Input"),
            ("some other error", "Internal"),
        ];
        for (msg, cat) in cases {
            let (got, _m) = classify_error_msg(msg);
            assert_eq!(got, cat, "msg={}", msg);
        }
    }
}

#[tauri::command]
async fn http_fake_request(input: HttpRequestInput, cfg: State<'_, SharedConfig>) -> Result<HttpResponseOutput, String> {
    // 将 MutexGuard 限定在局部作用域，避免跨 await 持有非 Send 的锁
    let cfg_val = {
        let g = cfg.lock().map_err(|e| e.to_string())?;
        g.clone()
    };

    // 早期校验 URL 与 host 白名单
    let mut current_url = input.url.clone();
    let parsed = current_url.parse::<hyper::Uri>().map_err(|e| format!("Input: invalid URL - {}", e))?;
    if parsed.scheme_str() != Some("https") { return Err("Input: only https is supported".into()); }
    let host = parsed.host().ok_or_else(|| "Input: url host missing".to_string())?;
    if !host_in_whitelist(host, &cfg_val) {
        return Err("Verify: SAN whitelist mismatch (precheck)".into());
    }

    // 日志脱敏后记录一次请求概览
    let redacted = redact_auth_in_headers(input.headers.clone(), cfg_val.logging.auth_header_masked);
    tracing::info!(target = "http", method = %input.method, url = %input.url, headers = ?redacted, "http_fake_request start");

    // 构造 client
    let client = HttpClient::new(cfg_val.clone());
    let follow = input.follow_redirects;
    let max_redirects = input.max_redirects;
    let mut redirects: Vec<RedirectInfo> = Vec::new();

    // 为了处理 301/302/303 -> GET，后续 307/308 需要保留“当前尝试”的方法与 body

    let mut attempt_input = input.clone();

    for i in 0..=max_redirects as u16 {
        let result = client.send(attempt_input.clone()).await;
        match result {
            Ok(mut out) => {
                // 检查是否需要继续跳转
                let status = out.status;
                let is_redirect = matches!(status, 301 | 302 | 303 | 307 | 308);
                if !is_redirect || !follow { // 不跟随或非跳转
                    // 合并收集的 redirect 链并返回
                    out.redirects = redirects;
                    return Ok(out);
                }
                // 提取 Location
                let location = out.headers.get("location").cloned();
                if location.is_none() {
                    out.redirects = redirects;
                    return Ok(out);
                }
                let loc = location.unwrap();

                // 解析并构造下一跳 URL（相对路径基于 current_url）
                let base = current_url.parse::<url::Url>().map_err(|e| format!("Internal: url parse {}", e))?;
                let next_url = base.join(&loc).map_err(|e| format!("Input: bad redirect location - {}", e))?.to_string();

                // 白名单预检下一跳 host
                let next_host = url::Url::parse(&next_url).map_err(|e| format!("Internal: url parse {}", e))?
                    .host_str().ok_or_else(|| "Input: redirect host missing".to_string())?.to_string();
                if !host_in_whitelist(&next_host, &cfg_val) { return Err("Verify: SAN whitelist mismatch (redirect)".into()); }

                redirects.push(RedirectInfo { status, location: next_url.clone(), count: (i as u8) + 1 });
                if i as u8 >= max_redirects { return Err(format!("Network: too many redirects (>{})", max_redirects)); }

                // 方法与 body 处理
                let mut next_input = attempt_input.clone();
                next_input.url = next_url.clone();
                current_url = next_url;
                match status {
                    301 | 302 | 303 => {
                        next_input.method = "GET".into();
                        next_input.body_base64 = None;
                    }
                    307 | 308 => {
                        // 保持当前尝试的 method/body（非最初的 original）
                        next_input.method = attempt_input.method.clone();
                        next_input.body_base64 = attempt_input.body_base64.clone();
                    }
                    _ => {}
                }
                attempt_input = next_input;
                continue; // 下一轮尝试
            }
            Err(e) => {
                let (cat, msg) = classify_error_msg(&e.to_string());
                return Err(format!("{}: {}", cat, msg));
            }
        }
    }

    Err("Network: redirect loop reached without resolution".into())
}

pub fn run(){
    logging::init_logging();
    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_http::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_opener::init())
        .manage(OAuthState::new(Mutex::new(None)))
        .manage(Arc::new(TaskRegistry::new()) as TaskRegistryState)
        .invoke_handler(tauri::generate_handler![
            greet,start_oauth_server,get_oauth_callback_data,clear_oauth_state,get_system_proxy,
            get_config,set_config,task_list,task_cancel,task_start_sleep,task_snapshot,git_clone,
            http_fake_request
        ]);
    builder = builder.setup(|app| { let base_dir: PathBuf = app.path().app_config_dir().unwrap_or_else(|_| std::env::current_dir().unwrap()); let cfg = cfg_loader::load_or_init_at(&base_dir).unwrap_or_default(); app.manage(Arc::new(Mutex::new(cfg)) as SharedConfig); app.manage::<ConfigBaseDir>(base_dir); Ok(()) });
    builder.run(tauri::generate_context!()).expect("error while running tauri application");
}
