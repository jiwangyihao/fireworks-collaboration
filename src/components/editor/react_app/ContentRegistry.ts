/**
 * ContentRegistry.ts - 统一内容注册中心
 *
 * 提供去中心化的内容类型注册机制，覆盖：
 * 1. Toolbar 能力配置 (supportedStyles, actions)
 * 2. SlashMenu 项注册 (一个类型可对应多个菜单项)
 * 3. SideMenu 操作注册
 *
 * 每个块组件自行注册其配置，实现组件自治。
 */
import { Block, BlockNoteEditor } from "@blocknote/core";
import { ReactNode } from "react";
import React from "react";
import { Icon } from "@iconify/react";

// ============================================================================
// Toolbar Action 类型定义
// ============================================================================

export type BlockActionType = "button" | "dropdown" | "input" | "toggle";

interface BaseActionDefinition {
  id: string;
  label: string;
  icon: ReactNode;
}

export interface ButtonActionDefinition extends BaseActionDefinition {
  type: "button";
}

export interface DropdownActionDefinition extends BaseActionDefinition {
  type: "dropdown";
  iconOnly?: boolean;
}

export interface InputActionDefinition extends BaseActionDefinition {
  type: "input";
  placeholder?: string;
  width?: string;
  hideIcon?: boolean;
}

export interface ToggleActionDefinition extends BaseActionDefinition {
  type: "toggle";
}

export type BlockActionDefinition =
  | ButtonActionDefinition
  | DropdownActionDefinition
  | InputActionDefinition
  | ToggleActionDefinition;

// ============================================================================
// Executor 接口
// ============================================================================

export interface DropdownOption {
  value: string;
  label: string;
  icon?: ReactNode;
}

export interface BlockActionExecutor {
  execute: (value?: any) => void;
  isActive: () => boolean;
  getValue?: () => any;
  getOptions?: () => DropdownOption[];
}

// ============================================================================
// SlashMenu 项定义
// ============================================================================

export interface SlashMenuItemDefinition {
  /** 唯一标识符 */
  id: string;
  /** 显示标题 */
  title: string;
  /** 副标题/描述 */
  subtext?: string;
  /** 图标 */
  icon: ReactNode;
  /** 分组 */
  group: string;
  /** 搜索别名 */
  aliases: string[];
  /** 对应的块类型 */
  blockType: string;
  /** 插入时的默认 props */
  props?: Record<string, any>;
  /** 插入后是否移动光标到新块 */
  moveCursor?: boolean;
}

// ============================================================================
// SideMenu Action 定义
// ============================================================================

export interface SideMenuActionDefinition {
  /** 唯一标识符 */
  id: string;
  /** 显示标签 */
  label: string;
  /** Iconify 图标名 */
  icon: string;
  /** 当前是否激活（可选） */
  isActive?: (block: Block<any, any, any>) => boolean;
  /** 执行操作 */
  execute: (block: Block<any, any, any>, editor: any) => void;
}

// ============================================================================
// 内容类型接口 (原 BlockCapabilities)
// ============================================================================

export interface ContentType {
  // --- Toolbar 能力 ---
  /** 支持的样式列表，true 表示支持所有默认样式 */
  supportedStyles: string[] | boolean;
  hasAlignment: boolean;
  hasIndent: boolean;
  hasTextColor: boolean;
  hasBackgroundColor: boolean;
  /** Toolbar 操作 */
  actions: BlockActionDefinition[];
  /** 图标 */
  icon: ReactNode;
  /** 显示名称 */
  label: string;

  // --- SlashMenu 配置 ---
  /** SlashMenu 项（一个类型可注册多个项） */
  slashMenuItems?: SlashMenuItemDefinition[];

  // --- SideMenu 配置 ---
  /** SideMenu 操作 */
  sideMenuActions?: SideMenuActionDefinition[];
}

// 兼容别名
export type BlockCapabilities = ContentType;

// ============================================================================
// 图标助手
// ============================================================================

export const iconify = (icon: string) =>
  React.createElement(Icon, { icon, className: "w-4 h-4" });

// ============================================================================
// 默认配置
// ============================================================================

