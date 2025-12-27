use serde::Serialize;

/// AppHandle wrapper that provides a unified interface for event emission.
///
/// With `tauri-core` feature: wraps `tauri::AppHandle<Wry>` and emits via Tauri
/// Without `tauri-core`: no-op placeholder that silently drops events
#[derive(Clone)]
pub struct AppHandle {
    #[cfg(feature = "tauri-app")]
    inner: tauri::AppHandle,
}

impl AppHandle {
    /// Create from a real Tauri AppHandle
    #[cfg(feature = "tauri-app")]
    pub fn from_tauri(handle: tauri::AppHandle) -> Self {
        Self { inner: handle }
    }

    /// Create from anything (placeholder for non-tauri-app modes)
    #[cfg(not(feature = "tauri-app"))]
    pub fn from_tauri<T>(_handle: T) -> Self {
        Self {}
    }

    /// Get the inner Tauri handle (only available with tauri-app feature)
    #[cfg(feature = "tauri-app")]
    pub fn inner(&self) -> &tauri::AppHandle {
        &self.inner
    }
}

/// Emit an event to all listeners.
///
/// With `tauri-app`: emits via Tauri's event system
/// Otherwise: silently drops the event (structured event bus handles test assertions)
#[cfg(feature = "tauri-app")]
pub fn emit_all<T: Serialize + Clone>(app: &AppHandle, event: &str, payload: &T) {
    use tauri::Emitter;
    if let Err(e) = app.inner.emit(event, payload.clone()) {
        tracing::warn!(target = "event", "emit failed: {} -> {:?}", event, e);
    }
}

#[cfg(not(feature = "tauri-app"))]
pub fn emit_all<T: Serialize + Clone>(_app: &AppHandle, _event: &str, _payload: &T) {
    // Silently drop - structured event bus handles test assertions
}
