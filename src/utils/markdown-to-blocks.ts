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
  createShikiCodeBlock,
} from "../types/block";
import type { Frontmatter, Document } from "../types/document";

// 容器类型默认标题（与 VitePress 官方一致）
const containerDefaultTitles: Record<ContainerType, string> = {
  tip: "提示",
  info: "信息",
  warning: "警告",
  danger: "危险",
  details: "详情",
};

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
    .use(remarkEnrichMath)
    .use(remarkVitePressContainers); // 处理 VitePress ::: container 语法
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

/**
 * 自定义插件：处理 VitePress 容器语法 (::: info, ::: tip 等)
 * 将 `::: type` 格式转换为 containerDirective 节点
 */
function remarkVitePressContainers() {
  return (tree: any) => {
    const visit = (node: any, index: number, parent: any) => {
      // 查找包含 `::: type` 格式的段落
      if (node.type === "paragraph" && node.children?.[0]?.type === "text") {
        const text = node.children[0].value as string;

        // 模式1: 单段落完整包含容器 (START ... END)
        // 匹配：::: type [title]\n content \n:::
        const singleBlockMatch = text.match(
          /^:::\s*(info|tip|warning|danger|details|note|code-group)(?:[ \t]+(.*?))?\n([\s\S]*?)\n:::\s*$/i
        );

        if (singleBlockMatch) {
          const containerType = singleBlockMatch[1].toLowerCase();
          const title = singleBlockMatch[2]?.trim() || "";
          const contentText = singleBlockMatch[3];

          // 解析内部 Markdown 内容
          // 使用与 createProcessor 相同的插件配置（除了本插件自身，防止递归死循环其实也不太可能）
          // 这里为了简单，创建一个新的 processor
          // 注意：需要确保 remarkMath 等插件被包含，否则内部公式无法解析
          const innerProcessor = unified()
            .use(remarkParse)
            .use(remarkGfm)
            .use(remarkDirective)
            .use(remarkMath)
            .use(remarkEnrichMath); // 复用文件内的 helper

          const innerRoot = innerProcessor.parse(contentText);
          const containerChildren = innerRoot.children;

          const containerNode = {
            type: "containerDirective",
            name: containerType,
            attributes: { title },
            children: containerChildren,
          };

          parent.children.splice(index, 1, containerNode);
          return index; // 继续
        }

        // 模式2: 多段落容器 (START ... 寻找 sibling END)
        const containerStartMatch = text.match(
          /^:::\s*(info|tip|warning|danger|details|note|code-group)(?:\s+(.*))?$/im
        );

        if (containerStartMatch) {
          const containerType = containerStartMatch[1].toLowerCase();
          const title = containerStartMatch[2]?.trim() || "";

          // 找到对应的结束标记 `:::`
          const siblings = parent?.children || [];
          let endIndex = -1;
          const containerChildren: any[] = [];

          for (let i = index + 1; i < siblings.length; i++) {
            const sibling = siblings[i];
            if (
              sibling.type === "paragraph" &&
              sibling.children?.[0]?.type === "text" &&
              sibling.children[0].value.trim() === ":::"
            ) {
              endIndex = i;
              break;
            }
            containerChildren.push(sibling);
          }

          if (endIndex > index) {
            // 创建 containerDirective 节点
            const containerNode = {
              type: "containerDirective",
              name: containerType,
              attributes: { title },
              children: containerChildren,
            };

            // 替换原节点和内容
            siblings.splice(index, endIndex - index + 1, containerNode);
            return index; // 返回当前索引继续处理
          }
        }
      }

      // 递归处理子节点
      if (node.children) {
        for (let i = 0; i < node.children.length; i++) {
          const result = visit(node.children[i], i, node);
          if (typeof result === "number") {
            i = result; // 如果节点被替换，调整索引
          }
        }
      }
    };

    visit(tree, 0, null);
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

      // 检测单 displayMode 公式段落：如果段落只包含一个 inlineMath 且 displayMode=true，转为 MathBlock
      if (
        node.children.length === 1 &&
        node.children[0].type === "inlineMath"
      ) {
        const mathNode = node.children[0] as any;
        // displayMode 可能在 data.displayMode 或 data.hChildren.displayMode
        const isDisplayMode =
          mathNode.data?.displayMode === true ||
          (mathNode.data?.hChildren &&
            mathNode.data.hChildren.some((c: any) => c.displayMode === true));
        if (isDisplayMode) {
          return {
            id: generateBlockId(),
            type: "math",
            props: {
              formula: mathNode.value || "",
            },
          } as MathBlock;
        }
      }

      return {
        id: generateBlockId(),
        type: "paragraph",
        content: convertInlineContent(node.children),
      } as ParagraphBlock;
    }

    case "blockquote": {
      // 检测 GitHub 风格的 callout: > [!NOTE], > [!TIP], > [!WARNING] 等
      const firstChild = node.children[0];
      if (
        firstChild?.type === "paragraph" &&
        firstChild.children[0]?.type === "text"
      ) {
        const text = (firstChild.children[0] as any).value as string;
        // 只匹配 [!TYPE] 及其后面同一行的可选标题（不包含换行后的内容）
        const calloutMatch = text.match(
          /^\[!(NOTE|TIP|WARNING|CAUTION|IMPORTANT)\](?:[ \t]+([^\n]*))?/i
        );
        if (calloutMatch) {
          const typeMap: Record<string, ContainerType> = {
            NOTE: "info",
            TIP: "tip",
            WARNING: "warning",
            CAUTION: "danger",
            IMPORTANT: "details",
          };
          const containerType =
            typeMap[calloutMatch[1].toUpperCase()] || "info";
          // 标题只取 [!TYPE] 同一行的内容（如果有）
          const title = calloutMatch[2]?.trim() || "";

          // 处理剩余内容：移除 callout 标记，保留换行后的内容
          const afterCallout = text.substring(calloutMatch[0].length);
          const remainingText = afterCallout.replace(/^\n/, ""); // 移除开头换行

          // 构建新的 children
          const newChildren: RootContent[] = [];

          if (remainingText) {
            // 如果有剩余文本，创建一个新段落
            newChildren.push({
              type: "paragraph",
              children: [{ type: "text", value: remainingText }],
            } as any);
          }

          // 添加原 blockquote 的其他子节点
          for (let i = 1; i < node.children.length; i++) {
            newChildren.push(node.children[i]);
          }

          // 构建 content：标题（自定义或默认）+ 内容
          const contentItems: InlineContent[] = [];
          // 总是添加标题（自定义或默认）
          const titleText =
            title || containerDefaultTitles[containerType] || "提示";
          contentItems.push({ type: "text", text: titleText } as InlineContent);
          // 添加换行符，确保在编辑器中标题与内容分行
          contentItems.push({ type: "text", text: "\n" } as InlineContent);

          // 转换剩余内容（多个段落用单换行分隔）
          newChildren.forEach((child: RootContent, index: number) => {
            if (child.type === "paragraph" && "children" in child) {
              // 添加段落间的换行（第一个段落前不加）
              if (index > 0) {
                contentItems.push({
                  type: "text",
                  text: "\n",
                } as InlineContent);
              }
              const paragraphContent = convertInlineContent(
                child.children as PhrasingContent[]
              );
              contentItems.push(...paragraphContent);
            }
          });

          return {
            id: generateBlockId(),
            type: "container",
            props: {
              containerType,
            },
            content: contentItems,
          } as ContainerBlock;
        }
      }

      return {
        id: generateBlockId(),
        type: "quote",
        children: convertNodes(node.children),
      } as QuoteBlock;
    }

    // VitePress 容器指令: ::: info, ::: tip, ::: warning, ::: danger, ::: details, ::: code-group
    case "containerDirective": {
      const directiveName = (node as any).name as string;
      const lowerName = directiveName.toLowerCase();

      // 特殊处理: ::: code-group
      if (lowerName === "code-group") {
        const children = (node as any).children || [];
        const tabs: any[] = [];

        for (const child of children) {
          if (child.type === "code") {
            const meta = child.meta || "";
            const filenameMatch = meta.match(/\[(.*?)\]/);
            const filename = filenameMatch ? filenameMatch[1] : "";

            const highlightMatch = meta.match(/\{(.*?)\}/);
            const highlightLines = highlightMatch
              ? `{${highlightMatch[1]}}`
              : "";

            const lineNumbersMatch = meta.match(/:line-numbers(?:=(\d+))?/);
            const showLineNumbers = !!lineNumbersMatch;
            const startLineNumber =
              lineNumbersMatch && lineNumbersMatch[1]
                ? parseInt(lineNumbersMatch[1], 10)
                : 1;

            tabs.push({
              code: child.value || "",
              language: child.lang || "text",
              filename,
              highlightLines,
              showLineNumbers,
              startLineNumber,
            });
          }
        }

        if (tabs.length > 0) {
          const activeTab = tabs[0];
          return createShikiCodeBlock(activeTab.code, activeTab.language, {
            filename: activeTab.filename,
            highlightLines: activeTab.highlightLines,
            showLineNumbers: activeTab.showLineNumbers,
            startLineNumber: activeTab.startLineNumber,
            tabs: JSON.stringify(tabs),
            activeTabIndex: 0,
          });
        }
      }

      // 普通容器: info, tip, warning, danger, details
      const typeMap: Record<string, ContainerType> = {
        info: "info",
        tip: "tip",
        warning: "warning",
        danger: "danger",
        details: "details",
      };
      // 默认为 info，code-group 已经处理过，这里不用担心覆盖
      const containerType = typeMap[lowerName] || "info";

      // 提取标题（指令属性或第一行文本）
      let title = "";
      const attrs = (node as any).attributes || {};
      if (attrs.title) {
        title = attrs.title;
      }

      // 构建 content：标题（自定义或默认）+ 内容
      const contentItems: InlineContent[] = [];
      // 总是添加标题（自定义或默认）
      const titleText =
        title || containerDefaultTitles[containerType] || "提示";
      contentItems.push({ type: "text", text: titleText } as InlineContent);
      // 添加换行符
      contentItems.push({ type: "text", text: "\n" } as InlineContent);

      // 转换子节点内容（多个段落用单换行分隔，对应编辑器中的 \n）
      const children = (node as any).children || [];
      children.forEach((child: any, index: number) => {
        if (child.type === "paragraph" && "children" in child) {
          // 添加段落间的换行（第一个段落前不加）
          if (index > 0) {
            contentItems.push({ type: "text", text: "\n" } as InlineContent);
          }
          const paragraphContent = convertInlineContent(
            child.children as PhrasingContent[]
          );
          contentItems.push(...paragraphContent);
        }
      });

      return {
        id: generateBlockId(),
        type: "container",
        props: {
          containerType,
        },
        content: contentItems,
      } as ContainerBlock;
    }

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

      // VitePress Shiki 代码块解析
      const meta = node.meta || "";
      const filenameMatch = meta.match(/\[(.*?)\]/);
      const filename = filenameMatch ? filenameMatch[1] : undefined;

      const highlightMatch = meta.match(/\{(.*?)\}/);
      const highlightLines = highlightMatch
        ? `{${highlightMatch[1]}}`
        : undefined;

      const lineNumbersMatch = meta.match(/:line-numbers(?:=(\d+))?/);
      let showLineNumbers = false;
      let startLineNumber = 1;

      if (lineNumbersMatch) {
        showLineNumbers = true;
        if (lineNumbersMatch[1]) {
          startLineNumber = parseInt(lineNumbersMatch[1], 10);
        }
      }

      return createShikiCodeBlock(node.value, language, {
        filename,
        highlightLines,
        showLineNumbers,
        startLineNumber,
      });
    }

    // 块级数学公式 ($$...$$)
    case "math": {
      return {
        id: generateBlockId(),
        type: "math",
        props: {
          formula: (node as any).value || "",
        },
      } as MathBlock;
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

  // 调试：输出完整 MDAST 树
  console.log("[Parser] Full MDAST tree:", JSON.stringify(tree, null, 2));

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
