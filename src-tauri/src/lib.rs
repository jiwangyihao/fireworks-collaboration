#![allow(dead_code)]
#![deny(unused_imports)]

pub mod core;
pub mod events;
pub mod logging;
#[cfg(feature = "tauri-app")]
pub mod app; // 新增：暴露 app 模块供 main.rs 调用

// 便于测试直接使用任务模块
pub use core::tasks;

// 测试支持（结构化事件断言工具）
pub mod tests_support;
