//! 代理模块集成测试

#[path = "../common/mod.rs"]
pub(crate) mod common;

mod config;
mod detector;
mod events;
mod http_connector;
mod manager;
mod manager_commands; // P5.6 proxy commands and events tests
mod manager_recovery; // P5.5 proxy recovery tests
mod socks5_connector;
mod state;
mod unit_tests;

#[cfg(feature = "tauri-app")]
mod app_commands_integration;
