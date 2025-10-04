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
            .ok_or_else(|| format!("仓库 ID '{}' 不存在", id))?;
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
            .ok_or_else(|| format!("仓库 ID '{}' 不存在", id))?;
        updater(repo);
        self.update_timestamp();
        Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_workspace_new() {
        let ws = Workspace::new("test-workspace".to_string(), PathBuf::from("/test"));
        assert_eq!(ws.name, "test-workspace");
        assert_eq!(ws.root_path, PathBuf::from("/test"));
        assert!(ws.repositories.is_empty());
        assert!(ws.created_at.len() > 0);
        assert_eq!(ws.created_at, ws.updated_at);
    }

    #[test]
    fn test_add_repository() {
        let mut ws = Workspace::new("test".to_string(), PathBuf::from("/test"));
        let repo = RepositoryEntry::new(
            "repo1".to_string(),
            "Repo 1".to_string(),
            PathBuf::from("repo1"),
            "https://github.com/test/repo1".to_string(),
        );

        assert!(ws.add_repository(repo.clone()).is_ok());
        assert_eq!(ws.repositories.len(), 1);
        assert_eq!(ws.get_repository("repo1").unwrap().name, "Repo 1");

        // 重复添加应该失败
        assert!(ws.add_repository(repo).is_err());
    }

    #[test]
    fn test_remove_repository() {
        let mut ws = Workspace::new("test".to_string(), PathBuf::from("/test"));
        let repo = RepositoryEntry::new(
            "repo1".to_string(),
            "Repo 1".to_string(),
            PathBuf::from("repo1"),
            "https://github.com/test/repo1".to_string(),
        );

        ws.add_repository(repo).unwrap();
        assert_eq!(ws.repositories.len(), 1);

        let removed = ws.remove_repository("repo1").unwrap();
        assert_eq!(removed.id, "repo1");
        assert_eq!(ws.repositories.len(), 0);

        // 移除不存在的仓库应该失败
        assert!(ws.remove_repository("repo1").is_err());
    }

    #[test]
    fn test_get_enabled_repositories() {
        let mut ws = Workspace::new("test".to_string(), PathBuf::from("/test"));
        
        let mut repo1 = RepositoryEntry::new(
            "repo1".to_string(),
            "Repo 1".to_string(),
            PathBuf::from("repo1"),
            "https://github.com/test/repo1".to_string(),
        );
        repo1.enabled = true;

        let mut repo2 = RepositoryEntry::new(
            "repo2".to_string(),
            "Repo 2".to_string(),
            PathBuf::from("repo2"),
            "https://github.com/test/repo2".to_string(),
        );
        repo2.enabled = false;

        ws.add_repository(repo1).unwrap();
        ws.add_repository(repo2).unwrap();

        let enabled = ws.get_enabled_repositories();
        assert_eq!(enabled.len(), 1);
        assert_eq!(enabled[0].id, "repo1");
    }

    #[test]
    fn test_repository_entry_tags() {
        let mut repo = RepositoryEntry::new(
            "repo1".to_string(),
            "Repo 1".to_string(),
            PathBuf::from("repo1"),
            "https://github.com/test/repo1".to_string(),
        );

        repo.add_tag("frontend".to_string());
        repo.add_tag("critical".to_string());
        assert_eq!(repo.tags.len(), 2);
        assert!(repo.has_tag("frontend"));
        assert!(repo.has_tag("critical"));
        assert!(!repo.has_tag("backend"));

        // 重复添加不应增加标签
        repo.add_tag("frontend".to_string());
        assert_eq!(repo.tags.len(), 2);

        repo.remove_tag("frontend");
        assert_eq!(repo.tags.len(), 1);
        assert!(!repo.has_tag("frontend"));
    }

    #[test]
    fn test_workspace_config_default() {
        let config = WorkspaceConfig::default();
        assert_eq!(config.enabled, false);
        assert_eq!(config.max_concurrent_repos, 3);
        assert!(config.default_template.is_none());
        assert!(config.workspace_file.is_none());
    }

    #[test]
    fn test_serialization() {
        let ws = Workspace::new("test".to_string(), PathBuf::from("/test"));
        let json = serde_json::to_string(&ws).unwrap();
        let deserialized: Workspace = serde_json::from_str(&json).unwrap();
        assert_eq!(ws, deserialized);
    }
}
