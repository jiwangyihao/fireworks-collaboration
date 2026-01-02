/**
 * Markdown → Block 转换器
 *
 * 使用 unified + remark 将 Markdown 解析为 Block 类型。
 * 支持 VitePress 扩展语法：容器、公式、Mermaid、Vue 组件、@include 指令。
 */

import { unified } from "unified";
import remarkParse from "remark-parse";
import remarkGfm from "remark-gfm";
import remarkFrontmatter from "remark-frontmatter";
import remarkDirective from "remark-directive";
import remarkMath from "remark-math";
import yaml from "yaml";
import type { Root, RootContent, PhrasingContent } from "mdast";

import {
  type Block,
  type InlineContent,
  type HeadingBlock,
  type ParagraphBlock,
  type BulletListItemBlock,
  type NumberedListItemBlock,
  type CheckListItemBlock,
  type CodeBlockBlock,
  type TableBlock,
  type TableRow,
  type TableCell,
  type ImageBlock,
  type QuoteBlock,
  type ThematicBreakBlock,
  type ContainerBlock,
  type ContainerType,
  type MathBlock,
  type MermaidBlock,
  type VueComponentBlock,
  type IncludeBlock,
  generateBlockId,
} from "../types/block";
import type { Frontmatter, Document } from "../types/document";

// ============================================================================
// 解析器配置
// ============================================================================

/**
 * 创建 unified 处理器
 */
function createProcessor() {
  return unified()
    .use(remarkParse)
    .use(remarkGfm) // 支持 GFM：表格、复选框、删除线等
    .use(remarkFrontmatter, ["yaml"])
    .use(remarkDirective)
    .use(remarkMath)
    .use(remarkEnrichMath);
}

/**
 * Custom plugin to enrich math nodes with source information.
 * Specifically checks if inlineMath nodes are wrapped in $$ (double dollars) in the source.
 */
function remarkEnrichMath() {
  return (tree: any, file: any) => {
    const visit = (node: any) => {
      if (node.type === "inlineMath") {
        const start = node.position?.start?.offset;
        const end = node.position?.end?.offset;
        if (
          typeof start === "number" &&
          typeof end === "number" &&
          file.value
        ) {
          const raw = file.value.slice(start, end);
          if (raw.startsWith("$$")) {
            node.data = node.data || {};
            node.data.displayMode = true;
          }
        }
      }
      if (node.children) {
        node.children.forEach(visit);
      }
    };
    visit(tree);
  };
}

// ============================================================================
// Frontmatter 解析
// ============================================================================

/**
 * 从 MDAST 中提取 Frontmatter
 */
function extractFrontmatter(tree: Root): Frontmatter {
  const yamlNode = tree.children.find((node) => node.type === "yaml");
  if (!yamlNode || !("value" in yamlNode)) {
    return {};
  }

  try {
    return yaml.parse(yamlNode.value as string) || {};
  } catch {
    console.warn("Failed to parse frontmatter YAML");
    return {};
  }
}

// ============================================================================
// 行内内容转换
// ============================================================================

/**
 * 将 MDAST PhrasingContent 转换为 InlineContent
 */
function convertPhrasingContent(node: PhrasingContent): InlineContent {
  switch (node.type) {
    case "text":
      return { type: "text", text: node.value };

    case "emphasis":
      return {
        type: "emphasis",
        children: node.children.map(convertPhrasingContent),
      };

    case "strong":
      return {
        type: "strong",
        children: node.children.map(convertPhrasingContent),
      };

    case "inlineCode":
      return { type: "code", text: node.value };

    case "link":
      return {
        type: "link",
        href: node.url,
        children: node.children.map(convertPhrasingContent),
      };

    case "inlineMath":
      return {
        type: "inlineMath",
        formula: node.value,
        displayMode: (node.data as any)?.displayMode || false,
      };

    default:
      // 对于未处理的节点类型，尝试提取文本
      if ("value" in node) {
        return { type: "text", text: String(node.value) };
      }
      if ("children" in node && Array.isArray(node.children)) {
        // 递归处理子节点
        const texts = (node.children as PhrasingContent[])
          .map(convertPhrasingContent)
          .map((c) => ("text" in c ? c.text : ""))
          .join("");
        return { type: "text", text: texts };
      }
      return { type: "text", text: "" };
  }
}