const DEFAULT_CONTENT_TYPE: ContentType = {
  supportedStyles: ["bold", "italic", "underline", "strike", "code", "link"],
  hasAlignment: true,
  hasIndent: true,
  hasTextColor: true,
  hasBackgroundColor: true,
  actions: [],
  icon: iconify("lucide:help-circle"),
  label: "Unknown Block",
  slashMenuItems: [],
  sideMenuActions: [],
};

// ============================================================================
// 注册表类
// ============================================================================

export class ContentRegistry {
  // 块类型 -> 能力配置
  private types: Map<string, Partial<ContentType>> = new Map();

  // 块实例 ID -> 操作执行器映射 (blockId -> actionId -> executor)
  private executors: Map<string, Map<string, BlockActionExecutor>> = new Map();

  // 当前激活的 Inline 内容
  private activeInline: {
    type: string;
    executor: Record<string, BlockActionExecutor>;
  } | null = null;

  // 订阅者
  private listeners: Set<() => void> = new Set();

  private initialized = false;

  constructor() {
    this.initDefaults();
  }

  // 初始化默认块 (BlockNote 内置块)
  private initDefaults() {
    if (this.initialized) return;

    // 段落
    this.register("paragraph", {
      icon: iconify("lucide:pilcrow"),
      label: "段落",
    });

    // 标题
    this.register("heading", {
      icon: iconify("lucide:heading-1"),
      label: "标题",
    });
    this.register("heading-1", {
      icon: iconify("lucide:heading-1"),
      label: "标题 1",
    });
    this.register("heading-2", {
      icon: iconify("lucide:heading-2"),
      label: "标题 2",
    });
    this.register("heading-3", {
      icon: iconify("lucide:heading-3"),
      label: "标题 3",
    });

    // 列表
    this.register("bulletListItem", {
      icon: iconify("lucide:list"),
      label: "无序列表",
    });
    this.register("numberedListItem", {
      icon: iconify("lucide:list-ordered"),
      label: "有序列表",
    });
    this.register("checkListItem", {
      icon: iconify("lucide:check-square"),
      label: "任务列表",
    });

    // 其他内置块
    this.register("quote", { icon: iconify("lucide:quote"), label: "引用" });
    this.register("image", {
      icon: iconify("lucide:image"),
      label: "图片",
      supportedStyles: [],
    });
    this.register("table", { icon: iconify("lucide:table"), label: "表格" });

    this.initialized = true;
  }

  // =========================================================================
  // 注册 API
  // =========================================================================

  /**
   * 注册或更新内容类型（由块组件调用）
   */
  register(type: string, config: Partial<ContentType>) {
    const existing = this.types.get(type) || {};
    this.types.set(type, { ...existing, ...config });
    this.notify();
  }

  /**
   * 获取内容类型配置
   */
  get(type: string): ContentType {
    const specific = this.types.get(type);

    if (!specific) {
      if (type.startsWith("heading-")) {
        const baseHeading = this.types.get("heading");
        if (baseHeading) {
          return {
            ...DEFAULT_CONTENT_TYPE,
            ...baseHeading,
            label: `标题 ${type.split("-")[1]}`,
          };
        }
      }
      return { ...DEFAULT_CONTENT_TYPE, label: type };
    }

    return { ...DEFAULT_CONTENT_TYPE, ...specific };
  }

  // =========================================================================
  // SlashMenu API
  // =========================================================================

  /**
   * 获取所有已注册的 SlashMenu 项
   */
  getSlashMenuItems(): SlashMenuItemDefinition[] {
    const items: SlashMenuItemDefinition[] = [];

    for (const [blockType, config] of this.types.entries()) {
      if (config.slashMenuItems && config.slashMenuItems.length > 0) {
        items.push(...config.slashMenuItems);
      }
    }

    return items;
  }

  // =========================================================================
  // SideMenu API
  // =========================================================================

  /**
   * 获取指定块类型的 SideMenu Actions
   */
  getSideMenuActions(type: string): SideMenuActionDefinition[] {
    const config = this.types.get(type);
    return config?.sideMenuActions || [];
  }

  // =========================================================================
  // 实例级操作执行器管理 (保持与原 BlockCapabilities 兼容)
  // =========================================================================

  registerExecutor(
    blockId: string,
    actionId: string,
    executor: BlockActionExecutor
  ) {
    if (!this.executors.has(blockId)) {
      this.executors.set(blockId, new Map());
    }
    this.executors.get(blockId)!.set(actionId, executor);
  }

