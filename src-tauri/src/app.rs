#![cfg(feature = "tauri-app")]
use std::{sync::{Arc, Mutex}, io::{Read, Write}, net::{TcpListener, TcpStream}, thread, path::PathBuf};
use serde::{Serialize, Deserialize};
use tauri::{Manager, State};
use crate::core::{config::{loader as cfg_loader, model::AppConfig}, tasks::{TaskRegistry, SharedTaskRegistry, TaskSnapshot, TaskKind}};
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
            get_config,set_config,task_list,task_cancel,task_start_sleep,task_snapshot
        ]);
    builder = builder.setup(|app| { let base_dir: PathBuf = app.path().app_config_dir().unwrap_or_else(|_| std::env::current_dir().unwrap()); let cfg = cfg_loader::load_or_init_at(&base_dir).unwrap_or_default(); app.manage(Arc::new(Mutex::new(cfg)) as SharedConfig); app.manage::<ConfigBaseDir>(base_dir); Ok(()) });
    builder.run(tauri::generate_context!()).expect("error while running tauri application");
}