/**
 * 将 MDAST 子节点数组转换为 InlineContent 数组
 */
function convertInlineContent(
  children: PhrasingContent[] | undefined
): InlineContent[] {
  if (!children) return [];
  return children.map(convertPhrasingContent);
}

// ============================================================================
// VitePress 特殊语法检测
// ============================================================================

/**
 * 检测 Vue 组件标签
 * 匹配模式：<ComponentName prop="value" /> 或 <ComponentName>...</ComponentName>
 */
function parseVueComponent(html: string): {
  name: string;
  attrs: Record<string, string | boolean>;
  selfClosing: boolean;
} | null {
  // 自闭合标签: <Component prop="value" />
  const selfClosingMatch = html.match(
    /^<([A-Z][a-zA-Z0-9]*)\s*([^>]*?)\s*\/?>$/s
  );
  if (selfClosingMatch) {
    const [, name, attrString] = selfClosingMatch;
    return {
      name,
      attrs: parseAttributes(attrString),
      selfClosing: true,
    };
  }

  // 非自闭合标签: <Component>...</Component>
  const openTagMatch = html.match(/^<([A-Z][a-zA-Z0-9]*)\s*([^>]*)>/);
  if (openTagMatch) {
    const [, name, attrString] = openTagMatch;
    return {
      name,
      attrs: parseAttributes(attrString),
      selfClosing: false,
    };
  }

  return null;
}

/**
 * 解析 HTML 属性字符串
 */
function parseAttributes(attrString: string): Record<string, string | boolean> {
  const attrs: Record<string, string | boolean> = {};
  // 匹配 key="value" 或 key='value' 或 key（布尔属性）
  const attrRegex =
    /([a-zA-Z_:][a-zA-Z0-9_:.-]*)\s*(?:=\s*(?:"([^"]*)"|'([^']*)'|([^\s>]+)))?/g;
  let match;
  while ((match = attrRegex.exec(attrString)) !== null) {
    const [, key, doubleQuoted, singleQuoted, unquoted] = match;
    if (doubleQuoted !== undefined) {
      attrs[key] = doubleQuoted;
    } else if (singleQuoted !== undefined) {
      attrs[key] = singleQuoted;
    } else if (unquoted !== undefined) {
      attrs[key] = unquoted;
    } else {
      // 布尔属性
      attrs[key] = true;
    }
  }
  return attrs;
}

/**
 * 检测 @include 指令
 * 匹配模式：<!--@include: path{lines}#region-->
 */
