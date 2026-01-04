//! VitePress 命令模块 - 处理 VitePress 项目相关操作
//!
//! 提供以下功能：
//! - 项目检测
//! - 配置解析
//! - 依赖管理
//! - Dev Server 管理
//! - 文档树
//! - 文档 CRUD

use git2;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::{Emitter, Runtime, State, Window};
use tauri_plugin_shell::process::{CommandChild, CommandEvent};
use tauri_plugin_shell::ShellExt;
use tokio::fs;
use tokio::sync::Mutex as AsyncMutex;

// ============================================================================
// 状态管理
// ============================================================================

pub struct DevServerState {
    pub servers: Mutex<HashMap<u32, CommandChild>>,
}

impl Default for DevServerState {
    fn default() -> Self {
        Self {
            servers: Mutex::new(HashMap::new()),
        }
    }
}

// ============================================================================
// 类型定义
// ============================================================================

/// VitePress 项目检测结果
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VitePressDetection {
    /// 是否为 VitePress 项目
    pub is_vitepress: bool,
    /// 配置文件路径（相对于项目根）
    pub config_path: Option<String>,
    /// 内容根目录（srcDir，相对于项目根）
    pub content_root: Option<String>,
    /// package.json 中的项目名
    pub project_name: Option<String>,
}

/// VitePress 配置
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct VitePressConfig {
    /// 站点标题
    pub title: Option<String>,
    /// 站点描述
    pub description: Option<String>,
    /// 语言
    pub lang: Option<String>,
    /// 内容目录（相对路径）
    pub src_dir: Option<String>,
    /// 排除的文件模式
    pub src_exclude: Vec<String>,
    /// 主题配置
    pub theme_config: Option<ThemeConfig>,
}

/// 主题配置
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ThemeConfig {
    /// 导航栏
    pub nav: Option<Vec<NavItem>>,
    /// Logo 路径
    pub logo: Option<String>,
}

/// 导航项
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NavItem {
    pub text: String,
    pub link: Option<String>,
    pub items: Option<Vec<NavItem>>,
}

/// 依赖状态
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DependencyStatus {
    /// 是否已安装
    pub installed: bool,
    /// pnpm-lock.yaml 是否存在
    pub pnpm_lock_exists: bool,
    /// node_modules 是否存在
    pub node_modules_exists: bool,
    /// .pnpm store 是否存在
    pub pnpm_store_exists: bool,
    /// 是否过期（需要重新安装）
    pub outdated: bool,
    /// 包管理器类型
    pub package_manager: String,
}

/// Dev Server 信息
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DevServerInfo {
    /// 访问 URL
    pub url: String,
    /// 端口
    pub port: u16,
    /// 进程 ID
    pub process_id: u32,
    /// 状态
    pub status: DevServerStatus,
}

/// Dev Server 状态
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DevServerStatus {
    Starting,
    Running,
    Stopped,
    Error,
}

/// 文档树节点
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocTreeNode {
    /// 文件/文件夹名
    pub name: String,
    /// 完整路径
    pub path: String,
    /// 节点类型
    pub node_type: DocTreeNodeType,
    /// 显示标题（从 frontmatter 或 # 标题提取）
    pub title: Option<String>,
    /// 子节点
    pub children: Option<Vec<DocTreeNode>>,
    /// Git 状态
    pub git_status: Option<GitFileStatus>,
    /// 排序序号
    pub order: Option<i32>,
}

/// 节点类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum DocTreeNodeType {
    File,
    Folder,
}

/// Git 文件状态
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GitFileStatus {
    Clean,
    Modified,
    Staged,
    Untracked,
    Conflict,
}

/// 文档内容
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentContent {
    /// 文件路径
    pub path: String,
    /// 文件内容
    pub content: String,
    /// Frontmatter（解析后的 YAML）
    pub frontmatter: Option<HashMap<String, serde_json::Value>>,
}

/// 保存结果
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveResult {
    pub success: bool,
    pub path: String,
    pub message: Option<String>,
}

// ============================================================================
// 组件解析相关
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComponentProp {
    pub name: String,
    pub type_name: String,
    pub description: Option<String>,
    pub default: Option<String>,
    pub optional: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VueComponent {
    pub name: String,
    pub path: String,
    pub description: Option<String>,
    pub props: Vec<ComponentProp>,
}

// ============================================================================
// 命令实现
// ============================================================================

/// 检测指定路径是否为 VitePress 项目
#[tauri::command(rename_all = "camelCase")]
pub async fn vitepress_detect_project(path: String) -> Result<VitePressDetection, String> {
    let project_path = Path::new(&path);

    // 检查配置文件是否存在
    let config_candidates = [
        ".vitepress/config.mts",
        ".vitepress/config.mjs",
        ".vitepress/config.ts",
        ".vitepress/config.js",
    ];

    let mut config_path: Option<String> = None;
    for candidate in config_candidates {
        let full_path = project_path.join(candidate);
        if full_path.exists() {
            config_path = Some(candidate.to_string());
            break;
        }
    }

    // 如果没有配置文件，不是 VitePress 项目
    if config_path.is_none() {
        return Ok(VitePressDetection {
            is_vitepress: false,
            config_path: None,
            content_root: None,
            project_name: None,
        });
    }

    // 尝试读取 package.json 获取项目名
    let mut project_name: Option<String> = None;
    let package_json_path = project_path.join("package.json");
    if package_json_path.exists() {
        if let Ok(content) = fs::read_to_string(&package_json_path).await {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                project_name = json
                    .get("name")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
            }
        }
    }

    // 默认内容根目录为 "."，后续可通过配置解析更新
    Ok(VitePressDetection {
        is_vitepress: true,
        config_path,
        content_root: Some(".".to_string()),
        project_name,
    })
}

