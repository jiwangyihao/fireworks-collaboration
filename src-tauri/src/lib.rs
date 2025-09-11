// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
use std::sync::{Arc, Mutex};
use std::io::prelude::*;
use std::net::{TcpListener, TcpStream};
use std::thread;
use tauri::State;
use tauri::Manager;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

mod logging;
mod events;
mod core;

use core::config::{loader as cfg_loader, model::AppConfig};

type SharedConfig = Arc<Mutex<AppConfig>>;
// 新增：配置基目录（由 Tauri 的 app_config_dir 决定）
type ConfigBaseDir = PathBuf;

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct OAuthCallbackData {
    pub code: Option<String>,
    pub state: Option<String>,
    pub error: Option<String>,
    pub error_description: Option<String>,
}

// 系统代理结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemProxy {
    pub enabled: bool,
    pub host: String,
    pub port: u16,
    pub bypass: String,
}

impl Default for SystemProxy {
    fn default() -> Self {
        Self {
            enabled: false,
            host: String::new(),
            port: 0,
            bypass: String::new(),
        }
    }
}

// 全局状态管理 OAuth 回调数据
type OAuthState = Arc<Mutex<Option<OAuthCallbackData>>>;

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

// 新增：获取配置
#[tauri::command]
async fn get_config(cfg: State<'_, SharedConfig>) -> Result<AppConfig, String> {
    cfg.lock().map(|c| c.clone()).map_err(|e| e.to_string())
}

// 新增：设置并保存配置
#[tauri::command]
#[allow(non_snake_case)]
async fn set_config(newCfg: AppConfig, cfg: State<'_, SharedConfig>, base: State<'_, ConfigBaseDir>) -> Result<(), String> {
    {
        let mut guard = cfg.lock().map_err(|e| e.to_string())?;
        *guard = newCfg.clone();
    }
    cfg_loader::save_at(&newCfg, &*base).map_err(|e| e.to_string())
}

// 启动简单的 HTTP 服务器来处理 OAuth 回调
#[tauri::command]
async fn start_oauth_server(state: State<'_, OAuthState>) -> Result<String, String> {
    let oauth_state = Arc::clone(&*state);

    thread::spawn(move || {
        if let Ok(listener) = TcpListener::bind("127.0.0.1:3429") {
            println!("OAuth server listening on port 3429");

            for stream in listener.incoming() {
                match stream {
                    Ok(stream) => {
                        let oauth_state = Arc::clone(&oauth_state);
                        thread::spawn(move || {
                            handle_oauth_request(stream, oauth_state);
                        });
                    }
                    Err(_) => {
                        break;
                    }
                }
            }
        } else {
            eprintln!("Failed to bind to port 3429");
        }
    });

    Ok("OAuth server started".to_string())
}

