/**
 * Block → Markdown 转换器
 *
 * 将 Block 类型序列化为 Markdown 文本。
 * 支持 VitePress 扩展语法输出。
 */

import {
  type Block,
  type InlineContent,
  type TextContent,
  type HeadingBlock,
  type ParagraphBlock,
  type BulletListItemBlock,
  type NumberedListItemBlock,
  type CheckListItemBlock,
  type CodeBlockBlock,
  type TableBlock,
  type ImageBlock,
  type QuoteBlock,
  type ContainerBlock,
  type MathBlock,
  type MermaidBlock,
  type VueComponentBlock,
  type IncludeBlock,
} from "../types/block";
import type { Document, Frontmatter } from "../types/document";

import yaml from "yaml";

// ============================================================================
// 行内内容序列化
// ============================================================================

/**
 * 将 InlineContent 或 InlineContent[] 转换为 Markdown 文本
 */
function inlineContentToMarkdown(
  content: InlineContent | InlineContent[]
): string {
  if (Array.isArray(content)) {
    return content.map((c) => inlineContentToMarkdown(c)).join("");
  }
  switch (content.type) {
    case "text":
      return content.text || "";

    case "emphasis":
      if (!content.children) return "";
      return `*${content.children.map(inlineContentToMarkdown).join("")}*`;

    case "strong":
      if (!content.children) return "";
      return `**${content.children.map(inlineContentToMarkdown).join("")}**`;

    case "code":
      return `\`${content.text || ""}\``;

    case "link":
      if (!content.children) return "";
      const linkText = content.children.map(inlineContentToMarkdown).join("");
      return `[${linkText}](${content.href || ""})`;

    case "inlineMath":
      if (content.displayMode) {
        return `$$${content.formula || ""}$$`;
      }
      return `$${content.formula || ""}$`;

    default:
      return content.text || "";
  }
}

/**
 * 将 InlineContent 数组转换为 Markdown 文本
 */
function inlineContentsToMarkdown(contents: InlineContent[]): string {
  return contents.map(inlineContentToMarkdown).join("");
}

/**
 * 预处理容器内容：将带有样式的换行符拆分出来。
 * 例如：**text\ntext** -> **text**\n**text**
 * 这可以防止序列化后的 **\n\n** 破坏 Markdown 结构。
 */
function preprocessContainerContent(
  contents: InlineContent[]
): InlineContent[] {
  const splitContent = (content: InlineContent): InlineContent[] => {
    // 文本节点：如果有换行符，直接拆分
    if (content.type === "text") {
      if (content.text && content.text.includes("\n")) {
        return content.text
          .split(/(\n)/)
          .filter((t) => t !== "")
          .map((t) => ({ type: "text", text: t }) as TextContent);
      }
      return [content];
    }

    // 容器节点 (strong, emphasis)：递归检查 children
    if ("children" in content && Array.isArray(content.children)) {
      // 链接不拆分
      if (content.type === "link") return [content];

      const newChildren = content.children.flatMap(splitContent);

      // 检查 children 是否有换行符
      const hasNewline = newChildren.some(
        (c) => c.type === "text" && (c as TextContent).text.includes("\n")
      );

      if (!hasNewline) {
        return [{ ...content, children: newChildren }];
      }

      // 如果有换行符，拆分当前节点
      const result: InlineContent[] = [];
      let currentChunk: InlineContent[] = [];

      for (const child of newChildren) {
        // 只要包含换行符就被视为分割点？
        // splitContent 应该已经把纯文本的 \n 拆出来了
        // 但如果 splitContent 没有正确拆分...

        if (child.type === "text" && child.text === "\n") {
          if (currentChunk.length > 0) {
            result.push({ ...content, children: currentChunk });
            currentChunk = [];
          }
          result.push(child); // 换行符提升到外层
        } else {
          currentChunk.push(child);
        }
      }
      if (currentChunk.length > 0) {
        result.push({ ...content, children: currentChunk });
      }
      return result;
    }

    return [content];
  };

  return contents.flatMap(splitContent);
}

// ============================================================================
// Block 序列化
// ============================================================================

/**
 * 将单个 Block 转换为 Markdown
 */
