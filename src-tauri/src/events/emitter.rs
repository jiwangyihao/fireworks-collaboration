use serde::Serialize;
use tauri::AppHandle;
use tauri::Emitter; // 引入 trait 才能调用 emit

pub fn emit_all<T: Serialize + Clone>(app: &AppHandle, event: &str, payload: &T) {
    if let Err(e) = app.emit(event, payload.clone()) {
        tracing::warn!(target = "event", "emit failed: {} -> {:?}", event, e);
    }
}
