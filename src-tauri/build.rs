#[cfg(feature = "tauri-app")]
fn main() {
    tauri_build::build();
}

#[cfg(not(feature = "tauri-app"))]
fn main() {
    // 非 tauri 测试模式：跳过 tauri_build
}
