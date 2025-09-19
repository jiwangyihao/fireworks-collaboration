use serde::Serialize;

#[cfg(feature = "tauri-app")]
use tauri::Emitter;

#[cfg(feature = "tauri-app")]
pub use tauri::AppHandle;

#[cfg(not(feature = "tauri-app"))]
#[derive(Clone)]
pub struct AppHandle; // 空占位

#[cfg(feature = "tauri-app")]
pub fn emit_all<T: Serialize + Clone>(app: &AppHandle, event: &str, payload: &T) {
    if let Err(e) = app.emit(event, payload.clone()) {
        tracing::warn!(target = "event", "emit failed: {} -> {:?}", event, e);
    }
}

#[cfg(not(feature = "tauri-app"))]
use once_cell::sync::Lazy;
#[cfg(not(feature = "tauri-app"))]
use std::sync::Mutex;

#[cfg(not(feature = "tauri-app"))]
static CAPTURED: Lazy<Mutex<Vec<(String, String)>>> = Lazy::new(|| Mutex::new(Vec::new()));

#[cfg(not(feature = "tauri-app"))]
pub fn emit_all<T: Serialize + Clone>(_app: &AppHandle, _event: &str, _payload: &T) {
    let json = serde_json::to_string(_payload).unwrap_or_else(|_| "<serialize-failed>".into());
    if let Ok(mut v) = CAPTURED.lock() { v.push((_event.to_string(), json)); }
}

#[cfg(not(feature = "tauri-app"))]
pub fn drain_captured_events() -> Vec<(String, String)> {
    if let Ok(mut v) = CAPTURED.lock() { let out = v.clone(); v.clear(); out } else { Vec::new() }
}

#[cfg(not(feature = "tauri-app"))]
pub fn peek_captured_events() -> Vec<(String, String)> {
    if let Ok(v) = CAPTURED.lock() { v.clone() } else { Vec::new() }
}
