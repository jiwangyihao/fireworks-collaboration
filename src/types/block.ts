/**
 * Block 类型定义
 *
 * 本文件定义编辑器内容模型的核心类型，作为 E0 阶段的基础设施。
 * 用于 E1（VitePress 项目集成）、E2（块编辑器）、E4（PDF 导入）。
 */

// ============================================================================
// 基础类型
// ============================================================================

/**
 * 生成唯一 Block ID
 */
export function generateBlockId(): string {
  return `block-${Date.now()}-${Math.random().toString(36).substring(2, 9)}`;
}

/**
 * PDF 来源信息（E4 阶段使用）
 * 用于追踪 Block 在 PDF 中的原始位置
 */
export interface BlockSource {
  /** PDF 页码（从 0 开始） */
  pageIndex: number;
  /** 边界框坐标 [x1, y1, x2, y2]（像素坐标） */
  bbox: [number, number, number, number];
}

/**
 * 行内内容类型
 * 用于表示段落、标题等块内的文本内容
 */
export interface InlineContent {
  type: "text" | "link" | "code" | "emphasis" | "strong" | "inlineMath";
  text?: string;
  href?: string;
  children?: InlineContent[];
  /** 行内数学公式（$...$） */
  formula?: string;
  /** 是否为行内显示模式（$$...$$） */
  displayMode?: boolean;
}

/**
 * 纯文本行内内容
 */
export interface TextContent extends InlineContent {
  type: "text";
  text: string;
}

/**
 * 链接行内内容
 */
export interface LinkContent extends InlineContent {
  type: "link";
  href: string;
  children: InlineContent[];
}

/**
 * 行内代码
 */
export interface CodeContent extends InlineContent {
  type: "code";
  text: string;
}

/**
 * 强调（斜体）
 */
export interface EmphasisContent extends InlineContent {
  type: "emphasis";
  children: InlineContent[];
}

/**
 * 加粗
 */
export interface StrongContent extends InlineContent {
  type: "strong";
  children: InlineContent[];
}

/**
 * 行内数学公式
 */
export interface InlineMathContent extends InlineContent {
  type: "inlineMath";
  formula: string;
  /** 是否为行内显示模式（$$...$$），默认为 false ($...$) */
  displayMode?: boolean;
}

// ============================================================================
// Block 基础接口
// ============================================================================

/**
 * Block 基础接口
 * 所有 Block 类型都继承此接口
 */
export interface BaseBlock {
  /** 唯一标识符 */
  id: string;
  /** Block 类型 */
  type: BlockType;
  /** 子 Block（用于嵌套结构） */
  children?: Block[];
  /** PDF 来源信息（E4 阶段使用） */
  source?: BlockSource;
}

// ============================================================================
// Block 类型枚举
// ============================================================================

/**
 * 标准块类型（对应 Markdown 原生语法）
 */
export type StandardBlockType =
  | "heading"
  | "paragraph"
  | "bulletListItem"
  | "numberedListItem"
  | "checkListItem"
  | "codeBlock"
  | "table"
  | "image"
  | "quote"
  | "thematicBreak";

/**
 * 自定义块类型（VitePress 扩展语法）
 */
export type CustomBlockType =
  | "container"
  | "math"
  | "mermaid"
  | "vueComponent"
  | "include"
  | "shikiCode";

/**
 * 所有 Block 类型的联合
 */
export type BlockType = StandardBlockType | CustomBlockType;

// ============================================================================
// 标准块类型定义
// ============================================================================

/**
 * 标题块
 */
export interface HeadingBlock extends BaseBlock {
  type: "heading";
  props: {
    level: 1 | 2 | 3 | 4 | 5 | 6;
  };
  content: InlineContent[];
}

/**
 * 段落块
 */
export interface ParagraphBlock extends BaseBlock {
  type: "paragraph";
  content: InlineContent[];
}

/**
 * 无序列表项
 */
export interface BulletListItemBlock extends BaseBlock {
  type: "bulletListItem";
  content: InlineContent[];
  children?: Block[];
}

/**
 * 有序列表项
 */
export interface NumberedListItemBlock extends BaseBlock {
  type: "numberedListItem";
  props: {
    /** 列表项编号（从 1 开始） */
    start?: number;
  };
  content: InlineContent[];
  children?: Block[];
}

/**
 * 复选框列表项
 */
