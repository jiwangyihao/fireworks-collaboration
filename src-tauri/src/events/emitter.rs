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
pub fn emit_all<T: Serialize + Clone>(_app: &AppHandle, _event: &str, _payload: &T) {
    // 仅在无 tauri 环境下静默丢弃（结构化事件总线承担测试断言作用）
}