// 处理 OAuth 回调请求
fn handle_oauth_request(mut stream: TcpStream, oauth_state: OAuthState) {
    let mut buffer = [0; 4096]; // 增加缓冲区大小

    match stream.read(&mut buffer) {
        Ok(bytes_read) => {
            // 只处理实际读取的字节
            let request_bytes = &buffer[..bytes_read];

            // 查找HTTP请求行的结束位置（第一个\r\n）
            let request_line_end = request_bytes.iter()
                .position(|&b| b == b'\r')
                .unwrap_or(bytes_read.min(1024)); // 限制请求行长度

            // 只转换请求行部分，避免处理可能的二进制数据
            let request_line_bytes = &request_bytes[..request_line_end];

            // 安全地转换为字符串，替换无效的UTF-8序列
            let request_line = String::from_utf8_lossy(request_line_bytes);

            println!("Full request line: {}", request_line); // 增强调试日志

            if request_line.starts_with("GET /auth/callback") {
                // 解析 URL 参数
                let mut callback_data = OAuthCallbackData {
                    code: None,
                    state: None,
                    error: None,
                    error_description: None,
                };

                if let Some(query_start) = request_line.find('?') {
                    let remaining = &request_line[query_start + 1..];
                    let query_end = remaining.find(' ').unwrap_or(remaining.len());
                    let query = &remaining[..query_end];

                    println!("Full query string: {}", query); // 更详细的调试日志

                    // 分割参数并打印每个参数
                    for param in query.split('&') {
                        println!("Processing parameter: {}", param); // 调试每个参数

                        if let Some(eq_pos) = param.find('=') {
                            let key = &param[..eq_pos];
                            let value = &param[eq_pos + 1..];

                            println!("Key: '{}', Value: '{}'", key, value); // 调试键值对

                            // 更安全的URL解码，忽略错误
                            let decoded_value = urlencoding::decode(value)
                                .map(|s| s.to_string())
                                .unwrap_or_else(|e| {
                                    eprintln!("Failed to decode URL parameter '{}': {}", value, e);
                                    value.to_string()
                                });

                            println!("Decoded value: '{}'", decoded_value); // 调试解码后的值

                            match key {
                                "code" => {
                                    callback_data.code = Some(decoded_value.clone());
                                    println!("Set code: {}", decoded_value);
                                },
                                "state" => {
                                    callback_data.state = Some(decoded_value.clone());
                                    println!("Set state: {}", decoded_value);
                                },
                                "error" => {
                                    callback_data.error = Some(decoded_value.clone());
                                    println!("Set error: {}", decoded_value);
                                },
                                "error_description" => {
                                    callback_data.error_description = Some(decoded_value.clone());
                                    println!("Set error_description: {}", decoded_value);
                                },
                                _ => {
                                    println!("Unknown parameter: {} = {}", key, decoded_value);
                                }
                            }
                        } else {
                            println!("Parameter without '=': {}", param);
                        }
                    }
                } else {
                    println!("No query string found in request");
                }

                // 保存回调数据到全局状态
                if let Ok(mut state) = oauth_state.lock() {
                    *state = Some(callback_data.clone());
                    println!("Final saved callback data: {:?}", callback_data); // 调试最终保存的数据
                }

                // 返回成功页面
                let response_body = if callback_data.error.is_some() {
                    format!(
                        r#"<!DOCTYPE html>
                        <html>
                        <head>
                            <title>GitHub 授权失败</title>
                            <meta charset="utf-8">
                            <style>
                                body {{ font-family: Arial, sans-serif; text-align: center; padding: 50px; background: #f5f5f5; }}
                                .container {{ max-width: 500px; margin: 0 auto; background: white; padding: 30px; border-radius: 10px; box-shadow: 0 2px 10px rgba(0,0,0,0.1); }}
                                .error {{ color: #d32f2f; }}
                            </style>
                        </head>
                        <body>
                            <div class="container">
                                <h1 class="error">授权失败</h1>
                                <p>错误: {}</p>
                                <p>您可以关闭此页面并返回应用程序。</p>
                            </div>
                            <script>
                                setTimeout(() => {{ window.close(); }}, 3000);
                            </script>
                        </body>
                        </html>"#,
                        callback_data.error_description.as_deref().unwrap_or("未知错误")
                    )
                } else {
                    r#"<!DOCTYPE html>
                    <html>
                    <head>
                        <title>GitHub 授权成功</title>
                        <meta charset="utf-8">
                        <style>
                            body { font-family: Arial, sans-serif; text-align: center; padding: 50px; background: #f5f5f5; }
                            .container { max-width: 500px; margin: 0 auto; background: white; padding: 30px; border-radius: 10px; box-shadow: 0 2px 10px rgba(0,0,0,0.1); }
                            .success { color: #2e7d32; }
                        </style>
                    </head>
                    <body>
                        <div class="container">
                            <h1 class="success">授权成功！</h1>
                            <p>您可以关闭此页面并返回应用程序。</p>
                        </div>
                        <script>
                            setTimeout(() => { window.close(); }, 2000);
                        </script>
                    </body>
                    </html>"#.to_string()
                };

                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    response_body.len(),
                    response_body
                );

                let _ = stream.write_all(response.as_bytes());
                let _ = stream.flush();
            } else {
                // 处理其他请求，返回 404
                let not_found_response = "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
                let _ = stream.write_all(not_found_response.as_bytes());
                let _ = stream.flush();
            }
        }
        Err(e) => {
            eprintln!("Failed to read from stream: {}", e);
        }
    }
}

// 获取 OAuth 回调数据
#[tauri::command]
async fn get_oauth_callback_data(state: State<'_, OAuthState>) -> Result<Option<OAuthCallbackData>, String> {
    if let Ok(mut oauth_state) = state.lock() {
        let data = oauth_state.take(); // 获取数据并清空状态

        // 如果有数据，确保所有字符串都是有效的 UTF-8
        if let Some(ref callback_data) = data {
            println!("准备返回回调数据: {:?}", callback_data);

            // 验证所有字符串字段的 UTF-8 有效性
            if let Some(ref code) = callback_data.code {
                if !code.is_ascii() {
                    println!("警告: code 包含非 ASCII 字符: {}", code);
                }
            }
            if let Some(ref state_val) = callback_data.state {
                if !state_val.is_ascii() {
                    println!("警告: state 包含非 ASCII 字符: {}", state_val);
                }
            }
            if let Some(ref error) = callback_data.error {
                if !error.is_ascii() {
                    println!("警告: error 包含非 ASCII 字符: {}", error);
                }
            }
            if let Some(ref error_desc) = callback_data.error_description {
                if !error_desc.is_ascii() {
                    println!("警告: error_description 包含非 ASCII 字符: {}", error_desc);
                }
            }
        }

        Ok(data)
    } else {
        Err("Failed to access OAuth state".to_string())
    }
}

// 清除 OAuth 状态
#[tauri::command]
async fn clear_oauth_state(state: State<'_, OAuthState>) -> Result<(), String> {
    if let Ok(mut oauth_state) = state.lock() {
        *oauth_state = None;
        Ok(())
    } else {
        Err("Failed to clear OAuth state".to_string())
    }
}

// 取消模块封装: 系统代理相关函数直接提升到顶层
#[cfg(windows)]
fn inner_get_system_proxy() -> SystemProxy {
    use std::ptr;
    use winapi::shared::minwindef::{DWORD, HKEY};
    use winapi::um::winnt::{KEY_READ, REG_DWORD, REG_SZ};
    use winapi::um::winreg::{RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY_CURRENT_USER};
    unsafe {
        let key_path = "Software\\Microsoft\\Windows\\CurrentVersion\\Internet Settings\0"
            .encode_utf16()
            .collect::<Vec<u16>>();
        let mut hkey: HKEY = ptr::null_mut();
        if RegOpenKeyExW(HKEY_CURRENT_USER, key_path.as_ptr(), 0, KEY_READ, &mut hkey) != 0 {
            return SystemProxy::default();
        }
        // ProxyEnable
        let proxy_enable_name = "ProxyEnable\0".encode_utf16().collect::<Vec<u16>>();
        let mut proxy_enable: DWORD = 0; let mut buffer_size: DWORD = 4; let mut value_type: DWORD = 0;
        let enable_result = RegQueryValueExW(
            hkey, proxy_enable_name.as_ptr(), ptr::null_mut(), &mut value_type,
            &mut proxy_enable as *mut DWORD as *mut u8, &mut buffer_size);
        if enable_result != 0 || value_type != REG_DWORD || proxy_enable == 0 { RegCloseKey(hkey); return SystemProxy::default(); }
        // ProxyServer
        let proxy_server_name = "ProxyServer\0".encode_utf16().collect::<Vec<u16>>();
        let mut buffer = vec![0u16; 1024]; let mut buffer_size: DWORD = (buffer.len()*2) as DWORD; let mut value_type: DWORD = 0;
        let server_result = RegQueryValueExW(
            hkey, proxy_server_name.as_ptr(), ptr::null_mut(), &mut value_type,
            buffer.as_mut_ptr() as *mut u8, &mut buffer_size);
        let mut proxy_server = String::new();
        if server_result == 0 && value_type == REG_SZ && buffer_size > 0 {
            let end_pos = buffer.iter().position(|&x| x==0).unwrap_or(buffer.len());
            proxy_server = String::from_utf16_lossy(&buffer[..end_pos]);
        }
        // ProxyOverride
        let proxy_override_name = "ProxyOverride\0".encode_utf16().collect::<Vec<u16>>();
        let mut bypass_buffer = vec![0u16; 1024]; let mut bypass_buffer_size: DWORD = (bypass_buffer.len()*2) as DWORD; let mut bypass_value_type: DWORD = 0;
        let override_result = RegQueryValueExW(
            hkey, proxy_override_name.as_ptr(), ptr::null_mut(), &mut bypass_value_type,
            bypass_buffer.as_mut_ptr() as *mut u8, &mut bypass_buffer_size);
        let mut bypass_list = String::new();
        if override_result == 0 && bypass_value_type == REG_SZ && bypass_buffer_size > 0 {
            let end_pos = bypass_buffer.iter().position(|&x| x==0).unwrap_or(bypass_buffer.len());
            bypass_list = String::from_utf16_lossy(&bypass_buffer[..end_pos]);
        }
        RegCloseKey(hkey);
        if !proxy_server.is_empty() {
            let (host, port) = if let Some(colon_pos) = proxy_server.rfind(':') {
                (proxy_server[..colon_pos].to_string(), proxy_server[colon_pos+1..].parse::<u16>().unwrap_or(8080))
            } else { (proxy_server, 8080) };
            return SystemProxy { enabled: true, host, port, bypass: bypass_list };
        }
        SystemProxy::default()
    }
}

#[cfg(not(windows))]
fn inner_get_system_proxy() -> SystemProxy { SystemProxy::default() }

#[tauri::command]
fn get_system_proxy() -> Result<SystemProxy, String> { Ok(inner_get_system_proxy()) }

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // 初始化日志
    logging::init_logging();

    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_http::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_opener::init())
        .manage(OAuthState::new(Mutex::new(None)))
        .invoke_handler(tauri::generate_handler![
            greet,
            start_oauth_server,
            get_oauth_callback_data,
            clear_oauth_state,
            get_system_proxy,
            get_config,
            set_config,
        ]);

    // 在 setup 中解析系统应用配置目录并加载配置，然后注入全局状态
    builder = builder.setup(|app| {
        let base_dir: PathBuf = app
            .path()
            .app_config_dir()
            .unwrap_or_else(|_| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
        // 确保目录存在并加载/创建配置
        let cfg = cfg_loader::load_or_init_at(&base_dir).unwrap_or_default();
        tracing::info!(target = "config", base = %base_dir.display(), level = %cfg.logging.log_level, "loaded config");
        // 注入状态
        app.manage(Arc::new(Mutex::new(cfg)) as SharedConfig);
        app.manage::<ConfigBaseDir>(base_dir);
        Ok(())
    });

    builder
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
