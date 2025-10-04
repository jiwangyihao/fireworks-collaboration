//! 子模块核心数据模型
//!
//! 定义子模块信息、配置等核心数据结构

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// 子模块信息
///
/// 表示一个 Git 子模块的详细信息
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SubmoduleInfo {
    /// 子模块名称
    pub name: String,
    /// 子模块路径（相对于父仓库根目录）
    pub path: PathBuf,
    /// 子模块 URL
    pub url: String,
    /// 子模块当前提交 SHA（40 字符十六进制）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub head_id: Option<String>,
    /// 子模块分支（如果有）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    /// 是否已初始化
    pub initialized: bool,
    /// 是否已克隆（工作目录存在）
    pub cloned: bool,
}

/// 子模块配置
///
/// 控制子模块操作的行为
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SubmoduleConfig {
    /// 是否启用子模块自动递归
    #[serde(default = "default_auto_recurse")]
    pub auto_recurse: bool,
    /// 最大递归深度（防止无限递归）
    #[serde(default = "default_max_depth")]
    pub max_depth: u32,
    /// 是否在 clone 时自动初始化子模块
    #[serde(default = "default_auto_init_on_clone")]
    pub auto_init_on_clone: bool,
    /// 是否在 update 时递归更新
    #[serde(default = "default_recursive_update")]
    pub recursive_update: bool,
    /// 是否并行处理子模块
    #[serde(default = "default_parallel")]
    pub parallel: bool,
    /// 并行处理的最大并发数
    #[serde(default = "default_max_parallel")]
    pub max_parallel: u32,
}

impl Default for SubmoduleConfig {
    fn default() -> Self {
        Self {
            auto_recurse: default_auto_recurse(),
            max_depth: default_max_depth(),
            auto_init_on_clone: default_auto_init_on_clone(),
            recursive_update: default_recursive_update(),
            parallel: default_parallel(),
            max_parallel: default_max_parallel(),
        }
    }
}

fn default_auto_recurse() -> bool {
    true
}

fn default_max_depth() -> u32 {
    5
}

fn default_auto_init_on_clone() -> bool {
    true
}

fn default_recursive_update() -> bool {
    true
}

fn default_parallel() -> bool {
    false
}

fn default_max_parallel() -> u32 {
    3
}

/// 子模块操作类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SubmoduleOperation {
    /// 初始化子模块
    Init,
    /// 更新子模块
    Update,
    /// 同步子模块 URL
    Sync,
    /// 递归克隆
    RecursiveClone,
}

impl SubmoduleOperation {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Init => "init",
            Self::Update => "update",
            Self::Sync => "sync",
            Self::RecursiveClone => "recursive_clone",
        }
    }
}

/// 子模块进度事件
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmoduleProgressEvent {
    /// 父任务 ID（如果是任务系统触发的）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_task_id: Option<uuid::Uuid>,
    /// 子模块名称
    pub submodule_name: String,
    /// 操作类型
    pub operation: SubmoduleOperation,
    /// 当前进度百分比（0-100）
    pub percent: u32,
    /// 当前递归深度
    #[serde(skip_serializing_if = "Option::is_none")]
    pub depth: Option<u32>,
    /// 总子模块数（如果已知）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_submodules: Option<u32>,
    /// 已处理子模块数
    #[serde(skip_serializing_if = "Option::is_none")]
    pub processed_submodules: Option<u32>,
}

/// 子模块错误事件
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmoduleErrorEvent {
    /// 父任务 ID（如果是任务系统触发的）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_task_id: Option<uuid::Uuid>,
    /// 子模块名称
    pub submodule_name: String,
    /// 操作类型
    pub operation: SubmoduleOperation,
    /// 错误分类
    pub category: String,
    /// 错误消息
    pub message: String,
    /// 当前递归深度
    #[serde(skip_serializing_if = "Option::is_none")]
    pub depth: Option<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_submodule_info_creation() {
        let info = SubmoduleInfo {
            name: "test-submodule".to_string(),
            path: PathBuf::from("libs/test"),
            url: "https://github.com/test/repo.git".to_string(),
            head_id: Some("abc123".to_string()),
            branch: Some("main".to_string()),
            initialized: true,
            cloned: true,
        };
        
        assert_eq!(info.name, "test-submodule");
        assert_eq!(info.path, PathBuf::from("libs/test"));
        assert_eq!(info.url, "https://github.com/test/repo.git");
        assert!(info.initialized);
        assert!(info.cloned);
    }

    #[test]
    fn test_submodule_config_defaults() {
        let config = SubmoduleConfig::default();
        
        assert!(config.auto_recurse);
        assert_eq!(config.max_depth, 5);
        assert!(config.auto_init_on_clone);
        assert!(config.recursive_update);
        assert!(!config.parallel);
        assert_eq!(config.max_parallel, 3);
    }

    #[test]
    fn test_submodule_config_serde() {
        let config = SubmoduleConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: SubmoduleConfig = serde_json::from_str(&json).unwrap();
        
        assert_eq!(config, deserialized);
    }

    #[test]
    fn test_submodule_operation_as_str() {
        assert_eq!(SubmoduleOperation::Init.as_str(), "init");
        assert_eq!(SubmoduleOperation::Update.as_str(), "update");
        assert_eq!(SubmoduleOperation::Sync.as_str(), "sync");
        assert_eq!(SubmoduleOperation::RecursiveClone.as_str(), "recursive_clone");
    }

    #[test]
    fn test_submodule_progress_event_serde() {
        let event = SubmoduleProgressEvent {
            parent_task_id: Some(uuid::Uuid::new_v4()),
            submodule_name: "test".to_string(),
            operation: SubmoduleOperation::Update,
            percent: 50,
            depth: Some(1),
            total_submodules: Some(5),
            processed_submodules: Some(2),
        };
        
        let json = serde_json::to_string(&event).unwrap();
        let deserialized: SubmoduleProgressEvent = serde_json::from_str(&json).unwrap();
        
        assert_eq!(event.submodule_name, deserialized.submodule_name);
        assert_eq!(event.percent, deserialized.percent);
    }
}