export interface CheckListItemBlock extends BaseBlock {
  type: "checkListItem";
  props: {
    checked: boolean;
  };
  content: InlineContent[];
  children?: Block[];
}

/**
 * 代码块
 */
export interface CodeBlockBlock extends BaseBlock {
  type: "codeBlock";
  props: {
    /** 代码语言 */
    language?: string;
    /** 代码内容 */
    code: string;
    /** 行高亮（如 {1,3-5}） */
    highlightLines?: string;
    /** 文件名显示 */
    filename?: string;
  };
}

/**
 * 表格单元格
 */
export interface TableCell {
  content: InlineContent[];
  /** 单元格对齐方式 */
  align?: "left" | "center" | "right";
}

/**
 * 表格行
 */
export interface TableRow {
  cells: TableCell[];
}

/**
 * 表格块
 */
export interface TableBlock extends BaseBlock {
  type: "table";
  props: {
    /** 表头行 */
    headerRow: TableRow;
    /** 数据行 */
    rows: TableRow[];
  };
}

/**
 * 图片块
 */
export interface ImageBlock extends BaseBlock {
  type: "image";
  props: {
    /** 图片 URL 或路径 */
    src: string;
    /** 替代文本 */
    alt?: string;
    /** 标题 */
    title?: string;
    /** 图片宽度 */
    width?: number | string;
    /** 图片高度 */
    height?: number | string;
  };
}

/**
 * 引用块
 */
export interface QuoteBlock extends BaseBlock {
  type: "quote";
  props?: {
    groupId?: string;
  };
  children: Block[];
}

/**
 * 分割线块
 */
export interface ThematicBreakBlock extends BaseBlock {
  type: "thematicBreak";
}

// ============================================================================
// 自定义块类型定义（VitePress 扩展）
// ============================================================================

/**
 * VitePress 容器类型（与 VitePress 官方一致）
 */
export type ContainerType = "tip" | "info" | "warning" | "danger" | "details";

/**
 * 容器块（VitePress :::tip 等）
 * 内容的第一行可作为标题，如果是默认标题则保存时不写入 Markdown
 */
export interface ContainerBlock extends BaseBlock {
  type: "container";
  props: {
    /** 容器类型 */
    containerType: ContainerType;
  };
  /** 内容（第一行可作为标题） */
  content: InlineContent[];
}

/**
 * 数学公式块
 */
export interface MathBlock extends BaseBlock {
  type: "math";
  props: {
    /** LaTeX 公式内容 */
    formula: string;
    /** 显示模式：inline（行内）或 block（块级） */
    display: "inline" | "block";
  };
}

/**
 * Mermaid 图表块
 */
export interface MermaidBlock extends BaseBlock {
  type: "mermaid";
  props: {
    /** Mermaid 代码 */
    code: string;
  };
}

/**
 * Vue 组件块（VitePress 中的自定义组件）
 */
export interface VueComponentBlock extends BaseBlock {
  type: "vueComponent";
  props: {
    /** 组件名称（如 "OList"） */
    componentName: string;
    /** 组件属性 */
    attributes: Record<string, string | number | boolean>;
    /** 是否自闭合标签 */
    selfClosing: boolean;
  };
  /** 组件插槽内容（非自闭合时） */
  children?: Block[];
}

/**
 * @include 指令块（VitePress 文件包含）
 */
export interface IncludeBlock extends BaseBlock {
  type: "include";
  props: {
    /** 文件路径（支持 @/ 别名） */
    path: string;
    /** 行范围（可选） */
    lineRange?: {
      start?: number;
      end?: number;
    };
    /** 区域名称（#region，可选） */
    region?: string;
  };
}

/**
 * Shiki 代码块（VitePress 高级语法支持）
 */
export interface ShikiCodeBlock extends BaseBlock {
  type: "shikiCode";
  props: {
    /** 代码内容 */
    code: string;
    /** 语言 */
    language: string;
    /** 文件名 [foo.ts] */
    filename?: string;
    /** 高亮行 {1,3-5} */
    highlightLines?: string;
    /** 是否显示行号 :line-numbers */
    showLineNumbers?: boolean;
    /** 起始行号 :line-numbers=2 */
    startLineNumber?: number;
    /** 代码组 Tabs (JSON string) */
    tabs?: string;
    /** 当前激活 Tab 索引 */
    activeTabIndex?: number;
  };
}

