/**
 * BlockCapabilities.ts - 块能力注册与管理中心
 *
 * 提供去中心化的块能力注册机制：
 * 1. 每个块自行注册其 capabilities
 * 2. 自定义 actions 由块内部实现，toolbar 仅负责触发
 * 3. 支持多种 action 类型：button, dropdown, input, toggle
 */
import { Block, BlockNoteEditor } from "@blocknote/core";
import { ReactNode } from "react";
import React from "react";
import { Icon } from "@iconify/react";

// --- Action 类型定义 ---

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
  iconOnly?: boolean; // 只显示图标，不显示当前选中值
}

export interface InputActionDefinition extends BaseActionDefinition {
  type: "input";
  placeholder?: string;
  width?: string; // e.g., "8rem"
  hideIcon?: boolean; // 隐藏图标
}

export interface ToggleActionDefinition extends BaseActionDefinition {
  type: "toggle";
}

export type BlockActionDefinition =
  | ButtonActionDefinition
  | DropdownActionDefinition
  | InputActionDefinition
  | ToggleActionDefinition;

// --- Executor 接口 ---

export interface DropdownOption {
  value: string;
  label: string;
  icon?: ReactNode;
}

export interface BlockActionExecutor {
  execute: (value?: any) => void; // value: 新选中项/输入值/toggle状态
  isActive: () => boolean; // 用于 button 高亮
  getValue?: () => any; // 当前值 (input/toggle/dropdown)
  getOptions?: () => DropdownOption[]; // dropdown 选项列表
}

// --- 块能力接口 ---

export interface BlockCapabilities {
  // supportedStyles: 支持的样式列表，如果为 true 则支持所有默认样式，如果为 false 或空数组则不支持任何样式
  supportedStyles: string[] | boolean;
  hasAlignment: boolean;
  hasIndent: boolean;
  hasTextColor: boolean;
  hasBackgroundColor: boolean;
  actions: BlockActionDefinition[];
  icon: ReactNode;
  label: string;
}

// --- 图标助手 ---
const iconify = (icon: string) =>
  React.createElement(Icon, { icon, className: "w-4 h-4" });

// 默认能力
const DEFAULT_CAPABILITIES: BlockCapabilities = {
  supportedStyles: ["bold", "italic", "underline", "strike", "code", "link"],
  hasAlignment: true,
  hasIndent: true,
  hasTextColor: true,
  hasBackgroundColor: true,
  actions: [],
  icon: iconify("lucide:help-circle"),
  label: "Unknown Block",
};

// --- 注册表类 ---

class BlockCapabilityRegistry {
  // 块类型 -> 能力配置
  private capabilities: Map<string, Partial<BlockCapabilities>> = new Map();

  // 块实例 ID -> 操作执行器映射 (blockId -> actionId -> executor)
  private executors: Map<string, Map<string, BlockActionExecutor>> = new Map();
  // 当前激活的 Inline 内容 (用于 InlineMath 等独立输入组件)
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

    this.register("paragraph", {
      icon: iconify("lucide:pilcrow"),
      label: "段落",
    });
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
    this.register("quote", { icon: iconify("lucide:quote"), label: "引用" });
    this.register("image", {
      icon: iconify("lucide:image"),
      label: "图片",
      supportedStyles: [],
    });
    this.register("table", { icon: iconify("lucide:table"), label: "表格" });

