/**
 * BlockNote 适配器
 *
 * 负责 Internal Block Model (E0) 与 BlockNote Block 格式之间的双向转换
 */

import type { Block as InternalBlock, InlineContent } from "@/types/block";

// BlockNote Block 类型（简化版，避免直接依赖 @blocknote/core 的复杂泛型）
interface BlockNoteBlock {
  id?: string;
  type: string;
  props: Record<string, unknown>;
  content: unknown; // 可能是 InlineContent[] 或 TableContent 对象
  children?: BlockNoteBlock[];
}

/**
 * 将 Internal InlineContent 转换为 BlockNote 的 content 格式
 */
function internalInlineToBlockNote(inlineContent: InlineContent[]): unknown[] {
  return inlineContent.map((item) => {
    switch (item.type) {
      case "text":
        return { type: "text", text: item.text, styles: {} };
      case "emphasis":
        return {
          type: "text",
          text: item.children
            ?.map((c: InlineContent) => ("text" in c ? c.text : ""))
            .join(""),
          styles: { italic: true },
        };
      case "strong":
        return {
          type: "text",
          text: item.children
            ?.map((c: InlineContent) => ("text" in c ? c.text : ""))
            .join(""),
          styles: { bold: true },
        };
      case "code":
        return { type: "text", text: item.text, styles: { code: true } };
      case "link":
        return {
          type: "link",
          href: item.href,
          content: item.children?.map((c: InlineContent) => ({
            type: "text",
            text: "text" in c ? c.text : "",
            styles: {},
          })),
        };
      case "inlineMath":
        const formula = "formula" in item ? item.formula : "";
        if (item.displayMode) {
          return {
            type: "text",
            text: `$$${formula}$$`,
            styles: {},
          };
        }
        return {
          type: "text",
          text: `$${formula}$`,
          styles: {},
        };
      default:
        return { type: "text", text: "", styles: {} };
    }
  });
}

/**
 * 将 BlockNote content 转换回 Internal InlineContent
 */
function blockNoteInlineToInternal(content: unknown[]): InlineContent[] {
  return content.map((item: any) => {
    if (item.type === "text") {
      // 检查样式
      if (item.styles?.bold) {
        return {
          type: "strong" as const,
          children: [{ type: "text" as const, text: item.text }],
        };
      }
      if (item.styles?.italic) {
        return {
          type: "emphasis" as const,
          children: [{ type: "text" as const, text: item.text }],
        };
      }
      if (item.styles?.code) {
        return { type: "code" as const, text: item.text };
      }
      return { type: "text" as const, text: item.text };
    }
    if (item.type === "link") {
      return {
        type: "link" as const,
        href: item.href,
        children: item.content?.map((c: any) => ({
          type: "text" as const,
          text: c.text,
        })),
      };
    }
    return { type: "text" as const, text: "" };
  });
}

/**
 * Internal Block Model -> BlockNote Block
 *
 * 将 E0 阶段产生的 Block 模型转换为 BlockNote 可渲染的格式
 */
