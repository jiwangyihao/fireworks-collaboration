/**
 * VitePress API 封装
 *
 * 提供 VitePress 项目相关的前端 API 调用
 */

import { invoke } from "./tauri";

// ============================================================================
// 类型定义
// ============================================================================

/** VitePress 项目检测结果 */
export interface VitePressDetection {
  isVitepress: boolean;
  configPath: string | null;
  contentRoot: string | null;
  projectName: string | null;
}

/** VitePress 配置 */
export interface VitePressConfig {
  title: string | null;
  description: string | null;
  lang: string | null;
  srcDir: string | null;
  srcExclude: string[];
  themeConfig: ThemeConfig | null;
}

/** 主题配置 */
export interface ThemeConfig {
  nav: NavItem[] | null;
  logo: string | null;
}

/** 导航项 */
export interface NavItem {
  text: string;
  link: string | null;
  items: NavItem[] | null;
}

/** 依赖状态 */
export interface DependencyStatus {
  installed: boolean;
  pnpmLockExists: boolean;
  nodeModulesExists: boolean;
  pnpmStoreExists: boolean;
  outdated: boolean;
  packageManager: string;
}

/** Dev Server 信息 */
export interface DevServerInfo {
  url: string;
  port: number;
  processId: number;
  status: "starting" | "running" | "stopped" | "error";
}

/** 文档树节点类型 */
export type DocTreeNodeType = "file" | "folder";

/** Git 文件状态 */
export type GitFileStatus =
  | "clean"
  | "modified"
  | "staged"
  | "untracked"
  | "conflict";

/** 文档树节点 */
export interface DocTreeNode {
  name: string;
  path: string;
  nodeType: DocTreeNodeType;
  title: string | null;
  children: DocTreeNode[] | null;
  gitStatus: GitFileStatus | null;
  order: number | null;
}

/** 文档内容 */
export interface DocumentContent {
  path: string;
  content: string;
  frontmatter: Record<string, unknown> | null;
}

/** 保存结果 */
export interface SaveResult {
  success: boolean;
  path: string;
  message: string | null;
}

// ============================================================================
// API 函数
// ============================================================================

/**
 * 检测指定路径是否为 VitePress 项目
 */
export async function detectVitePressProject(
  path: string
): Promise<VitePressDetection> {
  return invoke<VitePressDetection>("vitepress_detect_project", { path });
}

/**
 * 检查项目依赖状态
 */
export async function checkDependencies(
  projectPath: string
): Promise<DependencyStatus> {
  return invoke<DependencyStatus>("vitepress_check_dependencies", {
    projectPath,
  });
}

/**
 * 获取文档目录树
 */
export async function getDocTree(
  projectPath: string,
  contentRoot?: string
): Promise<DocTreeNode> {
  return invoke<DocTreeNode>("vitepress_get_doc_tree", {
    projectPath,
    contentRoot,
  });
}

/**
 * 读取文档内容
 */
export async function readDocument(path: string): Promise<DocumentContent> {
  return invoke<DocumentContent>("vitepress_read_document", { path });
}

/**
 * 保存文档
 */
export async function saveDocument(
  path: string,
  content: string
): Promise<SaveResult> {
  return invoke<SaveResult>("vitepress_save_document", { path, content });
}

/**
 * 创建新文档
 */
export async function createDocument(
  dir: string,
  name: string,
  template?: string
): Promise<string> {
  return invoke<string>("vitepress_create_document", { dir, name, template });
}

/**
 * 创建文件夹
 */
export async function createFolder(
  parent: string,
  name: string
): Promise<string> {
  return invoke<string>("vitepress_create_folder", { parent, name });
}

/**
 * 重命名文件或文件夹
 */
export async function renameItem(
  oldPath: string,
  newName: string
): Promise<string> {
  return invoke<string>("vitepress_rename", { oldPath, newName });
}

/**
 * 删除文件或文件夹
 */
export async function deleteItem(path: string): Promise<boolean> {
  return invoke<boolean>("vitepress_delete", { path });
}

/**
 * 安装依赖
 */
export async function installDependencies(projectPath: string): Promise<void> {
  return invoke<void>("vitepress_install_dependencies", { projectPath });
}

/**
 * 启动 Dev Server
 */
export async function startDevServer(
  projectPath: string,
  port?: number
): Promise<DevServerInfo> {
  return invoke<DevServerInfo>("vitepress_start_dev_server", {
    projectPath,
    port,
  });
}

/**
 * 停止 Dev Server
 */
export async function stopDevServer(processId: number): Promise<void> {
  return invoke<void>("vitepress_stop_dev_server", { processId });
}

/**
 * 解析 VitePress 配置
 */
export async function parseConfig(
  projectPath: string
): Promise<VitePressConfig> {
  return invoke<VitePressConfig>("vitepress_parse_config", { projectPath });
}