function parseIncludeDirective(html: string): {
  path: string;
  lineRange?: { start?: number; end?: number };
  region?: string;
} | null {
  const match = html.match(
    /<!--\s*@include:\s*([^\s{}#]+)(?:#([^\s{}]+))?(?:\{(\d*)-(\d*)\})?\s*-->/
  );
  if (!match) return null;

  const [, path, region, startStr, endStr] = match;
  const result: {
    path: string;
    lineRange?: { start?: number; end?: number };
    region?: string;
  } = {
    path,
  };

  if (region) {
    result.region = region;
  }

  if (startStr !== undefined || endStr !== undefined) {
    result.lineRange = {};
    if (startStr) result.lineRange.start = parseInt(startStr, 10);
    if (endStr) result.lineRange.end = parseInt(endStr, 10);
  }

  return result;
}

// ============================================================================
// Block 转换
// ============================================================================

/**
 * 将 MDAST 节点转换为 Block
 */
function convertNode(node: RootContent): Block | Block[] | null {
  switch (node.type) {
    case "heading":
      return {
        id: generateBlockId(),
        type: "heading",
        props: { level: node.depth as 1 | 2 | 3 | 4 | 5 | 6 },
        content: convertInlineContent(node.children),
      } as HeadingBlock;

    case "paragraph": {
      // 检测单图片段落：如果段落只包含一个 image 子节点，提取为独立的 ImageBlock
      if (node.children.length === 1 && node.children[0].type === "image") {
        const imgNode = node.children[0];
        return {
          id: generateBlockId(),
          type: "image",
          props: {
            src: imgNode.url,
            alt: imgNode.alt || undefined,
            title: imgNode.title || undefined,
          },
        } as ImageBlock;
      }

      return {
        id: generateBlockId(),
        type: "paragraph",
        content: convertInlineContent(node.children),
      } as ParagraphBlock;
    }

    case "blockquote":
      return {
        id: generateBlockId(),
        type: "quote",
        children: convertNodes(node.children),
      } as QuoteBlock;

    case "list": {
      // 将列表转换为多个列表项 Block
      const blocks: Block[] = [];
      const isOrdered = node.ordered || false;
      let startNumber = node.start || 1;

      for (const item of node.children) {
        if (item.type !== "listItem") continue;

        const isChecked = item.checked;
        const content: InlineContent[] = [];
        const childBlocks: Block[] = [];

        for (const child of item.children) {
          if (child.type === "paragraph") {
            content.push(...convertInlineContent(child.children));
          } else if (child.type === "list") {
            // 嵌套列表
            const nestedBlocks = convertNode(child);
            if (nestedBlocks) {
              if (Array.isArray(nestedBlocks)) {
                childBlocks.push(...nestedBlocks);
              } else {
                childBlocks.push(nestedBlocks);
              }
            }
          } else {
            const converted = convertNode(child);
            if (converted) {
              if (Array.isArray(converted)) {
                childBlocks.push(...converted);
              } else {
                childBlocks.push(converted);
              }
            }
          }
        }

        if (isChecked !== null && isChecked !== undefined) {
          // 复选框列表项
          blocks.push({
            id: generateBlockId(),
            type: "checkListItem",
            props: { checked: isChecked },
            content,
            children: childBlocks.length > 0 ? childBlocks : undefined,
          } as CheckListItemBlock);
        } else if (isOrdered) {
          // 有序列表项
          blocks.push({
            id: generateBlockId(),
            type: "numberedListItem",
            props: { start: startNumber++ },
            content,
            children: childBlocks.length > 0 ? childBlocks : undefined,
          } as NumberedListItemBlock);
        } else {
          // 无序列表项
          blocks.push({
            id: generateBlockId(),
            type: "bulletListItem",
            content,
            children: childBlocks.length > 0 ? childBlocks : undefined,
          } as BulletListItemBlock);
        }
      }
      return blocks;
    }

    case "code": {
      const language = node.lang || undefined;

      // 检测 Mermaid 代码块
      if (language === "mermaid") {
        return {
          id: generateBlockId(),
          type: "mermaid",
          props: { code: node.value },
        } as MermaidBlock;
      }

      // 普通代码块
      return {
        id: generateBlockId(),
        type: "codeBlock",
        props: {
          language,
          code: node.value,
          filename: node.meta || undefined,
        },
      } as CodeBlockBlock;
    }

    case "table": {
      const rows = node.children;
      if (rows.length === 0) return null;

      const convertTableRow = (
        row: (typeof rows)[0],
        align?: (string | null)[]
      ): TableRow => {
        return {
          cells: row.children.map(
            (cell, idx) =>
              ({
                content: convertInlineContent(cell.children),
                align:
                  (align?.[idx] as "left" | "center" | "right") || undefined,
              }) as TableCell
          ),
        };
      };

      const headerRow = convertTableRow(rows[0], node.align || undefined);
      const dataRows = rows
        .slice(1)
        .map((row) => convertTableRow(row, node.align || undefined));

      return {
        id: generateBlockId(),
        type: "table",
        props: {
          headerRow,
          rows: dataRows,
        },
      } as TableBlock;
    }

    case "image":
      return {
        id: generateBlockId(),
        type: "image",
        props: {
          src: node.url,
          alt: node.alt || undefined,
          title: node.title || undefined,
        },
      } as ImageBlock;

    case "thematicBreak":
      return {
        id: generateBlockId(),
        type: "thematicBreak",
      } as ThematicBreakBlock;

    case "math":
      return {
        id: generateBlockId(),
        type: "math",
        props: {
          formula: node.value,
          display: "block",
        },
      } as MathBlock;

    case "html": {
      const html = node.value;

      // 检测 Vue 组件
      const vueComponent = parseVueComponent(html);
      if (vueComponent) {
        return {
          id: generateBlockId(),
          type: "vueComponent",
          props: {
            componentName: vueComponent.name,
            attributes: vueComponent.attrs,
            selfClosing: vueComponent.selfClosing,
          },
        } as VueComponentBlock;
      }

      // 检测 @include 指令
      const includeDirective = parseIncludeDirective(html);
      if (includeDirective) {
        return {
          id: generateBlockId(),
          type: "include",
          props: {
            path: includeDirective.path,
            lineRange: includeDirective.lineRange,
            region: includeDirective.region,
          },
        } as IncludeBlock;
      }

      // 其他 HTML 作为段落处理
      return {
        id: generateBlockId(),
        type: "paragraph",
        content: [{ type: "text", text: html }],
      } as ParagraphBlock;
    }

    // 处理 remark-directive 的容器指令（:::tip 等）
    case "containerDirective": {
      const directiveNode = node as unknown as {
        name: string;
        children: RootContent[];
        attributes?: Record<string, string>;
      };

      const containerType = directiveNode.name as ContainerType;
      const validContainerTypes: ContainerType[] = [
        "tip",
        "warning",
        "danger",
        "details",
        "info",
        "note",
      ];

      if (validContainerTypes.includes(containerType)) {
        // 提取自定义标题（如果有）
        let title: string | undefined;
        const childBlocks: Block[] = [];

        for (const child of directiveNode.children) {
          const converted = convertNode(child as RootContent);
          if (converted) {
            if (Array.isArray(converted)) {
              childBlocks.push(...converted);
            } else {
              childBlocks.push(converted);
            }
          }
        }

        // 尝试从属性中获取标题
        if (directiveNode.attributes?.title) {
          title = directiveNode.attributes.title;
        }

        return {
          id: generateBlockId(),
          type: "container",
          props: { containerType, title },
          children: childBlocks,
        } as ContainerBlock;
      }

      // 未知的容器类型，作为引用块处理
      return {
        id: generateBlockId(),
        type: "quote",
        children: convertNodes(directiveNode.children as RootContent[]),
      } as QuoteBlock;
    }

    case "yaml":
      // Frontmatter 已单独处理，跳过
      return null;

    default:
      // 未处理的节点类型，记录警告
      console.warn(`Unhandled MDAST node type: ${node.type}`);
      return null;
  }
}

/**
 * 将多个 MDAST 节点转换为 Block 数组
 */
function convertNodes(nodes: RootContent[]): Block[] {
  const blocks: Block[] = [];
  for (const node of nodes) {
    const converted = convertNode(node);
    if (converted) {
      if (Array.isArray(converted)) {
        blocks.push(...converted);
      } else {
        blocks.push(converted);
      }
    }
  }
  return blocks;
}

// ============================================================================
// 公共 API
// ============================================================================

/**
 * 将 Markdown 文本转换为 Block 数组
 */
export function markdownToBlocks(markdown: string): Block[] {
  const processor = createProcessor();
  const parsedTree = processor.parse(markdown);
  const tree = processor.runSync(parsedTree, markdown) as Root;
  const blocks = convertNodes(tree.children);
  console.log(
    "[Parser] Parsed blocks from markdown:",
    JSON.stringify(blocks, null, 2)
  );
  return blocks;
}

/**
 * 解析 Markdown 文档（包括 Frontmatter）
 */
export function parseMarkdownDocument(
  markdown: string,
  path: string = ""
): Document {
  const processor = createProcessor();
  const parsedTree = processor.parse(markdown);
  const tree = processor.runSync(parsedTree, markdown) as Root;

  const frontmatter = extractFrontmatter(tree);
  const blocks = convertNodes(tree.children);

  return {
    path,
    frontmatter,
    blocks,
    rawContent: markdown,
    status: "clean",
  };
}

/**
 * 从 Markdown 中提取 Frontmatter
 */
export function extractMarkdownFrontmatter(markdown: string): Frontmatter {
  const processor = createProcessor();
  const tree = processor.parse(markdown) as Root;
  return extractFrontmatter(tree);
}
