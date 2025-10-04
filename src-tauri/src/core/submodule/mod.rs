//! 子模块管理模块
//!
//! 提供 Git 子模块的初始化、更新、同步等操作

pub mod model;
pub mod operations;

pub use model::{
    SubmoduleConfig, SubmoduleErrorEvent, SubmoduleInfo, SubmoduleOperation,
    SubmoduleProgressEvent,
};
pub use operations::{SubmoduleError, SubmoduleManager, SubmoduleResult};