/// 解析 VitePress 配置
#[tauri::command(rename_all = "camelCase")]
pub async fn vitepress_parse_config<R: Runtime>(
    project_path: String,
    window: Window<R>,
) -> Result<VitePressConfig, String> {
    let project = Path::new(&project_path);

    // 1. 查找配置文件
    let config_candidates = [
        ".vitepress/config.mts",
        ".vitepress/config.mjs",
        ".vitepress/config.ts",
        ".vitepress/config.js",
    ];

    let mut config_path: Option<PathBuf> = None;
    for candidate in config_candidates {
        let full_path = project.join(candidate);
        if full_path.exists() {
            config_path = Some(full_path);
            break;
        }
    }

    if config_path.is_none() {
        return Ok(VitePressConfig::default());
    }
    let config_abs_path = config_path
        .unwrap()
        .canonicalize()
        .map_err(|e| e.to_string())?;

    // 2. 生成临时解析脚本 (Node.js)
    let temp_dir = std::env::temp_dir();
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let script_path = temp_dir.join(format!("vitepress_parser_{}.mjs", timestamp));

    // 处理 Windows UNC 路径 (\\?\)
    // canonicalize 会生成 \\?\C:\... 格式，Node.js 的 ESM loader 在处理 file:// URL 时可能无法正确解析包含 unc 前缀的路径
    let mut path_str = config_abs_path.to_string_lossy().to_string();
    if path_str.starts_with(r"\\?\") {
        path_str = path_str[4..].to_string();
    }

    // 注意：Windows 路径需要转换为 file URL 格式，且反斜杠需处理
    let config_url = format!("file://{}", path_str.replace("\\", "/"));

    let script_content = format!(
        r#"
import config from '{}';
try {{
    const resolved = config.default || config;
    const result = {{
        title: resolved.title,
        description: resolved.description,
        lang: resolved.lang,
        srcDir: resolved.srcDir,
        srcExclude: resolved.srcExclude || [],
        themeConfig: resolved.themeConfig ? {{
            nav: resolved.themeConfig.nav,
            logo: resolved.themeConfig.logo
        }} : undefined
    }};
    console.log(JSON.stringify(result));
}} catch (e) {{
    console.error(e);
    process.exit(1);
}}
"#,
        config_url
    );

    fs::write(&script_path, script_content)
        .await
        .map_err(|e| format!("Failed to write temp script: {}", e))?;

    // 3. 执行脚本
    let output = window
        .shell()
        .command("node")
        .args([script_path.to_string_lossy().to_string()])
        .current_dir(project)
        .output()
        .await
        .map_err(|e| format!("Failed to execute node script: {}", e))?;

    // 清理临时文件 (不阻塞)
    let _ = fs::remove_file(&script_path).await;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Config parsing failed: {}", stderr));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let config: VitePressConfig = serde_json::from_str(&stdout)
        .map_err(|e| format!("Failed to parse config JSON: {} (stdout: {})", e, stdout))?;

    Ok(config)
}

/// 检查项目依赖状态
#[tauri::command(rename_all = "camelCase")]
pub async fn vitepress_check_dependencies(
    project_path: String,
) -> Result<DependencyStatus, String> {
    let project = Path::new(&project_path);

    let pnpm_lock = project.join("pnpm-lock.yaml");
    let node_modules = project.join("node_modules");
    let pnpm_store = node_modules.join(".pnpm");

    let pnpm_lock_exists = pnpm_lock.exists();
    let node_modules_exists = node_modules.exists();
    let pnpm_store_exists = pnpm_store.exists();

    // 简单判断：node_modules/.pnpm 存在则认为已安装
    let installed = pnpm_lock_exists && pnpm_store_exists;

    Ok(DependencyStatus {
        installed,
        pnpm_lock_exists,
        node_modules_exists,
        pnpm_store_exists,
        outdated: false, // TODO: 更精确的过期检测
        package_manager: "pnpm".to_string(),
    })
}

/// 获取文档树
#[tauri::command(rename_all = "camelCase")]
pub async fn vitepress_get_doc_tree(
    project_path: String,
    content_root: Option<String>,
) -> Result<DocTreeNode, String> {
    let project = Path::new(&project_path);
    let root_dir = content_root.unwrap_or_else(|| ".".to_string());
    let content_path = project.join(&root_dir);

    if !content_path.exists() {
        return Err(format!(
            "Content root not found: {}",
            content_path.display()
        ));
    }

    // 递归构建文档树
    let git_map = get_git_status_map(project);
    build_doc_tree(&content_path, &project_path, &git_map).await
}