    this.initialized = true;
  }

  /**
   * 注册或更新块类型能力（由块组件调用）
   */
  register(type: string, caps: Partial<BlockCapabilities>) {
    const existing = this.capabilities.get(type) || {};
    this.capabilities.set(type, { ...existing, ...caps });
    this.notify();
  }

  /**
   * 获取块能力
   */
  get(type: string): BlockCapabilities {
    const specific = this.capabilities.get(type);

    if (!specific) {
      if (type.startsWith("heading-")) {
        const baseHeading = this.capabilities.get("heading");
        if (baseHeading) {
          return {
            ...DEFAULT_CAPABILITIES,
            ...baseHeading,
            label: `标题 ${type.split("-")[1]}`,
          };
        }
      }
      return { ...DEFAULT_CAPABILITIES, label: type };
    }

    return { ...DEFAULT_CAPABILITIES, ...specific };
  }

  // --- 实例级操作执行器管理 ---

  /**
   * 注册块实例的操作执行器（由块组件 mount 时调用）
   */
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

  /**
   * 注销块实例的所有执行器（由块组件 unmount 时调用）
   */
  unregisterExecutors(blockId: string) {
    this.executors.delete(blockId);
  }

  /**
   * 注销块实例的单个执行器
   */
  unregisterExecutor(blockId: string, actionId: string) {
    this.executors.get(blockId)?.delete(actionId);
  }

  /**
   * 获取指定块实例的操作执行器
   */
  getExecutor(
    blockId: string,
    actionId: string
  ): BlockActionExecutor | undefined {
    if (blockId === "__active_inline__") {
      return this.activeInline?.executor?.[actionId];
    }
    return this.executors.get(blockId)?.get(actionId);
  }

  /**
   * 执行指定块的操作
   */
  executeAction(blockId: string, actionId: string, value?: any) {
    const executor = this.getExecutor(blockId, actionId);
    if (executor) {
      executor.execute(value);
    }
  }

  /**
   * 检查指定块的操作是否激活
   */
  isActionActive(blockId: string, actionId: string): boolean {
    const executor = this.getExecutor(blockId, actionId);
    return executor?.isActive() ?? false;
  }

  /**
   * 获取 action 当前值 (用于 dropdown/input/toggle)
   */
  getActionValue(blockId: string, actionId: string): any {
    const executor = this.getExecutor(blockId, actionId);
    return executor?.getValue?.();
  }

  /**
   * 获取 dropdown 选项
   */
  getActionOptions(blockId: string, actionId: string): DropdownOption[] {
    const executor = this.getExecutor(blockId, actionId);
    return executor?.getOptions?.() ?? [];
  }

  // --- Inline 内容激活管理 ---

  /**
   * 设置当前激活的 Inline 内容及其实例执行器
   */
  setActiveInline(
    type: string | null,
    executors: Record<string, BlockActionExecutor> = {}
  ) {
    // Guard: 如果没有变化，不触发 notify 避免无限循环
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

  /**
   * 执行 Active Inline 的操作
   */
  executeInlineAction(actionId: string, value?: any) {
    if (this.activeInline && this.activeInline.executor[actionId]) {
      this.activeInline.executor[actionId].execute(value);
    }
  }

  // --- 块焦点辅助 ---

  /**
   * 将 BlockNote 的选区设置到指定块
   *
   * 用于自定义块（如 MathBlock）在其内部元素获焦时，
   * 同步更新 BlockNote 的选区，使 StaticToolbar 能正确检测当前块类型。
   *
   * @param editor BlockNote 编辑器实例
   * @param blockId 块 ID
   * @param position 光标位置，默认 "end"
   */
  focusBlock(editor: any, blockId: string, position: "start" | "end" = "end") {
    try {
      editor.setTextCursorPosition(blockId, position);
    } catch (e) {
      // 某些块可能不支持 setTextCursorPosition，静默忽略
      console.warn("[BlockRegistry] focusBlock failed:", e);
    }
  }

  // --- 订阅系统 ---

  subscribe(listener: () => void) {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  /**
   * 触发所有订阅者更新（块内部状态变化时调用）
   */
  notify() {
    this.listeners.forEach((cb) => cb());
  }
}

// 导出单例
export const blockRegistry = new BlockCapabilityRegistry();

// 兼容旧 API
export function getBlockCapabilities(blockType: string): BlockCapabilities {
  return blockRegistry.get(blockType);
}

/**
 * 获取所有注册块类型的列表选项
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
    const caps = blockRegistry.get(type);
    let icon = caps.icon;
    let label = caps.label;
    let value = type;

    if (type === "heading" && props?.level) {
      value = `heading-${props.level}`;
      const levelCaps = blockRegistry.get(value);
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