export function internalToBlockNote(blocks: InternalBlock[]): BlockNoteBlock[] {
  console.log(
    "[Adapter] Converting internal blocks:",
    JSON.stringify(blocks, null, 2)
  );
  return blocks.map((block) => {
    // Reverting to keep IDs for now as they are required by the type definition I wrote above
    const common = {};
    // const common = { id: block.id };

    switch (block.type) {
      case "paragraph":
        return {
          ...common,
          type: "paragraph",
          props: {},
          content: internalInlineToBlockNote(block.content || []),
          children: undefined,
        };

      case "heading":
        return {
          ...common,
          type: "heading",
          props: { level: block.props.level },
          content: internalInlineToBlockNote(block.content || []),
          children: undefined,
        };

      case "bulletListItem":
        return {
          ...common,
          type: "bulletListItem",
          props: {},
          content: internalInlineToBlockNote(block.content || []),
          children:
            block.children && block.children.length > 0
              ? internalToBlockNote(block.children)
              : undefined,
        };

      case "numberedListItem":
        return {
          ...common,
          type: "numberedListItem",
          props: { start: block.props.start || 1 },
          content: internalInlineToBlockNote(block.content || []),
          children:
            block.children && block.children.length > 0
              ? internalToBlockNote(block.children)
              : undefined,
        };

      case "checkListItem":
        return {
          ...common,
          type: "checkListItem",
          props: { checked: block.props.checked || false },
          content: internalInlineToBlockNote(block.content || []),
          children:
            block.children && block.children.length > 0
              ? internalToBlockNote(block.children)
              : undefined,
        };

      case "codeBlock":
        return {
          ...common,
          type: "codeBlock",
          props: { language: block.props.language || "text" },
          content: [{ type: "text", text: block.props.code, styles: {} }],
          children: undefined,
        };

      case "image":
        return {
          ...common,
          type: "image",
          props: {
            url: block.props.src,
            caption: block.props.alt || "",
            previewWidth: 512,
          },
          content: [],
          children: undefined,
        };

      case "table": {
        // 将 Internal TableBlock 转换为 BlockNote table 格式
        const tableBlock = block as any;
        const { headerRow, rows } = tableBlock.props || {};

        // BlockNote table 格式: content.rows[].cells 是 InlineContent[][] (二维数组)
        const tableRows: any[] = [];

        // 添加表头行
        if (headerRow && headerRow.cells) {
          tableRows.push({
            cells: headerRow.cells.map((cell: any) => ({
              type: "tableCell",
              content: internalInlineToBlockNote(cell.content || []),
              props: {},
            })),
          });
        }

        // 添加数据行
        if (rows && Array.isArray(rows)) {
          for (const row of rows) {
            tableRows.push({
              cells: row.cells.map((cell: any) => ({
                type: "tableCell",
                content: internalInlineToBlockNote(cell.content || []),
                props: {},
              })),
            });
          }
        }

        return {
          ...common,
          type: "table",
          props: {},
          content: {
            type: "tableContent",
            rows: tableRows,
          },
          children: undefined,
        };
      }

      case "quote":
        // Fallback: Map quote to a paragraph with "> " prefix
        // Since we don't have a native blockquote block active in schema yet
        return {
          ...common,
          type: "paragraph",
          props: {},
          content: [
            { type: "text", text: "> ", styles: {} },
            ...internalInlineToBlockNote(
              block.children
                ? block.children.flatMap((c) =>
                    c.type === "paragraph" ? c.content : []
                  )
                : []
            ),
          ],
          children: undefined,
        };

      // VitePress 扩展块 - 暂时转为代码块显示
      case "container":
      case "math":
      case "mermaid":
      case "vueComponent":
      case "include":
        return {
          ...common,
          type: "codeBlock",
          props: { language: "json" },
          content: [
            {
              type: "text",
              text: `/* ${block.type} block */\n${JSON.stringify(block, null, 2)}`,
              styles: {},
            },
          ],
          children: undefined,
        };

      default:
        // 后备：转为段落
        return {
          ...common,
          type: "paragraph",
          props: {},
          content: [
            {
              type: "text",
              text: `[未支持的块类型: ${block.type}]`,
              styles: {},
            },
          ],
          children: undefined,
        };
    }
  });
}

/**
 * BlockNote Block -> Internal Block Model
 *
 * 将 BlockNote 编辑器的输出转换回 E0 的 Block 模型
 */
