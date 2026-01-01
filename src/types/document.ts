/**
 * Document 类型定义
 *
 * 本文件定义文档结构和 Frontmatter 元数据类型。
 * 用于 E1（VitePress 项目集成）阶段的文档管理。
 */

import type { Block } from "./block";

// ============================================================================
// Frontmatter 类型
// ============================================================================

/**
 * VitePress 文档 Frontmatter 元数据
 * 参考：https://vitepress.dev/reference/frontmatter-config
 */
export interface Frontmatter {
  /** 文档标题（覆盖从 # 标题提取的标题） */
  title?: string;
  /** 文档描述（用于 SEO） */
  description?: string;
  /** 标签列表 */
  tags?: string[];
  /** 发布日期 */
  date?: string;
  /** 作者 */
  author?: string;
  /** 布局类型 */
  layout?: "doc" | "page" | "home" | string;
  /** 是否显示在侧边栏 */
  sidebar?: boolean;
  /** 是否显示大纲 */
  outline?: boolean | number | [number, number] | "deep";
  /** 是否显示上/下一页导航 */
  prev?: boolean | string | { text: string; link: string };
  next?: boolean | string | { text: string; link: string };
  /** 是否显示最后更新时间 */
  lastUpdated?: boolean | Date;
  /** 是否可编辑（用于"编辑此页"链接） */
  editLink?: boolean;
  /** 页面特定的 head 配置 */
  head?: Array<[string, Record<string, string>]>;
  /** 允许任意自定义字段 */
  [key: string]: unknown;
}

// ============================================================================
// Document 类型
// ============================================================================

/**
 * 文档状态枚举
 */
export type DocumentStatus =
  | "clean" // 未修改
  | "modified" // 已修改未保存
  | "saving" // 保存中
  | "error"; // 错误状态

/**
 * 文档结构
 */
export interface Document {
  /** 文件路径（相对于项目根目录） */
  path: string;
  /** 绝对文件路径 */
  absolutePath?: string;
  /** Frontmatter 元数据 */
  frontmatter: Frontmatter;
  /** 文档内容块 */
  blocks: Block[];
  /** 原始 Markdown 内容（用于比较变更） */
  rawContent?: string;
  /** 最后修改时间（ISO 8601 格式） */
  lastModified?: string;
  /** 文档状态 */
  status?: DocumentStatus;
}

// ============================================================================
// 文档树类型（用于 E1 目录展示）
// ============================================================================

/**
 * 文档树节点类型
 */
export type DocTreeNodeType = "file" | "folder";

/**
 * Git 状态类型
 */
export type GitStatus =
  | "clean" // 无变化
  | "modified" // 已修改
  | "staged" // 已暂存
  | "untracked" // 未跟踪
  | "conflict"; // 冲突

/**
 * 文档树节点（用于目录树展示）
 */
export interface DocTreeNode {
  /** 节点名称（文件名或文件夹名） */
  name: string;
  /** 节点路径（相对于项目根目录） */
  path: string;
  /** 节点类型 */
  type: DocTreeNodeType;
  /** 显示标题（从 index.md 或 # 标题提取） */
  title?: string;
  /** 子节点（仅文件夹有） */
  children?: DocTreeNode[];
  /** Git 状态 */
  gitStatus?: GitStatus;
  /** 是否展开（UI 状态） */
  expanded?: boolean;
  /** 排序权重（可选） */
  order?: number;
}

// ============================================================================
// VitePress 配置类型（用于 E1 解析）
// ============================================================================

/**
 * VitePress 站点配置（简化版）
 * 完整配置参考：https://vitepress.dev/reference/site-config
 */
export interface VitePressConfig {
  /** 站点标题 */
  title?: string;
  /** 站点描述 */
  description?: string;
  /** 基础路径 */
  base?: string;
  /** 源文件目录 */
  srcDir?: string;
  /** 排除的文件模式 */
  srcExclude?: string[];
  /** 是否使用干净 URL */
  cleanUrls?: boolean;
  /** 主题配置 */
  themeConfig?: {
    /** 导航栏 */
    nav?: NavItem[];
    /** 侧边栏 */
    sidebar?: SidebarConfig;
    /** 社交链接 */
    socialLinks?: SocialLink[];
  };
}

/**
 * 导航项
 */
export interface NavItem {
  text: string;
  link?: string;
  items?: NavItem[];
  activeMatch?: string;
}

/**
 * 侧边栏配置
 */
export type SidebarConfig =
  | SidebarItem[]
  | Record<string, SidebarItem[]>
  | "auto";

/**
 * 侧边栏项
 */
export interface SidebarItem {
  text: string;
  link?: string;
  items?: SidebarItem[];
  collapsed?: boolean;
}

/**
 * 社交链接
 */
export interface SocialLink {
  icon: string;
  link: string;
  ariaLabel?: string;
}

// ============================================================================
// VitePress 项目检测结果
// ============================================================================

/**
 * VitePress 项目检测结果
 */
export interface VitePressDetection {
  /** 是否为 VitePress 项目 */
  isVitePress: boolean;
  /** 配置文件路径（如有） */
  configPath?: string;
  /** VitePress 版本（如有） */
  version?: string;
  /** 内容根目录 */
  contentRoot?: string;
  /** 错误信息（如检测失败） */
  error?: string;
}

// ============================================================================
// 辅助函数
// ============================================================================

/**
 * 创建空文档
 */
export function createEmptyDocument(path: string): Document {
  return {
    path,
    frontmatter: {},
    blocks: [],
    status: "clean",
  };
}

/**
 * 创建文档树叶子节点（文件）
 */
export function createDocTreeFile(
  name: string,
  path: string,
  title?: string
): DocTreeNode {
  return {
    name,
    path,
    type: "file",
    title: title || name.replace(/\.md$/, ""),
  };
}

/**
 * 创建文档树文件夹节点
 */
export function createDocTreeFolder(
  name: string,
  path: string,
  children: DocTreeNode[] = [],
  title?: string
): DocTreeNode {
  return {
    name,
    path,
    type: "folder",
    title: title || name,
    children,
    expanded: false,
  };
}
