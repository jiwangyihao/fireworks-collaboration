#![allow(dead_code)]
#![deny(unused_imports)]

#[cfg(feature = "tauri-app")]
pub mod app;
pub mod core;
pub mod events;
pub mod logging; // 新增：暴露 app 模块供 main.rs 调用
pub mod soak;

// 便于测试直接使用任务模块
pub use core::tasks;

// （移除）测试支持模块改由集成测试侧 tests/common 提供
