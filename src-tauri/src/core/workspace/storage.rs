//! 工作区存储管理
//!
//! 负责工作区数据的持久化，包括读取和写入 workspace.json

use super::model::Workspace;
use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

/// 工作区存储管理器
pub struct WorkspaceStorage {
    /// 存储文件路径
    file_path: PathBuf,
}

impl WorkspaceStorage {
    /// 创建新的存储管理器
    pub fn new(file_path: PathBuf) -> Self {
        Self { file_path }
    }

    /// 从应用数据目录创建
    pub fn from_app_data_dir(app_data_dir: &Path) -> Self {
        let file_path = app_data_dir.join("workspace.json");
        Self { file_path }
    }

    /// 获取存储文件路径
    pub fn file_path(&self) -> &Path {
        &self.file_path
    }

    /// 加载工作区
    pub fn load(&self) -> Result<Workspace> {
        if !self.file_path.exists() {
            anyhow::bail!("工作区配置文件不存在: {:?}", self.file_path);
        }

        let content = fs::read_to_string(&self.file_path)
            .with_context(|| format!("读取工作区配置失败: {:?}", self.file_path))?;

        let workspace: Workspace = serde_json::from_str(&content)
            .with_context(|| format!("解析工作区配置失败: {:?}", self.file_path))?;

        info!("成功加载工作区 '{}', 包含 {} 个仓库", 
              workspace.name, workspace.repositories.len());
        debug!("工作区根路径: {:?}", workspace.root_path);

        Ok(workspace)
    }

    /// 保存工作区
    pub fn save(&self, workspace: &Workspace) -> Result<()> {
        // 确保目录存在
        if let Some(parent) = self.file_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("创建目录失败: {parent:?}"))?;
        }

        // 序列化为 JSON（带格式化）
        let json = serde_json::to_string_pretty(workspace)
            .with_context(|| "序列化工作区配置失败")?;

        // 先写入临时文件，成功后再重命名（原子操作）
        let temp_path = self.file_path.with_extension("json.tmp");
        fs::write(&temp_path, &json)
            .with_context(|| format!("写入临时文件失败: {temp_path:?}"))?;

        fs::rename(&temp_path, &self.file_path)
            .with_context(|| format!("重命名文件失败: {:?} -> {:?}", temp_path, self.file_path))?;

        info!("成功保存工作区 '{}' 到 {:?}", workspace.name, self.file_path);
        Ok(())
    }

    /// 检查工作区配置是否存在
    pub fn exists(&self) -> bool {
        self.file_path.exists()
    }

    /// 删除工作区配置
    pub fn delete(&self) -> Result<()> {
        if !self.file_path.exists() {
            warn!("工作区配置文件不存在，无需删除: {:?}", self.file_path);
            return Ok(());
        }

        fs::remove_file(&self.file_path)
            .with_context(|| format!("删除工作区配置失败: {:?}", self.file_path))?;

        info!("成功删除工作区配置: {:?}", self.file_path);
        Ok(())
    }

    /// 备份工作区配置
    pub fn backup(&self) -> Result<PathBuf> {
        if !self.file_path.exists() {
            anyhow::bail!("工作区配置文件不存在，无法备份: {:?}", self.file_path);
        }

        let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
        let backup_path = self.file_path.with_extension(format!("json.backup.{timestamp}"));

        fs::copy(&self.file_path, &backup_path)
            .with_context(|| format!("备份工作区配置失败: {backup_path:?}"))?;

        info!("成功备份工作区配置到: {:?}", backup_path);
        Ok(backup_path)
    }

    /// 从备份恢复
    pub fn restore_from_backup(&self, backup_path: &Path) -> Result<()> {
        if !backup_path.exists() {
            anyhow::bail!("备份文件不存在: {:?}", backup_path);
        }

        // 验证备份文件是否有效
        let content = fs::read_to_string(backup_path)
            .with_context(|| format!("读取备份文件失败: {backup_path:?}"))?;

        let _workspace: Workspace = serde_json::from_str(&content)
            .with_context(|| format!("备份文件格式无效: {backup_path:?}"))?;

        // 如果当前文件存在，先备份
        if self.file_path.exists() {
            let _ = self.backup(); // 忽略备份失败
        }

        // 复制备份文件到当前位置
        fs::copy(backup_path, &self.file_path)
            .with_context(|| format!("恢复备份失败: {backup_path:?}"))?;

        info!("成功从备份恢复工作区配置: {:?}", backup_path);
        Ok(())
    }

    /// 验证工作区配置格式
    pub fn validate(&self) -> Result<()> {
        if !self.file_path.exists() {
            anyhow::bail!("工作区配置文件不存在: {:?}", self.file_path);
        }

        let content = fs::read_to_string(&self.file_path)
            .with_context(|| format!("读取工作区配置失败: {:?}", self.file_path))?;

        let workspace: Workspace = serde_json::from_str(&content)
            .with_context(|| format!("解析工作区配置失败: {:?}", self.file_path))?;

        // 基本验证
        if workspace.name.is_empty() {
            anyhow::bail!("工作区名称不能为空");
        }

        // 检查仓库 ID 唯一性
        let mut ids = std::collections::HashSet::new();
        for repo in &workspace.repositories {
            if !ids.insert(&repo.id) {
                anyhow::bail!("仓库 ID 重复: {}", repo.id);
            }
        }

        info!("工作区配置验证通过: {}", workspace.name);
        Ok(())
    }
}