/// 获取 Git 状态 Map
fn get_git_status_map(project_path: &Path) -> HashMap<String, GitFileStatus> {
    let mut map = HashMap::new();
    if let Ok(repo) = git2::Repository::open(project_path) {
        if let Ok(statuses) = repo.statuses(None) {
            for entry in statuses.iter() {
                let status = entry.status();
                let path = entry.path().unwrap_or("").to_string();

                // 将相对路径转换为绝对路径以便匹配
                let absolute_path = project_path.join(&path).to_string_lossy().to_string();

                let file_status = if status.contains(git2::Status::CONFLICTED) {
                    GitFileStatus::Conflict
                } else if status.intersects(
                    git2::Status::INDEX_NEW
                        | git2::Status::INDEX_MODIFIED
                        | git2::Status::INDEX_DELETED
                        | git2::Status::INDEX_RENAMED
                        | git2::Status::INDEX_TYPECHANGE,
                ) {
                    GitFileStatus::Staged
                } else if status.contains(git2::Status::WT_NEW) {
                    GitFileStatus::Untracked
                } else if status.intersects(
                    git2::Status::WT_MODIFIED
                        | git2::Status::WT_DELETED
                        | git2::Status::WT_TYPECHANGE
                        | git2::Status::WT_RENAMED,
                ) {
                    GitFileStatus::Modified
                } else {
                    GitFileStatus::Clean
                };

                if matches!(file_status, GitFileStatus::Clean) {
                    continue;
                }

                map.insert(absolute_path, file_status);
            }
        }
    }
    map
}

/// 递归构建文档树（使用 Box::pin 解决异步递归问题）
fn build_doc_tree<'a>(
    dir_path: &'a Path,
    project_root: &'a str,
    git_map: &'a HashMap<String, GitFileStatus>,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<DocTreeNode, String>> + Send + 'a>> {
    Box::pin(async move {
        let name = dir_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "root".to_string());

        let path = dir_path.to_string_lossy().to_string();

        let mut children = Vec::new();

        // 读取目录内容
        let mut entries = fs::read_dir(dir_path)
            .await
            .map_err(|e| format!("Failed to read directory: {}", e))?;

        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| format!("Failed to read entry: {}", e))?
        {
            let entry_path = entry.path();
            let entry_name = entry.file_name().to_string_lossy().to_string();

            // 跳过隐藏文件和特定目录
            if entry_name.starts_with('.')
                || entry_name == "node_modules"
                || entry_name == "dist"
                || entry_name == ".vitepress"
            {
                continue;
            }

            if entry_path.is_dir() {
                // 递归处理子目录
                if let Ok(child) = build_doc_tree(&entry_path, project_root, git_map).await {
                    children.push(child);
                }
            } else if entry_name.ends_with(".md") {
                // Markdown 文件
                let title = extract_title(&entry_path).await;
                let abs_path = entry_path.to_string_lossy().to_string();
                let status = git_map
                    .get(&abs_path)
                    .cloned()
                    .unwrap_or(GitFileStatus::Clean);

                children.push(DocTreeNode {
                    name: entry_name,
                    path: abs_path,
                    node_type: DocTreeNodeType::File,
                    title,
                    children: None,
                    git_status: Some(status),
                    order: None,
                });
            }
        }

        // 按名称排序（文件夹在前，文件在后）
        children.sort_by(|a, b| match (&a.node_type, &b.node_type) {
            (DocTreeNodeType::Folder, DocTreeNodeType::File) => std::cmp::Ordering::Less,
            (DocTreeNodeType::File, DocTreeNodeType::Folder) => std::cmp::Ordering::Greater,
            _ => a.name.cmp(&b.name),
        });

        // 尝试从 index.md 获取文件夹标题
        let title = if dir_path.join("index.md").exists() {
            extract_title(&dir_path.join("index.md")).await
        } else {
            None
        };

        // 文件夹也可以有状态（如果包含变动文件），这里简化处理，暂不计算文件夹的聚合状态

        Ok(DocTreeNode {
            name,
            path,
            node_type: DocTreeNodeType::Folder,
            title,
            children: if children.is_empty() {
                None
            } else {
                Some(children)
            },
            git_status: None,
            order: None,
        })
    })
}

/// 从 Markdown 文件中提取标题
async fn extract_title(file_path: &Path) -> Option<String> {
    let content = fs::read_to_string(file_path).await.ok()?;

    // 计算 frontmatter 结束位置
    let body_start = if content.starts_with("---") {
        if let Some(end_idx) = content[3..].find("---") {
            // 尝试从 frontmatter 提取 title
            let frontmatter = &content[3..end_idx + 3];
            for line in frontmatter.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with("title:") {
                    let title = trimmed[6..].trim();
                    // 去除引号
                    let title = title.trim_matches(|c| c == '"' || c == '\'');
                    if !title.is_empty() {
                        return Some(title.to_string());
                    }
                }
            }
            // frontmatter 结束位置 = 3 (开头 ---) + end_idx + 3 (结尾 ---)
            end_idx + 6
        } else {
            0
        }
    } else {
        0
    };

    // 在 body 中查找第一个 # 标题（跳过 frontmatter）
    let body = if body_start > 0 && body_start < content.len() {
        &content[body_start..]
    } else {
        &content
    };

    for line in body.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("# ") {
            return Some(trimmed[2..].trim().to_string());
        }
    }

    None
}

