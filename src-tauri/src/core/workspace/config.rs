//! 工作区配置管理
//!
//! 负责工作区配置的加载、解析、验证和热更新

use super::model::WorkspaceConfig;
use anyhow::Result;
use std::path::PathBuf;
use tracing::{debug, info, warn};

/// 工作区配置管理器
pub struct WorkspaceConfigManager {
    config: WorkspaceConfig,
}

impl WorkspaceConfigManager {
    /// 创建新的配置管理器
    pub fn new(config: WorkspaceConfig) -> Self {
        Self { config }
    }

    /// 从默认值创建
    pub fn with_defaults() -> Self {
        Self {
            config: WorkspaceConfig::default(),
        }
    }

    /// 获取配置
    pub fn config(&self) -> &WorkspaceConfig {
        &self.config
    }

    /// 更新配置
    pub fn update_config(&mut self, config: WorkspaceConfig) -> Result<()> {
        self.validate_config(&config)?;
        info!("更新工作区配置: enabled={}, max_concurrent={}", 
              config.enabled, config.max_concurrent_repos);
        self.config = config;
        Ok(())
    }

    /// 验证配置
    fn validate_config(&self, config: &WorkspaceConfig) -> Result<()> {
        if config.max_concurrent_repos == 0 {
            anyhow::bail!("max_concurrent_repos 必须大于 0");
        }

        if config.max_concurrent_repos > 100 {
            warn!("max_concurrent_repos 设置较高 ({}), 可能导致资源耗尽", 
                  config.max_concurrent_repos);
        }

        if let Some(ref workspace_file) = config.workspace_file {
            debug!("工作区配置文件路径: {:?}", workspace_file);
        }

        Ok(())
    }

    /// 检查是否启用工作区功能
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// 获取最大并发数
    pub fn max_concurrent_repos(&self) -> usize {
        self.config.max_concurrent_repos
    }

    /// 获取默认模板名称
    pub fn default_template(&self) -> Option<&str> {
        self.config.default_template.as_deref()
    }

    /// 获取工作区文件路径
    pub fn workspace_file(&self) -> Option<&PathBuf> {
        self.config.workspace_file.as_ref()
    }

    /// 设置是否启用
    pub fn set_enabled(&mut self, enabled: bool) {
        info!("设置工作区功能启用状态: {}", enabled);
        self.config.enabled = enabled;
    }

    /// 设置最大并发数
    pub fn set_max_concurrent_repos(&mut self, max: usize) -> Result<()> {
        if max == 0 {
            anyhow::bail!("max_concurrent_repos 必须大于 0");
        }
        info!("设置最大并发仓库数: {}", max);
        self.config.max_concurrent_repos = max;
        Ok(())
    }

    /// 设置默认模板
    pub fn set_default_template(&mut self, template: Option<String>) {
        if let Some(ref t) = template {
            info!("设置默认工作区模板: {}", t);
        } else {
            info!("清除默认工作区模板");
        }
        self.config.default_template = template;
    }

    /// 设置工作区文件路径
    pub fn set_workspace_file(&mut self, path: Option<PathBuf>) {
        if let Some(ref p) = path {
            info!("设置工作区配置文件路径: {:?}", p);
        } else {
            info!("清除工作区配置文件路径");
        }
        self.config.workspace_file = path;
    }

    /// 合并配置（用于热更新）
    pub fn merge_config(&mut self, partial: PartialWorkspaceConfig) -> Result<()> {
        if let Some(enabled) = partial.enabled {
            self.config.enabled = enabled;
        }
        if let Some(max_concurrent) = partial.max_concurrent_repos {
            if max_concurrent == 0 {
                anyhow::bail!("max_concurrent_repos 必须大于 0");
            }
            self.config.max_concurrent_repos = max_concurrent;
        }
        if partial.default_template.is_some() {
            self.config.default_template = partial.default_template;
        }
        if partial.workspace_file.is_some() {
            self.config.workspace_file = partial.workspace_file;
        }

        self.validate_config(&self.config)?;
        info!("合并工作区配置完成");
        Ok(())
    }
}

/// 部分工作区配置（用于热更新）
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct PartialWorkspaceConfig {
    pub enabled: Option<bool>,
    pub max_concurrent_repos: Option<usize>,
    pub default_template: Option<String>,
    pub workspace_file: Option<PathBuf>,
}
