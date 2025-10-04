//! 子模块操作实现
//!
//! 实现子模块的初始化、更新、同步等核心操作

use super::model::{SubmoduleConfig, SubmoduleInfo};
use git2::{Repository, SubmoduleUpdateOptions};
use std::path::Path;
use tracing::{error, info, warn};

/// 子模块操作结果
pub type SubmoduleResult<T> = Result<T, SubmoduleError>;

/// 子模块错误类型
#[derive(Debug, thiserror::Error)]
pub enum SubmoduleError {
    #[error("Git error: {0}")]
    Git(#[from] git2::Error),

    #[error("Repository not found at path: {0}")]
    RepositoryNotFound(String),

    #[error("Submodule not found: {0}")]
    SubmoduleNotFound(String),

    #[error("Max recursion depth exceeded: {0}")]
    MaxDepthExceeded(u32),

    #[error("Invalid submodule configuration: {0}")]
    InvalidConfig(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

impl SubmoduleError {
    pub fn category(&self) -> &'static str {
        match self {
            Self::Git(_) => "Git",
            Self::RepositoryNotFound(_) => "NotFound",
            Self::SubmoduleNotFound(_) => "NotFound",
            Self::MaxDepthExceeded(_) => "Limit",
            Self::InvalidConfig(_) => "Config",
            Self::Io(_) => "Io",
        }
    }
}

/// 子模块操作管理器
pub struct SubmoduleManager {
    config: SubmoduleConfig,
}

impl SubmoduleManager {
    /// 创建新的子模块管理器
    pub fn new(config: SubmoduleConfig) -> Self {
        Self { config }
    }

    /// 列出仓库中的所有子模块
    pub fn list_submodules<P: AsRef<Path>>(
        &self,
        repo_path: P,
    ) -> SubmoduleResult<Vec<SubmoduleInfo>> {
        let repo_path = repo_path.as_ref();
        info!(target: "submodule", "Listing submodules in repository: {}", repo_path.display());

        let repo = Repository::open(repo_path).map_err(|e| {
            error!(target: "submodule", "Failed to open repository: {}", e);
            SubmoduleError::RepositoryNotFound(repo_path.display().to_string())
        })?;

        let mut submodules = Vec::new();

        for submodule in repo.submodules()? {
            let name = submodule.name().unwrap_or("unknown").to_string();
            let path = submodule.path().to_path_buf();
            let url = submodule.url().unwrap_or("").to_string();
            let head_id = submodule.head_id().map(|oid| oid.to_string());
            let branch = submodule.branch().map(|s| s.to_string());

            // 检查子模块是否已克隆（子模块目录存在）
            let cloned = if let Some(path) = submodule.path().parent() {
                repo_path
                    .join(path)
                    .join(submodule.path().file_name().unwrap_or_default())
                    .exists()
            } else {
                false
            };

            let info = SubmoduleInfo {
                name,
                path,
                url,
                head_id,
                branch,
                initialized: submodule.open().is_ok(),
                cloned,
            };

            submodules.push(info);
        }

        info!(target: "submodule", "Found {} submodules", submodules.len());
        Ok(submodules)
    }

    /// 初始化所有子模块
    pub fn init_all<P: AsRef<Path>>(&self, repo_path: P) -> SubmoduleResult<Vec<String>> {
        let repo_path = repo_path.as_ref();
        info!(target: "submodule", "Initializing all submodules in: {}", repo_path.display());

        let repo = Repository::open(repo_path)
            .map_err(|_e| SubmoduleError::RepositoryNotFound(repo_path.display().to_string()))?;

        let mut initialized = Vec::new();

        for mut submodule in repo.submodules()? {
            let name = submodule.name().unwrap_or("unknown").to_string();

            match submodule.init(false) {
                Ok(_) => {
                    info!(target: "submodule", "Initialized submodule: {}", name);
                    initialized.push(name);
                }
                Err(e) => {
                    warn!(target: "submodule", "Failed to initialize submodule {}: {}", name, e);
                }
            }
        }

        Ok(initialized)
    }

    /// 初始化指定子模块
    pub fn init<P: AsRef<Path>>(&self, repo_path: P, submodule_name: &str) -> SubmoduleResult<()> {
        let repo_path = repo_path.as_ref();
        info!(target: "submodule", "Initializing submodule '{}' in: {}", submodule_name, repo_path.display());

        let repo = Repository::open(repo_path)
            .map_err(|_e| SubmoduleError::RepositoryNotFound(repo_path.display().to_string()))?;

        let mut submodule = repo
            .find_submodule(submodule_name)
            .map_err(|_| SubmoduleError::SubmoduleNotFound(submodule_name.to_string()))?;

        submodule.init(false)?;
        info!(target: "submodule", "Successfully initialized submodule: {}", submodule_name);
        Ok(())
    }

    /// 更新所有子模块
    pub fn update_all<P: AsRef<Path>>(
        &self,
        repo_path: P,
        depth: u32,
    ) -> SubmoduleResult<Vec<String>> {
        let repo_path = repo_path.as_ref();

        if depth >= self.config.max_depth {
            return Err(SubmoduleError::MaxDepthExceeded(depth));
        }

        info!(target: "submodule", "Updating all submodules in: {} (depth: {})", repo_path.display(), depth);

        let repo = Repository::open(repo_path)
            .map_err(|_e| SubmoduleError::RepositoryNotFound(repo_path.display().to_string()))?;

        let mut updated = Vec::new();
        let mut update_opts = SubmoduleUpdateOptions::new();

        for mut submodule in repo.submodules()? {
            let name = submodule.name().unwrap_or("unknown").to_string();

            match submodule.update(true, Some(&mut update_opts)) {
                Ok(_) => {
                    info!(target: "submodule", "Updated submodule: {}", name);
                    updated.push(name.clone());

                    // 递归更新子模块的子模块
                    if self.config.recursive_update {
                        let submodule_path = repo_path.join(submodule.path());
                        if submodule_path.exists() {
                            if let Err(e) = self.update_all(&submodule_path, depth + 1) {
                                warn!(target: "submodule", "Failed to recursively update submodule {}: {}", name, e);
                            }
                        }
                    }
                }
                Err(e) => {
                    warn!(target: "submodule", "Failed to update submodule {}: {}", name, e);
                }
            }
        }

        Ok(updated)
    }

    /// 更新指定子模块
    pub fn update<P: AsRef<Path>>(
        &self,
        repo_path: P,
        submodule_name: &str,
    ) -> SubmoduleResult<()> {
        let repo_path = repo_path.as_ref();
        info!(target: "submodule", "Updating submodule '{}' in: {}", submodule_name, repo_path.display());

        let repo = Repository::open(repo_path)
            .map_err(|_e| SubmoduleError::RepositoryNotFound(repo_path.display().to_string()))?;

        let mut submodule = repo
            .find_submodule(submodule_name)
            .map_err(|_| SubmoduleError::SubmoduleNotFound(submodule_name.to_string()))?;

        let mut update_opts = SubmoduleUpdateOptions::new();
        submodule.update(true, Some(&mut update_opts))?;

        info!(target: "submodule", "Successfully updated submodule: {}", submodule_name);
        Ok(())
    }

    /// 同步所有子模块的 URL
    pub fn sync_all<P: AsRef<Path>>(&self, repo_path: P) -> SubmoduleResult<Vec<String>> {
        let repo_path = repo_path.as_ref();
        info!(target: "submodule", "Syncing all submodules in: {}", repo_path.display());

        let repo = Repository::open(repo_path)
            .map_err(|_e| SubmoduleError::RepositoryNotFound(repo_path.display().to_string()))?;

        let mut synced = Vec::new();

        for mut submodule in repo.submodules()? {
            let name = submodule.name().unwrap_or("unknown").to_string();

            match submodule.sync() {
                Ok(_) => {
                    info!(target: "submodule", "Synced submodule: {}", name);
                    synced.push(name);
                }
                Err(e) => {
                    warn!(target: "submodule", "Failed to sync submodule {}: {}", name, e);
                }
            }
        }

        Ok(synced)
    }

    /// 同步指定子模块的 URL
    pub fn sync<P: AsRef<Path>>(&self, repo_path: P, submodule_name: &str) -> SubmoduleResult<()> {
        let repo_path = repo_path.as_ref();
        info!(target: "submodule", "Syncing submodule '{}' in: {}", submodule_name, repo_path.display());

        let repo = Repository::open(repo_path)
            .map_err(|_e| SubmoduleError::RepositoryNotFound(repo_path.display().to_string()))?;

        let mut submodule = repo
            .find_submodule(submodule_name)
            .map_err(|_| SubmoduleError::SubmoduleNotFound(submodule_name.to_string()))?;

        submodule.sync()?;

        info!(target: "submodule", "Successfully synced submodule: {}", submodule_name);
        Ok(())
    }

    /// 检查仓库是否有子模块
    pub fn has_submodules<P: AsRef<Path>>(&self, repo_path: P) -> SubmoduleResult<bool> {
        let repo_path = repo_path.as_ref();

        let repo = Repository::open(repo_path)
            .map_err(|_e| SubmoduleError::RepositoryNotFound(repo_path.display().to_string()))?;

        let submodules = repo.submodules()?;
        Ok(!submodules.is_empty())
    }

    /// 获取配置
    pub fn config(&self) -> &SubmoduleConfig {
        &self.config
    }
}