/// 读取文档内容
#[tauri::command(rename_all = "camelCase")]
pub async fn vitepress_read_document(
    path: String,
    base_path: Option<String>,
    root_path: Option<String>,
) -> Result<DocumentContent, String> {
    let mut target_path = PathBuf::from(&path);

    // 路径解析逻辑
    if path.starts_with("@/") {
        // 处理别名 (相对于项目根)
        if let Some(root) = root_path {
            target_path = Path::new(&root).join(&path[2..]);
        }
    } else if path.starts_with(".") {
        // 处理相对路径 (相对于当前文件)
        if let Some(base) = base_path {
            let base_dir = Path::new(&base)
                .parent()
                .ok_or_else(|| "Invalid base path".to_string())?;
            target_path = base_dir.join(&path);
        }
    }

    // 规范化路径 (处理 .. 等) - 简单处理，实际需要 canonicalize 但要小心文件不存在的情况
    // 这里我们先尝试直接读取，如果不存在再报错

    if !target_path.exists() {
        return Err(format!("File not found: {}", target_path.display()));
    }

    let content = fs::read_to_string(&target_path)
        .await
        .map_err(|e| format!("Failed to read file: {}", e))?;

    // 解析 frontmatter
    let frontmatter = parse_frontmatter(&content);

    Ok(DocumentContent {
        path: target_path.to_string_lossy().to_string(),
        content,
        frontmatter,
    })
}

/// 解析 frontmatter
fn parse_frontmatter(content: &str) -> Option<HashMap<String, serde_json::Value>> {
    if !content.starts_with("---") {
        return None;
    }

    let rest = &content[3..];
    let end_idx = rest.find("---")?;
    let frontmatter_str = &rest[..end_idx];

    // 使用简单的行解析
    let mut map = HashMap::new();
    for line in frontmatter_str.lines() {
        let trimmed = line.trim();
        if let Some(colon_idx) = trimmed.find(':') {
            let key = trimmed[..colon_idx].trim().to_string();
            let value = trimmed[colon_idx + 1..].trim();
            // 简单值处理
            let json_value = if value.starts_with('[') || value.starts_with('{') {
                serde_json::from_str(value).unwrap_or(serde_json::Value::String(value.to_string()))
            } else if value == "true" {
                serde_json::Value::Bool(true)
            } else if value == "false" {
                serde_json::Value::Bool(false)
            } else if let Ok(num) = value.parse::<i64>() {
                serde_json::Value::Number(num.into())
            } else {
                // 去除引号
                let clean = value.trim_matches(|c| c == '"' || c == '\'');
                serde_json::Value::String(clean.to_string())
            };
            map.insert(key, json_value);
        }
    }

    if map.is_empty() {
        None
    } else {
        Some(map)
    }
}

/// 保存文档
#[tauri::command(rename_all = "camelCase")]
pub async fn vitepress_save_document(path: String, content: String) -> Result<SaveResult, String> {
    let file_path = Path::new(&path);

    // 确保父目录存在
    if let Some(parent) = file_path.parent() {
        fs::create_dir_all(parent)
            .await
            .map_err(|e| format!("Failed to create directory: {}", e))?;
    }

    fs::write(file_path, &content)
        .await
        .map_err(|e| format!("Failed to write file: {}", e))?;

    Ok(SaveResult {
        success: true,
        path,
        message: None,
    })
}

/// 创建新文档
#[tauri::command(rename_all = "camelCase")]
pub async fn vitepress_create_document(
    dir: String,
    name: String,
    template: Option<String>,
) -> Result<String, String> {
    let dir_path = Path::new(&dir);

    // 确保文件名有 .md 后缀
    let file_name = if name.ends_with(".md") {
        name
    } else {
        format!("{}.md", name)
    };

    let file_path = dir_path.join(&file_name);

    if file_path.exists() {
        return Err(format!("File already exists: {}", file_path.display()));
    }

    // 确保目录存在
    fs::create_dir_all(dir_path)
        .await
        .map_err(|e| format!("Failed to create directory: {}", e))?;

    // 使用模板或默认内容
    let content = template.unwrap_or_else(|| {
        format!(
            r#"---
title: {}
---

# {}

"#,
            file_name.trim_end_matches(".md"),
            file_name.trim_end_matches(".md")
        )
    });

    fs::write(&file_path, content)
        .await
        .map_err(|e| format!("Failed to write file: {}", e))?;

    Ok(file_path.to_string_lossy().to_string())
}

/// 创建文件夹
#[tauri::command(rename_all = "camelCase")]
pub async fn vitepress_create_folder(parent: String, name: String) -> Result<String, String> {
    let parent_path = Path::new(&parent);
    let folder_path = parent_path.join(&name);

    if folder_path.exists() {
        return Err(format!("Folder already exists: {}", folder_path.display()));
    }

    fs::create_dir_all(&folder_path)
        .await
        .map_err(|e| format!("Failed to create folder: {}", e))?;

    // 创建 index.md
    let index_path = folder_path.join("index.md");
    let content = format!(
        r#"---
title: {}
---

# {}

"#,
        name, name
    );

    fs::write(&index_path, content)
        .await
        .map_err(|e| format!("Failed to create index.md: {}", e))?;

    Ok(folder_path.to_string_lossy().to_string())
}

/// 重命名文件或文件夹
#[tauri::command(rename_all = "camelCase")]
pub async fn vitepress_rename(old_path: String, new_name: String) -> Result<String, String> {
    let old = Path::new(&old_path);

    if !old.exists() {
        return Err(format!("Path not found: {}", old_path));
    }

    let parent = old
        .parent()
        .ok_or_else(|| "Cannot rename root".to_string())?;

    let new_path = parent.join(&new_name);

    if new_path.exists() {
        return Err(format!("Target already exists: {}", new_path.display()));
    }

    fs::rename(old, &new_path)
        .await
        .map_err(|e| format!("Failed to rename: {}", e))?;

    Ok(new_path.to_string_lossy().to_string())
}