function blockToMarkdown(block: Block, indent: string = ""): string {
  switch (block.type) {
    case "heading": {
      const heading = block as HeadingBlock;
      const prefix = "#".repeat(heading.props.level);
      const text = inlineContentsToMarkdown(heading.content);
      return `${prefix} ${text}`;
    }

    case "paragraph": {
      const paragraph = block as ParagraphBlock;
      return inlineContentsToMarkdown(paragraph.content);
    }

    case "bulletListItem": {
      const item = block as BulletListItemBlock;
      const content = inlineContentsToMarkdown(item.content);
      let result = `${indent}- ${content}`;
      if (item.children && item.children.length > 0) {
        const childrenMd = item.children
          .map((child) => blockToMarkdown(child, indent + "  "))
          .join("\n");
        result += "\n" + childrenMd;
      }
      return result;
    }

    case "numberedListItem": {
      const item = block as NumberedListItemBlock;
      const content = inlineContentsToMarkdown(item.content);
      const num = item.props.start || 1;
      let result = `${indent}${num}. ${content}`;
      if (item.children && item.children.length > 0) {
        const childrenMd = item.children
          .map((child) => blockToMarkdown(child, indent + "   "))
          .join("\n");
        result += "\n" + childrenMd;
      }
      return result;
    }

    case "checkListItem": {
      const item = block as CheckListItemBlock;
      const content = inlineContentsToMarkdown(item.content);
      const checkbox = item.props.checked ? "[x]" : "[ ]";
      let result = `${indent}- ${checkbox} ${content}`;
      if (item.children && item.children.length > 0) {
        const childrenMd = item.children
          .map((child) => blockToMarkdown(child, indent + "  "))
          .join("\n");
        result += "\n" + childrenMd;
      }
      return result;
    }

    case "codeBlock": {
      const codeBlock = block as CodeBlockBlock;
      const lang = codeBlock.props.language || "";
      const meta = codeBlock.props.filename || "";
      const fence = "```";
      const header = meta ? `${lang} ${meta}` : lang;
      return `${fence}${header}\n${codeBlock.props.code}\n${fence}`;
    }

    case "table": {
      const table = block as TableBlock;
      const { headerRow, rows } = table.props;

      // 渲染表头
      const headerCells = headerRow.cells.map((cell) =>
        inlineContentsToMarkdown(cell.content)
      );
      const headerLine = `| ${headerCells.join(" | ")} |`;

      // 渲染分隔线
      const separatorCells = headerRow.cells.map((cell) => {
        const align = cell.align;
        if (align === "center") return ":---:";
        if (align === "right") return "---:";
        return "---";
      });
      const separatorLine = `| ${separatorCells.join(" | ")} |`;

      // 渲染数据行
      const dataLines = rows.map((row) => {
        const cells = row.cells.map((cell) =>
          inlineContentsToMarkdown(cell.content)
        );
        return `| ${cells.join(" | ")} |`;
      });

      return [headerLine, separatorLine, ...dataLines].join("\n");
    }

    case "image": {
      const image = block as ImageBlock;
      const alt = image.props.alt || "";
      const src = image.props.src;
      const title = image.props.title ? ` "${image.props.title}"` : "";
      return `![${alt}](${src}${title})`;
    }

    case "quote": {
      const quote = block as QuoteBlock;
      if (!quote.children) return "";
      const content = quote.children
        .map((child) => blockToMarkdown(child, ""))
        .join("\n\n");
      // 每行前加 >
      return content
        .split("\n")
        .map((line) => `> ${line}`)
        .join("\n");
    }

    case "thematicBreak":
      return "---";

    case "container": {
      const container = block as ContainerBlock;
      const { containerType } = container.props;

      // 智能提取标题：寻找第一个换行符
      const content = container.content || [];
      let title = "";
      let bodyContent: InlineContent[] = content;

      if (content.length > 0) {
        let fullText = "";
        let splitIndex = -1;
        let splitOffset = -1;

        // 寻找第一个换行符
        for (let i = 0; i < content.length; i++) {
          const item = content[i];
          if (item.type === "text") {
            const textItem = item as TextContent;
            const textIdx = textItem.text.indexOf("\n");
            if (textIdx !== -1) {
              splitIndex = i;
              splitOffset = textIdx;
              fullText += textItem.text.substring(0, textIdx);
              break;
            }
            fullText += textItem.text;
          }
        }

        if (splitIndex === -1) {
          // 没有换行符，全部作为标题
          title = content
            .map((c) => (c.type === "text" ? c.text : ""))
            .join("")
            .trim();
          bodyContent = [];
        } else {
          // 提取标题
          title = fullText.trim();

          // 提取 Body
          bodyContent = [];
          const splitNode = content[splitIndex];
          if (splitNode.type === "text") {
            const splitTextNode = splitNode as TextContent;
            // 移除标题部分和换行符，保留后续内容
            let afterText = splitTextNode.text.substring(splitOffset);
            afterText = afterText.replace(/^\n+/, "");

            if (afterText) {
              bodyContent.push({ ...splitNode, text: afterText });
            }
          }
          // 添加后续节点
          for (let i = splitIndex + 1; i < content.length; i++) {
            bodyContent.push(content[i]);
          }
        }

        // 默认标题列表
        const defaultTitles = [
          "提示",
          "警告",
          "危险",
          "详情",
          "信息",
          "注意",
          "TIP",
          "WARNING",
          "DANGER",
          "DETAILS",
          "INFO",
          "NOTE",
        ];

        // 如果是默认标题，title 置空
        if (defaultTitles.includes(title)) {
          title = "";
        }
      }

      const titlePart = title ? ` ${title}` : "";

      // 预处理内容，确保样式不跨越换行符
      const processedBodyContent = preprocessContainerContent(bodyContent);

      let bodyMarkdown = inlineContentToMarkdown(processedBodyContent);
      // 容器内容中的单换行在保存时转为双换行（VitePress 格式）
      bodyMarkdown = bodyMarkdown.replace(/\n/g, "\n\n");
      return `:::${containerType}${titlePart}\n${bodyMarkdown}\n:::`;
    }

    case "math": {
      const math = block as MathBlock;
      if (math.props.display === "inline") {
        return `$${math.props.formula}$`;
      }
      return `$$\n${math.props.formula}\n$$`;
    }

    case "mermaid": {
      const mermaid = block as MermaidBlock;
      return `\`\`\`mermaid\n${mermaid.props.code}\n\`\`\``;
    }

    case "vueComponent": {
      const vue = block as VueComponentBlock;
      const { componentName, attributes, selfClosing } = vue.props;

      // 构建属性字符串
      const attrParts: string[] = [];
      for (const [key, value] of Object.entries(attributes)) {
        if (value === true) {
          attrParts.push(key);
        } else if (value === false) {
          attrParts.push(`:${key}="false"`);
        } else if (typeof value === "number") {
          attrParts.push(`:${key}="${value}"`);
        } else {
          attrParts.push(`${key}="${value}"`);
        }
      }
      const attrString = attrParts.length > 0 ? ` ${attrParts.join(" ")}` : "";

      if (selfClosing) {
        return `<${componentName}${attrString} />`;
      }

      // 非自闭合标签，渲染子内容
      const content = vue.children
        ? vue.children.map((child) => blockToMarkdown(child, "")).join("\n\n")
        : "";
      return `<${componentName}${attrString}>\n${content}\n</${componentName}>`;
    }

    case "include": {
      const include = block as IncludeBlock;
      const { path, lineRange, region } = include.props;

      let result = `<!--@include: ${path}`;
      if (region) {
        result += `#${region}`;
      }
      if (lineRange) {
        const start = lineRange.start ?? "";
        const end = lineRange.end ?? "";
        result += `{${start}-${end}}`;
      }
      result += "-->";
      return result;
    }

    default:
      console.warn(`Unknown block type: ${(block as Block).type}`);
      return "";
  }
}

