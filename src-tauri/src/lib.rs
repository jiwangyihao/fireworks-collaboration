#![allow(dead_code)]

pub mod core;
pub mod events;
pub mod logging;
#[cfg(feature = "tauri-app")]
pub mod app; // 新增：暴露 app 模块供 main.rs 调用

// 便于测试直接使用任务模块
pub use core::tasks;