/// 删除文件或文件夹
#[tauri::command(rename_all = "camelCase")]
pub async fn vitepress_delete(path: String) -> Result<bool, String> {
    let target = Path::new(&path);

    if !target.exists() {
        return Err(format!("Path not found: {}", path));
    }

    if target.is_dir() {
        fs::remove_dir_all(target)
            .await
            .map_err(|e| format!("Failed to delete folder: {}", e))?;
    } else {
        fs::remove_file(target)
            .await
            .map_err(|e| format!("Failed to delete file: {}", e))?;
    }

    Ok(true)
}

/// 安装依赖（删除 node_modules 后运行 pnpm install）
#[tauri::command(rename_all = "camelCase")]
pub async fn vitepress_install_dependencies<R: Runtime>(
    project_path: String,
    window: Window<R>,
) -> Result<(), String> {
    // 先删除 node_modules 目录
    let node_modules_path = Path::new(&project_path).join("node_modules");
    if node_modules_path.exists() {
        window
            .emit("vitepress://install-progress", "正在删除 node_modules...")
            .ok();
        fs::remove_dir_all(&node_modules_path)
            .await
            .map_err(|e| format!("Failed to remove node_modules: {}", e))?;
        window
            .emit("vitepress://install-progress", "node_modules 已删除")
            .ok();
    }

    // Windows: 使用 cmd /C 执行 pnpm install
    #[cfg(target_os = "windows")]
    let (mut rx, _child) = window
        .shell()
        .command("cmd")
        .args(["/C", "pnpm install"])
        .env("FORCE_COLOR", "1") // 强制彩色输出
        .current_dir(Path::new(&project_path))
        .spawn()
        .map_err(|e| format!("Failed to spawn pnpm install: {}", e))?;

    // 非 Windows: 直接调用 pnpm
    #[cfg(not(target_os = "windows"))]
    let (mut rx, _child) = window
        .shell()
        .command("pnpm")
        .args(["install"])
        .env("FORCE_COLOR", "1") // 强制彩色输出
        .current_dir(Path::new(&project_path))
        .spawn()
        .map_err(|e| format!("Failed to spawn pnpm install: {}", e))?;

    let window_clone = window.clone();
    tauri::async_runtime::spawn(async move {
        while let Some(event) = rx.recv().await {
            match event {
                CommandEvent::Stdout(line) => {
                    let msg = String::from_utf8_lossy(&line).to_string();
                    window_clone.emit("vitepress://install-progress", msg).ok();
                }
                CommandEvent::Stderr(line) => {
                    let msg = String::from_utf8_lossy(&line).to_string();
                    window_clone.emit("vitepress://install-progress", msg).ok();
                }
                CommandEvent::Terminated(payload) => {
                    let success = payload.code.unwrap_or(0) == 0;
                    // let msg = if success { ... } -- unused
                    window_clone
                        .emit("vitepress://install-finish", success)
                        .ok();
                }
                _ => {}
            }
        }
    });

    Ok(())
}

/// 启动 VitePress Dev Server
#[tauri::command(rename_all = "camelCase")]
pub async fn vitepress_start_dev_server<R: Runtime>(
    project_path: String,
    port: Option<u16>,
    window: Window<R>,
    state: State<'_, DevServerState>,
) -> Result<DevServerInfo, String> {
    let mut args = vec!["run", "docs:dev"];
    let port_str = port.map(|p| p.to_string()); // Keep alive longer
    if let Some(ref p) = port_str {
        args.push("--port");
        args.push(p);
    }

    // Check if port is available? (Optional)

    #[cfg(target_os = "windows")]
    let (mut rx, child) = {
        let mut cmd_args = vec!["/C", "pnpm", "run", "docs:dev"];
        if let Some(ref p) = port_str {
            cmd_args.push("--port");
            cmd_args.push(p);
        }
        window
            .shell()
            .command("cmd")
            .args(&cmd_args)
            .current_dir(Path::new(&project_path))
            .spawn()
            .map_err(|e| format!("Failed to start dev server: {}", e))?
    };

    #[cfg(not(target_os = "windows"))]
    let (mut rx, child) = window
        .shell()
        .command("pnpm")
        .args(&args)
        .current_dir(Path::new(&project_path))
        .spawn()
        .map_err(|e| format!("Failed to start dev server: {}", e))?;

    let pid = child.pid();

    // Channel to receive URL
    let (tx, mut rx_url) = tokio::sync::mpsc::channel(1);
    let window_clone = window.clone();

    tauri::async_runtime::spawn(async move {
        let mut url_found = false;
        while let Some(event) = rx.recv().await {
            match event {
                CommandEvent::Stdout(line) => {
                    let msg = String::from_utf8_lossy(&line).to_string();

                    // Try to find URL
                    if !url_found {
                        // Remove color codes
                        let clean_msg =
                            msg.replace(|c: char| c.is_control() && c != '\n' && c != '\r', "");
                        if clean_msg.contains("http://localhost:")
                            || clean_msg.contains("http://127.0.0.1:")
                        {
                            if let Some(idx) = clean_msg.find("http") {
                                let url_part = &clean_msg[idx..];
                                // Simple split by whitespace
                                let url = url_part.split_whitespace().next().unwrap_or(url_part);
                                // Strip ANSI codes if any remains
                                // Basic URL validation/cleaning
                                let final_url = url
                                    .trim_matches(|c| {
                                        !char::is_alphanumeric(c) && c != '/' && c != ':'
                                    })
                                    .to_string();
                                if !final_url.is_empty() {
                                    tx.send(final_url).await.ok();
                                    url_found = true;
                                }
                            }
                        }
                    }

                    window_clone
                        .emit("vitepress://dev-server-output", &msg)
                        .ok();
                }
                CommandEvent::Stderr(line) => {
                    let msg = String::from_utf8_lossy(&line).to_string();
                    window_clone
                        .emit("vitepress://dev-server-output", &msg)
                        .ok();
                }
                _ => {}
            }
        }
    });

    // Store child
    state.servers.lock().unwrap().insert(pid, child);

    // Wait for URL (max 15s)
    let url = match tokio::time::timeout(tokio::time::Duration::from_secs(15), rx_url.recv()).await
    {
        Ok(Some(u)) => u,
        Ok(None) => format!("http://localhost:{}", port.unwrap_or(5173)),
        Err(_) => format!("http://localhost:{}", port.unwrap_or(5173)),
    };

    Ok(DevServerInfo {
        url,
        port: port.unwrap_or(5173),
        process_id: pid,
        status: DevServerStatus::Running,
    })
}