// ============================================================================
// Frontmatter 序列化
// ============================================================================

/**
 * 将 Frontmatter 转换为 YAML 字符串（包含 --- 分隔符）
 */
function frontmatterToMarkdown(frontmatter: Frontmatter): string {
  // 过滤掉 undefined 和空值
  const cleaned: Record<string, unknown> = {};
  for (const [key, value] of Object.entries(frontmatter)) {
    if (value !== undefined && value !== null && value !== "") {
      cleaned[key] = value;
    }
  }

  if (Object.keys(cleaned).length === 0) {
    return "";
  }

  const yamlStr = yaml.stringify(cleaned, { lineWidth: 0 }).trim();
  return `---\n${yamlStr}\n---`;
}

// ============================================================================
// 公共 API
// ============================================================================

/**
 * 将 Block 数组转换为 Markdown 文本
 */
export function blocksToMarkdown(blocks: Block[]): string {
  const parts: string[] = [];

  for (let i = 0; i < blocks.length; i++) {
    const block = blocks[i];
    const md = blockToMarkdown(block, "");

    if (md) {
      parts.push(md);
    }
  }

  return parts.join("\n\n");
}

/**
 * 将 Document 转换为完整的 Markdown 文本（包含 Frontmatter）
 */
export function documentToMarkdown(document: Document): string {
  const parts: string[] = [];

  // 添加 Frontmatter
  const frontmatterMd = frontmatterToMarkdown(document.frontmatter);
  if (frontmatterMd) {
    parts.push(frontmatterMd);
  }

  // 添加内容
  const contentMd = blocksToMarkdown(document.blocks);
  if (contentMd) {
    parts.push(contentMd);
  }

  return parts.join("\n\n");
}

/**
 * 将单个 Block 转换为 Markdown（便捷方法）
 */
export function singleBlockToMarkdown(block: Block): string {
  return blockToMarkdown(block, "");
}