export function blockNoteToInternal(blocks: BlockNoteBlock[]): InternalBlock[] {
  return blocks.map((block) => {
    const common = {
      id:
        block.id ||
        `block-${Date.now()}-${Math.random().toString(36).substr(2, 9)}`,
    };

    switch (block.type) {
      case "paragraph":
        return {
          ...common,
          type: "paragraph" as const,
          content: blockNoteInlineToInternal(block.content as unknown[]),
        };

      case "heading":
        return {
          ...common,
          type: "heading" as const,
          props: { level: (block.props.level as 1 | 2 | 3 | 4 | 5 | 6) || 1 },
          content: blockNoteInlineToInternal(block.content as unknown[]),
        };

      case "bulletListItem":
        return {
          ...common,
          type: "bulletListItem" as const,
          content: blockNoteInlineToInternal(block.content as unknown[]),
          children:
            block.children && block.children.length > 0
              ? blockNoteToInternal(block.children)
              : undefined,
        };

      case "numberedListItem":
        return {
          ...common,
          type: "numberedListItem" as const,
          props: { start: (block.props.start as number) || 1 },
          content: blockNoteInlineToInternal(block.content as unknown[]),
          children:
            block.children && block.children.length > 0
              ? blockNoteToInternal(block.children)
              : undefined,
        };

      case "checkListItem":
        return {
          ...common,
          type: "checkListItem" as const,
          props: { checked: (block.props.checked as boolean) || false },
          content: blockNoteInlineToInternal(block.content as unknown[]),
          children:
            block.children && block.children.length > 0
              ? blockNoteToInternal(block.children)
              : undefined,
        };

      case "codeBlock": {
        const language = (block.props.language as string) || "text";
        const code = ((block.content as any[])?.[0] as any)?.text || "";

        // E2.2: 尝试恢复自定义块（反序列化 JSON）
        if (language === "json") {
          // 匹配 /* type block */ 注释
          const match = code.match(/^\/\* (\w+) block \*\/\n([\s\S]*)/);
          if (match) {
            const [, type, jsonStr] = match;
            try {
              const customBlock = JSON.parse(jsonStr);
              // 验证类型是否匹配（简单的安全检查）
              if (customBlock.type === type) {
                return {
                  ...common,
                  ...customBlock,
                  // 确保 ID 使用新的或保持原样（如果 JSON 中有 ID，优先使用 JSON 中的，还是 create new? JSON 中的原 ID 是 block-timestamp，这里 block.id 也是 block-timestamp。
                  // 如果 BlockNote 生成了新 ID， block.id 变了。
                  // 实际上，我们应该信任 JSON 中的原始数据，除了 ID 可能需要更新为 block.id 以保持一致性，但 InternalBlock ID 主要是为了 React Key。
                  // 让我们使用 JSON 中的完整数据，覆盖 common.id?
                  // 不，保持 block.id 可能更好此时，或者...
                  // 其实直接返回 customBlock 即可，它就是 InternalBlock 结构
                };
              }
            } catch (e) {
              console.warn("[Adapter] Failed to parse custom block JSON:", e);
            }
          }
        }

        return {
          ...common,
          type: "codeBlock" as const,
          props: {
            language,
            code,
          },
        };
      }

      case "table": {
        // BlockNote table 格式转换为 Internal TableBlock
        const tableContent = block.content as any;
        const bnRows = tableContent?.rows || [];

        // 辅助函数：安全地转换 cell 内容
        const convertCell = (cell: any) => {
          // BlockNote cell 是对象: { type: "tableCell", content: InlineContent[], props: {...} }
          if (
            cell &&
            cell.type === "tableCell" &&
            Array.isArray(cell.content)
          ) {
            return { content: blockNoteInlineToInternal(cell.content) };
          }
          // 如果是直接的数组（兼容旧格式）
          if (Array.isArray(cell)) {
            return { content: blockNoteInlineToInternal(cell) };
          }
          // 如果不是预期格式，返回空内容
          return { content: [] };
        };

        // 第一行作为表头
        const headerRow =
          bnRows.length > 0
            ? {
                cells: bnRows[0].cells.map(convertCell),
              }
            : { cells: [] };

        // 剩余行作为数据行
        const dataRows = bnRows.slice(1).map((row: any) => ({
          cells: row.cells.map(convertCell),
        }));

        return {
          ...common,
          type: "table" as const,
          props: {
            headerRow,
            rows: dataRows,
          },
        };
      }

      case "image":
        return {
          ...common,
          type: "image" as const,
          props: {
            src: (block.props.url as string) || "",
            alt: (block.props.caption as string) || undefined,
          },
        };

      default:
        // 后备：转为段落
        return {
          ...common,
          type: "paragraph" as const,
          content: [{ type: "text" as const, text: "" }],
        };
    }
  }) as InternalBlock[];
}

/**
 * 完整加载流程：Markdown -> BlockNote Blocks
 *
 * @param markdown Markdown 字符串
 * @returns BlockNote 可渲染的 Block 数组
 */
export async function loadMarkdownToEditor(
  markdown: string
): Promise<BlockNoteBlock[]> {
  // 使用 E0 的转换器
  const { markdownToBlocks } = await import("@/utils/markdown-to-blocks");
  const internalBlocks = markdownToBlocks(markdown);
  return internalToBlockNote(internalBlocks);
}

/**
 * 完整保存流程：BlockNote Blocks -> Markdown
 *
 * @param blocks BlockNote Block 数组
 * @param frontmatter 可选的 Frontmatter 元数据
 * @returns Markdown 字符串
 */
export async function saveEditorToMarkdown(
  blocks: BlockNoteBlock[],
  frontmatter?: Record<string, any>
): Promise<string> {
  // 使用 E0 的转换器
  const { blocksToMarkdown, documentToMarkdown } = await import(
    "@/utils/blocks-to-markdown"
  );
  const internalBlocks = blockNoteToInternal(blocks);

  let markdown = "";

  if (frontmatter && Object.keys(frontmatter).length > 0) {
    markdown = documentToMarkdown({
      path: "",
      blocks: internalBlocks,
      frontmatter,
    } as any);
  } else {
    markdown = blocksToMarkdown(internalBlocks);
  }

  // Ensure trailing newline for Prettier compliance
  return markdown.endsWith("\n") ? markdown : markdown + "\n";
}