// ============================================================================
// Block 联合类型
// ============================================================================

/**
 * 标准块联合类型
 */
export type StandardBlock =
  | HeadingBlock
  | ParagraphBlock
  | BulletListItemBlock
  | NumberedListItemBlock
  | CheckListItemBlock
  | CodeBlockBlock
  | TableBlock
  | ImageBlock
  | QuoteBlock
  | ThematicBreakBlock;

/**
 * 自定义块联合类型
 */
export type CustomBlock =
  | ContainerBlock
  | MathBlock
  | MermaidBlock
  | VueComponentBlock
  | IncludeBlock
  | ShikiCodeBlock;

/**
 * 所有 Block 类型的联合
 */
export type Block = StandardBlock | CustomBlock;

// ============================================================================
// 类型守卫函数
// ============================================================================

/**
 * 检查是否为标准块类型
 */
export function isStandardBlock(block: Block): block is StandardBlock {
  const standardTypes: StandardBlockType[] = [
    "heading",
    "paragraph",
    "bulletListItem",
    "numberedListItem",
    "checkListItem",
    "codeBlock",
    "table",
    "image",
    "quote",
    "thematicBreak",
  ];
  return standardTypes.includes(block.type as StandardBlockType);
}

/**
 * 检查是否为自定义块类型
 */
export function isCustomBlock(block: Block): block is CustomBlock {
  const customTypes: CustomBlockType[] = [
    "container",
    "math",
    "mermaid",
    "vueComponent",
    "include",
  ];
  return customTypes.includes(block.type as CustomBlockType);
}

/**
 * 检查是否为列表项块
 */
export function isListItemBlock(
  block: Block
): block is BulletListItemBlock | NumberedListItemBlock | CheckListItemBlock {
  return ["bulletListItem", "numberedListItem", "checkListItem"].includes(
    block.type
  );
}

/**
 * 检查是否为容器类块（可包含子块）
 */
export function isContainerBlock(
  block: Block
): block is QuoteBlock | ContainerBlock | VueComponentBlock {
  return ["quote", "container", "vueComponent"].includes(block.type);
}

// ============================================================================
// 创建辅助函数
// ============================================================================

/**
 * 创建文本行内内容
 */
export function createTextContent(text: string): TextContent {
  return { type: "text", text };
}

/**
 * 创建段落块
 */
export function createParagraphBlock(
  content: InlineContent[] | string
): ParagraphBlock {
  const normalizedContent =
    typeof content === "string" ? [createTextContent(content)] : content;
  return {
    id: generateBlockId(),
    type: "paragraph",
    content: normalizedContent,
  };
}

/**
 * 创建标题块
 */
export function createHeadingBlock(
  level: 1 | 2 | 3 | 4 | 5 | 6,
  content: InlineContent[] | string
): HeadingBlock {
  const normalizedContent =
    typeof content === "string" ? [createTextContent(content)] : content;
  return {
    id: generateBlockId(),
    type: "heading",
    props: { level },
    content: normalizedContent,
  };
}

/**
 * 创建代码块
 */
export function createCodeBlock(
  code: string,
  language?: string
): CodeBlockBlock {
  return {
    id: generateBlockId(),
    type: "codeBlock",
    props: { code, language },
  };
}

/**
 * 创建容器块
 */
export function createContainerBlock(
  containerType: ContainerType,
  content: InlineContent[] = []
): ContainerBlock {
  return {
    id: generateBlockId(),
    type: "container",
    props: { containerType },
    content,
  };
}

/**
 * 创建数学公式块
 */
export function createMathBlock(
  formula: string,
  display: "inline" | "block" = "block"
): MathBlock {
  return {
    id: generateBlockId(),
    type: "math",
    props: { formula, display },
  };
}

/**
 * 创建 Mermaid 块
 */
export function createMermaidBlock(code: string): MermaidBlock {
  return {
    id: generateBlockId(),
    type: "mermaid",
    props: { code },
  };
}

/**
 * 创建 Shiki 代码块
 */
export function createShikiCodeBlock(
  code: string,
  language: string = "text",
  options: Partial<ShikiCodeBlock["props"]> = {}
): ShikiCodeBlock {
  return {
    id: generateBlockId(),
    type: "shikiCode",
    props: {
      code,
      language,
      ...options,
    },
  };
}
