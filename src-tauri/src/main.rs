// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(feature = "tauri-app")]
fn main() {
    fireworks_collaboration_lib::app::run();
}

#[cfg(not(feature = "tauri-app"))]
fn main() { /* core tests: no tauri runtime */
}