  unregisterExecutors(blockId: string) {
    this.executors.delete(blockId);
  }

  unregisterExecutor(blockId: string, actionId: string) {
    this.executors.get(blockId)?.delete(actionId);
  }

  getExecutor(
    blockId: string,
    actionId: string
  ): BlockActionExecutor | undefined {
    if (blockId === "__active_inline__") {
      return this.activeInline?.executor?.[actionId];
    }
    return this.executors.get(blockId)?.get(actionId);
  }

  executeAction(blockId: string, actionId: string, value?: any) {
    const executor = this.getExecutor(blockId, actionId);
    if (executor) {
      executor.execute(value);
    }
  }

  isActionActive(blockId: string, actionId: string): boolean {
    const executor = this.getExecutor(blockId, actionId);
    return executor?.isActive() ?? false;
  }

  getActionValue(blockId: string, actionId: string): any {
    const executor = this.getExecutor(blockId, actionId);
    return executor?.getValue?.();
  }

  getActionOptions(blockId: string, actionId: string): DropdownOption[] {
    const executor = this.getExecutor(blockId, actionId);
    return executor?.getOptions?.() ?? [];
  }

  // =========================================================================
  // Inline 内容激活管理
  // =========================================================================

  setActiveInline(
    type: string | null,
    executors: Record<string, BlockActionExecutor> = {}
  ) {
    if (type === null && this.activeInline === null) {
      return;
    }
    if (type === null) {
      this.activeInline = null;
    } else {
      this.activeInline = { type, executor: executors };
    }
    this.notify();
  }

  getActiveInline() {
    return this.activeInline;
  }

  executeInlineAction(actionId: string, value?: any) {
    if (this.activeInline && this.activeInline.executor[actionId]) {
      this.activeInline.executor[actionId].execute(value);
    }
  }

  // =========================================================================
  // 块焦点辅助
  // =========================================================================

  focusBlock(editor: any, blockId: string, position: "start" | "end" = "end") {
    try {
      editor.setTextCursorPosition(blockId, position);
    } catch (e) {
      console.warn("[ContentRegistry] focusBlock failed:", e);
    }
  }

  // =========================================================================
  // 订阅系统
  // =========================================================================

  subscribe(listener: () => void) {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  notify() {
    this.listeners.forEach((cb) => cb());
  }
}

// ============================================================================
// 导出
// ============================================================================

// 新导出名
export const contentRegistry = new ContentRegistry();

/**
 * 获取所有注册块类型的列表选项（用于块类型下拉）
 */
export function getRegisteredBlockTypes(
  editor: BlockNoteEditor<any, any, any>
): { value: string; label: string; icon: ReactNode; props?: any }[] {
  const specs = editor.schema.blockSpecs;
  const items: {
    value: string;
    label: string;
    icon: ReactNode;
    props?: any;
  }[] = [];

  const prioritizedTypes = [
    "paragraph",
    "heading-1",
    "heading-2",
    "heading-3",
    "bulletListItem",
    "numberedListItem",
    "checkListItem",
    "quote",
    "shikiCode",
    "math",
    "mermaid",
    "table",
    "image",
    "container",
    "vueComponent",
    "include",
  ];

  const processedTypes = new Set<string>();

  const addItem = (type: string, props?: any) => {
    const caps = contentRegistry.get(type);
    let icon = caps.icon;
    let label = caps.label;
    let value = type;

    if (type === "heading" && props?.level) {
      value = `heading-${props.level}`;
      const levelCaps = contentRegistry.get(value);
      if (levelCaps.label !== "Unknown Block") {
        label = levelCaps.label;
        icon = levelCaps.icon;
      }
    }

    items.push({ value, label, icon, props });
  };

  for (const pType of prioritizedTypes) {
    if (pType.startsWith("heading-")) {
      if ("heading" in specs && !processedTypes.has(pType)) {
        const level = parseInt(pType.split("-")[1]);
        addItem("heading", { level });
        processedTypes.add(pType);
      }
    } else if (pType in specs) {
      addItem(pType);
      processedTypes.add(pType);
    }
  }

  for (const type of Object.keys(specs)) {
    if (type === "heading") continue;
    if (type === "codeBlock") continue;
    if (processedTypes.has(type)) continue;
    addItem(type);
  }

  return items;
}
