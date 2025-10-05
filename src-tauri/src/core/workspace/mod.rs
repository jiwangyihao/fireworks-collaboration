//! 工作区管理模块
//!
//! 提供多仓库管理的工作区功能，包括：
//! - 工作区数据模型
//! - 工作区配置管理
//! - 工作区存储管理
//!
//! 本模块为 P7.0 阶段的基础架构，为后续批量操作、子模块支持等功能提供基础。

pub mod config;
pub mod model;
pub mod status;
pub mod storage;

// 重新导出常用类型
pub use config::{PartialWorkspaceConfig, WorkspaceConfigManager};
pub use model::{RepositoryEntry, Workspace, WorkspaceConfig};
pub use status::{
    RepositoryStatus, StatusFilter, StatusQuery, StatusSort, StatusSortDirection, StatusSortField,
    SyncState, WorkingTreeState, WorkspaceStatusResponse, WorkspaceStatusService,
};
pub use storage::WorkspaceStorage;

/// 工作区管理器
///
/// 提供工作区的统一管理接口，整合配置和存储功能
pub struct WorkspaceManager {
    config_manager: WorkspaceConfigManager,
    storage: WorkspaceStorage,
    current_workspace: Option<Workspace>,
}

impl WorkspaceManager {
    /// 创建新的工作区管理器
    pub fn new(config: WorkspaceConfig, storage_path: std::path::PathBuf) -> Self {
        Self {
            config_manager: WorkspaceConfigManager::new(config),
            storage: WorkspaceStorage::new(storage_path),
            current_workspace: None,
        }
    }

    /// 获取配置管理器
    pub fn config_manager(&self) -> &WorkspaceConfigManager {
        &self.config_manager
    }

    /// 获取可变配置管理器
    pub fn config_manager_mut(&mut self) -> &mut WorkspaceConfigManager {
        &mut self.config_manager
    }

    /// 获取存储管理器
    pub fn storage(&self) -> &WorkspaceStorage {
        &self.storage
    }

    /// 获取当前工作区
    pub fn current_workspace(&self) -> Option<&Workspace> {
        self.current_workspace.as_ref()
    }

    /// 加载工作区
    pub fn load_workspace(&mut self) -> anyhow::Result<()> {
        if !self.config_manager.is_enabled() {
            anyhow::bail!("工作区功能未启用");
        }

        let workspace = self.storage.load()?;
        self.current_workspace = Some(workspace);
        Ok(())
    }

    /// 保存当前工作区
    pub fn save_workspace(&self) -> anyhow::Result<()> {
        let workspace = self
            .current_workspace
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("没有加载的工作区"))?;

        self.storage.save(workspace)?;
        Ok(())
    }

    /// 创建新工作区
    pub fn create_workspace(
        &mut self,
        name: String,
        root_path: std::path::PathBuf,
    ) -> anyhow::Result<()> {
        if !self.config_manager.is_enabled() {
            anyhow::bail!("工作区功能未启用");
        }

        let workspace = Workspace::new(name, root_path);
        self.current_workspace = Some(workspace);
        self.save_workspace()?;
        Ok(())
    }

    /// 添加仓库到当前工作区
    pub fn add_repository(&mut self, repo: RepositoryEntry) -> anyhow::Result<()> {
        let workspace = self
            .current_workspace
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("没有加载的工作区"))?;

        workspace
            .add_repository(repo)
            .map_err(|e| anyhow::anyhow!(e))?;
        Ok(())
    }

    /// 从当前工作区移除仓库
    pub fn remove_repository(&mut self, id: &str) -> anyhow::Result<RepositoryEntry> {
        let workspace = self
            .current_workspace
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("没有加载的工作区"))?;

        workspace
            .remove_repository(id)
            .map_err(|e| anyhow::anyhow!(e))
    }

    /// 获取仓库
    pub fn get_repository(&self, id: &str) -> anyhow::Result<&RepositoryEntry> {
        let workspace = self
            .current_workspace
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("没有加载的工作区"))?;

        workspace
            .get_repository(id)
            .ok_or_else(|| anyhow::anyhow!("仓库 ID '{}' 不存在", id))
    }

    /// 获取所有仓库
    pub fn get_repositories(&self) -> anyhow::Result<&[RepositoryEntry]> {
        let workspace = self
            .current_workspace
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("没有加载的工作区"))?;

        Ok(workspace.get_repositories())
    }

    /// 获取启用的仓库
    pub fn get_enabled_repositories(&self) -> anyhow::Result<Vec<&RepositoryEntry>> {
        let workspace = self
            .current_workspace
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("没有加载的工作区"))?;

        Ok(workspace.get_enabled_repositories())
    }

    /// 关闭当前工作区
    pub fn close_workspace(&mut self) {
        self.current_workspace = None;
    }

    /// 检查工作区是否加载
    pub fn is_workspace_loaded(&self) -> bool {
        self.current_workspace.is_some()
    }
}