/// 停止 Dev Server
#[tauri::command(rename_all = "camelCase")]
pub async fn vitepress_stop_dev_server(
    process_id: u32,
    project_root: Option<String>,
    state: State<'_, DevServerState>,
) -> Result<(), String> {
    // On Windows, run taskkill FIRST to ensure we catch the process tree before the parent dies
    #[cfg(target_os = "windows")]
    {
        let output = std::process::Command::new("taskkill")
            .args(["/F", "/T", "/PID", &process_id.to_string()])
            .output();

        match output {
            Ok(o) => {
                if !o.status.success() {
                    let stderr = String::from_utf8_lossy(&o.stderr);
                    println!("Taskkill warning for PID {}: {}", process_id, stderr);
                }
            }
            Err(e) => println!("Taskkill failed for PID {}: {}", process_id, e),
        }
    }

    // 先处理 state，在 await 之前释放 MutexGuard
    {
        let mut servers = state.servers.lock().unwrap();
        if let Some(child) = servers.remove(&process_id) {
            // Still try to call kill on the wrapper for cleanup
            let _ = child.kill();
        }
    } // MutexGuard 在这里被释放

    // 清理预览目录（现在可以安全地 await）
    if let Some(root) = project_root {
        let preview_dir = Path::new(&root).join(PREVIEW_DIR_NAME);
        if preview_dir.exists() {
            if let Err(e) = fs::remove_dir_all(&preview_dir).await {
                println!("Failed to clean preview directory: {}", e);
            } else {
                println!("Cleaned preview directory: {:?}", preview_dir);
            }
        }
    }

    Ok(())
}

// ============================================================================
// 预览文件管理
// ============================================================================

/// 预览目录名称
const PREVIEW_DIR_NAME: &str = "_fireworks_preview";

/// 使用静态锁防止并发修改 .git/info/exclude
static EXCLUDE_LOCK: AsyncMutex<()> = AsyncMutex::const_new(());

/// 确保 Git 忽略预览目录（使用 .git/info/exclude，不修改 .gitignore）
async fn ensure_git_exclude(project_root: &Path) -> Result<(), String> {
    // 获取全局锁，确保同一时间只有一个任务在修改 exclude 文件
    let _guard = EXCLUDE_LOCK.lock().await;

    let exclude_path = project_root.join(".git/info/exclude");

    if !exclude_path.exists() {
        // .git/info 目录可能不存在
        if let Some(parent) = exclude_path.parent() {
            match fs::create_dir_all(parent).await {
                Ok(_) => (),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => (), // 忽略已存在错误
                Err(e) => return Err(format!("Failed to create .git/info: {}", e)),
            }
        }
        fs::write(&exclude_path, format!("{}/\n", PREVIEW_DIR_NAME))
            .await
            .map_err(|e| format!("Failed to create exclude file: {}", e))?;
        return Ok(());
    }

    let content = fs::read_to_string(&exclude_path)
        .await
        .map_err(|e| format!("Failed to read exclude file: {}", e))?;

    let pattern = format!("{}/", PREVIEW_DIR_NAME);
    if !content.contains(&pattern) {
        let new_content = if content.ends_with('\n') {
            format!("{}{}\n", content, pattern)
        } else {
            format!("{}\n{}\n", content, pattern)
        };
        fs::write(&exclude_path, new_content)
            .await
            .map_err(|e| format!("Failed to update exclude file: {}", e))?;
    }

    Ok(())
}

