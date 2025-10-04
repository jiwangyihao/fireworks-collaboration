//! 工作区核心数据模型
//!
//! 定义工作区、仓库条目等核心数据结构

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// 工作区结构
///
/// 表示一个包含多个相关仓库的工作区
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Workspace {
    /// 工作区名称
    pub name: String,
    /// 工作区描述
    pub description: Option<String>,
    /// 工作区根路径
    pub root_path: PathBuf,
    /// 仓库列表
    pub repositories: Vec<RepositoryEntry>,
    /// 创建时间（RFC3339 格式）
    pub created_at: String,
    /// 最后修改时间（RFC3339 格式）
    pub updated_at: String,
    /// 自定义元数据
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

/// 仓库条目
///
/// 表示工作区中的一个仓库
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RepositoryEntry {
    /// 仓库唯一标识（在工作区内）
    pub id: String,
    /// 仓库名称
    pub name: String,
    /// 仓库本地路径（相对于工作区根路径）
    pub path: PathBuf,
    /// 远程仓库 URL
    pub remote_url: String,
    /// 默认分支
    #[serde(default = "default_branch")]
    pub default_branch: String,
    /// 标签列表
    #[serde(default)]
    pub tags: Vec<String>,
    /// 是否启用
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    /// 是否包含子模块
    #[serde(default)]
    pub has_submodules: bool,
    /// 自定义配置（可继承工作区配置）
    #[serde(default)]
    pub custom_config: HashMap<String, serde_json::Value>,
}

fn default_branch() -> String {
    "main".to_string()
}

fn default_enabled() -> bool {
    true
}

/// 工作区配置
///
/// 全局工作区功能配置（来自 config.json）
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkspaceConfig {
    /// 是否启用工作区功能
    #[serde(default = "default_workspace_enabled")]
    pub enabled: bool,
    /// 最大并发仓库操作数
    #[serde(default = "default_max_concurrent_repos")]
    pub max_concurrent_repos: usize,
    /// 默认工作区模板名称
    pub default_template: Option<String>,
    /// 工作区配置文件路径
    pub workspace_file: Option<PathBuf>,
}

fn default_workspace_enabled() -> bool {
    false // 默认不启用，保持向后兼容
}

fn default_max_concurrent_repos() -> usize {
    3 // 保守的默认并发数
}

impl Default for WorkspaceConfig {
    fn default() -> Self {
        Self {
            enabled: default_workspace_enabled(),
            max_concurrent_repos: default_max_concurrent_repos(),
            default_template: None,
            workspace_file: None,
        }
    }
}

impl Workspace {
    /// 创建新工作区
    pub fn new(name: String, root_path: PathBuf) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Self {
            name,
            description: None,
            root_path,
            repositories: Vec::new(),
            created_at: now.clone(),
            updated_at: now,
            metadata: HashMap::new(),
        }
    }

    /// 添加仓库
    pub fn add_repository(&mut self, repo: RepositoryEntry) -> Result<(), String> {
        // 检查 ID 是否已存在
        if self.repositories.iter().any(|r| r.id == repo.id) {
            return Err(format!("仓库 ID '{}' 已存在", repo.id));
        }
        self.repositories.push(repo);
        self.update_timestamp();
        Ok(())
    }

    /// 移除仓库
    pub fn remove_repository(&mut self, id: &str) -> Result<RepositoryEntry, String> {
        let index = self
            .repositories
            .iter()
            .position(|r| r.id == id)
            .ok_or_else(|| format!("仓库 ID '{id}' 不存在"))?;
        let repo = self.repositories.remove(index);
        self.update_timestamp();
        Ok(repo)
    }

    /// 获取仓库
    pub fn get_repository(&self, id: &str) -> Option<&RepositoryEntry> {
        self.repositories.iter().find(|r| r.id == id)
    }

    /// 获取可变仓库引用
    pub fn get_repository_mut(&mut self, id: &str) -> Option<&mut RepositoryEntry> {
        self.repositories.iter_mut().find(|r| r.id == id)
    }

    /// 获取所有仓库
    pub fn get_repositories(&self) -> &[RepositoryEntry] {
        &self.repositories
    }

    /// 获取启用的仓库
    pub fn get_enabled_repositories(&self) -> Vec<&RepositoryEntry> {
        self.repositories.iter().filter(|r| r.enabled).collect()
    }

    /// 更新时间戳
    fn update_timestamp(&mut self) {
        self.updated_at = chrono::Utc::now().to_rfc3339();
    }

    /// 更新仓库
    pub fn update_repository(&mut self, id: &str, updater: impl FnOnce(&mut RepositoryEntry)) -> Result<(), String> {
        let repo = self
            .get_repository_mut(id)
            .ok_or_else(|| format!("仓库 ID '{id}' 不存在"))?;
        updater(repo);
        self.update_timestamp();
        Ok(())
    }

    /// 兼容旧的 WorkspaceManager 接口，返回自身引用
    pub fn get_workspace(&self) -> &Self {
        self
    }

    /// 兼容旧的 WorkspaceManager 接口，返回自身可变引用
    pub fn get_workspace_mut(&mut self) -> &mut Self {
        self
    }
}

impl RepositoryEntry {
    /// 创建新仓库条目
    pub fn new(id: String, name: String, path: PathBuf, remote_url: String) -> Self {
        Self {
            id,
            name,
            path,
            remote_url,
            default_branch: default_branch(),
            tags: Vec::new(),
            enabled: default_enabled(),
            has_submodules: false,
            custom_config: HashMap::new(),
        }
    }

    /// 添加标签
    pub fn add_tag(&mut self, tag: String) {
        if !self.tags.contains(&tag) {
            self.tags.push(tag);
        }
    }

    /// 移除标签
    pub fn remove_tag(&mut self, tag: &str) {
        self.tags.retain(|t| t != tag);
    }

    /// 检查是否有标签
    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.contains(&tag.to_string())
    }
}