/// 创建或更新预览文件
#[tauri::command(rename_all = "camelCase")]
pub async fn vitepress_create_preview(
    project_root: String,
    preview_id: String,
    content: String,
    content_type: String, // "markdown" | "vue"
) -> Result<String, String> {
    let root = Path::new(&project_root);
    let preview_dir = root.join(PREVIEW_DIR_NAME);

    // 确保预览目录存在
    if !preview_dir.exists() {
        fs::create_dir_all(&preview_dir)
            .await
            .map_err(|e| format!("Failed to create preview dir: {}", e))?;

        // 确保 Git 忽略
        ensure_git_exclude(root).await?;
    }

    // 生成预览文件
    let file_name = format!("{}.md", preview_id);
    let file_path = preview_dir.join(&file_name);

    // 构建预览 Markdown 内容
    // 使用 layout: false 避免导航栏
    // 使用 vp-doc 类保留 VitePress 文档样式
    let preview_content = match content_type.as_str() {
        "vue" => format!(
            r#"---
layout: false
title: Preview
---

<div class="vp-doc preview-root" style="padding: 16px;">

{}

</div>

<script setup>
import {{ onMounted }} from 'vue'
onMounted(() => {{
  const previewId = '{}'
  const root = document.querySelector('.preview-root')
  const sendHeight = () => {{
    if (!root) return
    const height = root.scrollHeight
    window.parent.postMessage({{ type: 'resize', previewId, height }}, '*')
  }}
  // 初始发送
  sendHeight()
  // 持续监听高度变化（动态内容）
  const observer = new ResizeObserver(sendHeight)
  if (root) observer.observe(root)
}})
</script>

<style>
@import 'vitepress/dist/client/theme-default/styles/vars.css';
@import 'vitepress/dist/client/theme-default/styles/base.css';
@import 'vitepress/dist/client/theme-default/styles/utils.css';
@import 'vitepress/dist/client/theme-default/styles/components/vp-doc.css';

html, body {{
  height: auto !important;
  min-height: 0 !important;
  overflow: hidden !important;
}}

/* 禁用第一个子元素的上外边距 */
.preview-root > :first-child {{ margin-top: 0; }}
</style>
"#,
            content, preview_id
        ),
        _ => format!(
            r#"---
layout: false
title: Preview
---

<div class="vp-doc preview-root" style="padding: 16px;">

{}

</div>

<script setup>
import {{ onMounted }} from 'vue'
onMounted(() => {{
  const previewId = '{}'
  const root = document.querySelector('.preview-root')
  const sendHeight = () => {{
    if (!root) return
    const height = root.scrollHeight
    window.parent.postMessage({{ type: 'resize', previewId, height }}, '*')
  }}
  // 初始发送
  sendHeight()
  // 持续监听高度变化（动态内容）
  const observer = new ResizeObserver(sendHeight)
  if (root) observer.observe(root)
}})
</script>

<style>
@import 'vitepress/dist/client/theme-default/styles/vars.css';
@import 'vitepress/dist/client/theme-default/styles/base.css';
@import 'vitepress/dist/client/theme-default/styles/utils.css';
@import 'vitepress/dist/client/theme-default/styles/components/vp-doc.css';

html, body {{
  height: auto !important;
  min-height: 0 !important;
  overflow: hidden !important;
}}

/* 禁用第一个子元素的上外边距 */
.preview-root > :first-child {{ margin-top: 0; }}
</style>
"#,
            content, preview_id
        ),
    };

    fs::write(&file_path, preview_content)
        .await
        .map_err(|e| format!("Failed to write preview file: {}", e))?;

    // 返回预览 URL 路径（相对于 VitePress root）
    Ok(format!("/{}/{}.html", PREVIEW_DIR_NAME, preview_id))
}

/// 删除单个预览文件
#[tauri::command(rename_all = "camelCase")]
pub async fn vitepress_delete_preview(
    project_root: String,
    preview_id: String,
) -> Result<(), String> {
    let root = Path::new(&project_root);
    let file_path = root
        .join(PREVIEW_DIR_NAME)
        .join(format!("{}.md", preview_id));

    if file_path.exists() {
        fs::remove_file(&file_path)
            .await
            .map_err(|e| format!("Failed to delete preview file: {}", e))?;
    }

    Ok(())
}

/// 清理所有预览文件（编辑器关闭时调用）
#[tauri::command(rename_all = "camelCase")]
pub async fn vitepress_cleanup_previews(project_root: String) -> Result<(), String> {
    let preview_dir = Path::new(&project_root).join(PREVIEW_DIR_NAME);

    if preview_dir.exists() {
        fs::remove_dir_all(&preview_dir)
            .await
            .map_err(|e| format!("Failed to cleanup preview dir: {}", e))?;
    }

    Ok(())
}

/// 获取项目的所有 Vue 组件信息（解析 props 和 metadata）
#[tauri::command(rename_all = "camelCase")]
pub async fn vitepress_get_components(project_root: String) -> Result<Vec<VueComponent>, String> {
    let components_dir = Path::new(&project_root).join(".vitepress/theme/components");
    if !components_dir.exists() {
        return Ok(vec![]);
    }

    let mut components = Vec::new();
    // 使用栈进行迭代式递归扫描
    let mut stack = vec![components_dir.clone()];

    while let Some(current_dir) = stack.pop() {
        if let Ok(mut entries) = fs::read_dir(&current_dir).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else if path.extension().and_then(|s| s.to_str()) == Some("vue") {
                    if let Ok(content) = fs::read_to_string(&path).await {
                        if let Some(mut comp) = parse_vue_component(&path, &content) {
                            // 计算相对路径
                            if let Ok(rel) = path.strip_prefix(&components_dir) {
                                comp.path = rel.to_string_lossy().replace("\\", "/");
                            }
                            components.push(comp);
                        }
                    }
                }
            }
        }
    }

    // 按名称排序
    components.sort_by(|a, b| a.name.cmp(&b.name));

    Ok(components)
}

/// 解析 Vue 组件内容
fn parse_vue_component(path: &Path, content: &str) -> Option<VueComponent> {
    // 检查是否忽略
    if content.contains("@editor-ignore") {
        return None;
    }

    let name = path.file_stem()?.to_str()?.to_string();
    let mut description = None;
    let mut props = Vec::new();

    // 1. 尝试解析 <editor> XML 块 (主要约定)
    let editor_regex = Regex::new(r"(?s)<editor>\s*(\{.*?\})\s*</editor>").unwrap();
    if let Some(caps) = editor_regex.captures(content) {
        if let Some(json_str) = caps.get(1) {
            if let Ok(meta) = serde_json::from_str::<serde_json::Value>(json_str.as_str()) {
                // 如果标记为 hidden，则跳过
                if meta
                    .get("hidden")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false)
                {
                    return None;
                }

                if let Some(desc) = meta.get("description").and_then(|v| v.as_str()) {
                    description = Some(desc.to_string());
                }

                if let Some(props_meta) = meta.get("props").and_then(|v| v.as_array()) {
                    for p in props_meta {
                        props.push(ComponentProp {
                            name: p
                                .get("name")
                                .and_then(|v| v.as_str())
                                .unwrap_or("unknown")
                                .to_string(),
                            type_name: p
                                .get("type")
                                .and_then(|v| v.as_str())
                                .unwrap_or("string")
                                .to_string(),
                            description: p
                                .get("description")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string()),
                            default: p.get("default").map(|v| v.to_string()),
                            optional: p.get("optional").and_then(|v| v.as_bool()).unwrap_or(true), // 默认为可选
                        });
                    }
                    // 如果有 <editor> props 定义，直接返回，不再尝试正则解析 props
                    return Some(VueComponent {
                        name,
                        path: path.file_name()?.to_str()?.to_string(),
                        description,
                        props,
                    });
                }
            }
        }
    }

    // 如果 <editor> 中没有提供 description，尝试从组件注释解析
    if description.is_none() {
        // 查找顶层 JSDoc: /** ... */ (简单匹配第一个)
        let doc_regex = Regex::new(r"(?s)/\*\*\s*(.*?)\s*\*/").unwrap();
        if let Some(caps) = doc_regex.captures(content) {
            if let Some(doc_content) = caps.get(1) {
                let lines: Vec<&str> = doc_content
                    .as_str()
                    .lines()
                    .map(|l| l.trim().trim_start_matches('*').trim())
                    .filter(|l| !l.starts_with('@')) // 排除 @xxx 标签，但保留空行
                    .collect();

                // 合并连续空行为单个空行，并去除首尾空行
                let mut result = Vec::new();
                let mut prev_empty = true; // 跳过开头空行
                for line in lines {
                    if line.is_empty() {
                        if !prev_empty {
                            result.push(""); // 保留段落分隔
                            prev_empty = true;
                        }
                    } else {
                        result.push(line);
                        prev_empty = false;
                    }
                }
                // 去除末尾空行
                while result.last() == Some(&"") {
                    result.pop();
                }

                let desc_clean = result.join("\n");
                if !desc_clean.is_empty() {
                    description = Some(desc_clean);
                }
            }
        }
    }

    // 如果 <editor> 没有提供 props，尝试从 defineProps 解析
    if props.is_empty() {
        // 匹配 defineProps<{ ... }>()
        let props_block_regex =
            Regex::new(r"(?s)defineProps\s*[<|(]\s*[\{](.*?)[\}]\s*[>|)]").unwrap();
        if let Some(caps) = props_block_regex.captures(content) {
            if let Some(inner) = caps.get(1) {
                let inner_text = inner.as_str();

                // 匹配 JSDoc + 属性定义
                // 模式: (/** doc */)? name?: type
                let prop_item_regex = Regex::new(r"(?m)(?:/\*\*\s*(?P<doc>[\s\S]*?)\s*\*/\s*)?^\s*(?P<name>[a-zA-Z0-9_$]+)(?P<optional>\?)?\s*:\s*(?P<type>[^,;\n]+)").unwrap();

                for cap in prop_item_regex.captures_iter(inner_text) {
                    let name = cap
                        .name("name")
                        .map(|m| m.as_str())
                        .unwrap_or("")
                        .to_string();
                    let type_str = cap
                        .name("type")
                        .map(|m| m.as_str().trim())
                        .unwrap_or("string")
                        .to_string();

                    // 移除可能的逗号或分号
                    let type_clean = type_str
                        .trim_end_matches(|c| c == ',' || c == ';')
                        .to_string();

                    let mut prop_desc = None;
                    let mut prop_default = None;

                    if let Some(doc_match) = cap.name("doc") {
                        let doc_lines: Vec<&str> = doc_match
                            .as_str()
                            .lines()
                            .map(|l| l.trim().trim_start_matches('*').trim())
                            .filter(|l| !l.is_empty())
                            .collect();

                        // 提取 @default 值
                        for line in &doc_lines {
                            if line.starts_with("@default") {
                                let default_val =
                                    line.trim_start_matches("@default").trim().to_string();
                                if !default_val.is_empty() {
                                    prop_default = Some(default_val);
                                }
                            }
                        }

                        // 过滤掉 @xxx 标签，保留纯描述
                        let clean_doc = doc_lines
                            .iter()
                            .filter(|l| !l.starts_with('@'))
                            .copied()
                            .collect::<Vec<_>>()
                            .join("\n"); // 保留换行

                        if !clean_doc.is_empty() {
                            prop_desc = Some(clean_doc);
                        }
                    }

                    // 检查是否可选 (通过 ? 标记)
                    let is_optional = cap.name("optional").is_some();

                    props.push(ComponentProp {
                        name,
                        type_name: type_clean,
                        description: prop_desc,
                        default: prop_default,
                        optional: is_optional,
                    });
                }
            }
        }
    }

    Some(VueComponent {
        name,
        path: path.file_name()?.to_str()?.to_string(), // 这里先保持 filename，外部会更新为 relative path
        description,
        props,
    })
}
