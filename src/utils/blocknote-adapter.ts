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
      case "inlineMath": {
        const formula = "formula" in item ? (item.formula as string) : "";
        // 使用 BlockNote 自定义内联内容类型
        return {
          type: "inlineMath",
          props: { formula },
        };
      }
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
    // E2.3: 自定义内联内容 - inlineMath
    if (item.type === "inlineMath") {
      return {
        type: "inlineMath" as const,
        formula: (item.props?.formula as string) || "",
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
  // 使用 flatMap 允许单个内部块返回多个 BlockNote 块（如 quote 的多行）
  return blocks.flatMap((block) => {
    // Reverting to keep IDs for now as they are required by the type definition I wrote above
    const common = {};
    // const common = { id: block.id };

    switch (block.type) {
      case "paragraph":
        return [
          {
            ...common,
            type: "paragraph",
            props: {},
            content: internalInlineToBlockNote(block.content || []),
            children: undefined,
          },
        ];

      case "heading":
        return [
          {
            ...common,
            type: "heading",
            props: { level: block.props.level },
            content: internalInlineToBlockNote(block.content || []),
            children: undefined,
          },
        ];

      case "bulletListItem":
        return [
          {
            ...common,
            type: "bulletListItem",
            props: {},
            content: internalInlineToBlockNote(block.content || []),
            children:
              block.children && block.children.length > 0
                ? internalToBlockNote(block.children)
                : undefined,
          },
        ];

      case "numberedListItem":
        return [
          {
            ...common,
            type: "numberedListItem",
            props: { start: block.props.start || 1 },
            content: internalInlineToBlockNote(block.content || []),
            children:
              block.children && block.children.length > 0
                ? internalToBlockNote(block.children)
                : undefined,
          },
        ];

      case "checkListItem":
        return [
          {
            ...common,
            type: "checkListItem",
            props: { checked: block.props.checked || false },
            content: internalInlineToBlockNote(block.content || []),
            children:
              block.children && block.children.length > 0
                ? internalToBlockNote(block.children)
                : undefined,
          },
        ];

      case "codeBlock":
        return [
          {
            ...common,
            type: "codeBlock",
            props: { language: block.props.language || "text" },
            content: [{ type: "text", text: block.props.code, styles: {} }],
            children: undefined,
          },
        ];

      case "image":
        return [
          {
            ...common,
            type: "image",
            props: {
              url: block.props.src,
              caption: block.props.alt || "",
              previewWidth: 512,
            },
            content: [],
            children: undefined,
          },
        ];

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

        return [
          {
            ...common,
            type: "table",
            props: {},
            content: {
              type: "tableContent",
              rows: tableRows,
            },
            children: undefined,
          },
        ];
      }

      case "quote": {
        // E2.4: List-item model - 每个段落作为独立的顶级 quote 块
        // 使用 flatMap 返回数组，让同级段落成为同级 BlockNote 块
        const quoteBlocks: any[] = [];

        // 生成唯一的 groupId 标识这一组引用块
        const groupId = `group-${Date.now()}-${Math.random().toString(36).substr(2, 9)}`;

        if (block.children) {
          block.children.forEach((child) => {
            if (child.type === "paragraph" && child.content) {
              // 每个段落创建一个独立的顶级 quote 块
              // 第一个块标记为组内首位，后续为同组兄弟
              quoteBlocks.push({
                id:
                  (child as any).id ||
                  `block-${Date.now()}-${Math.random().toString(36).substr(2, 9)}`,
                type: "quote",
                props: { groupId, isFirstInGroup: quoteBlocks.length === 0 },
                content: internalInlineToBlockNote(child.content || []),
                children: [],
              });
            } else if (child.type === "quote") {
              // 嵌套 quote: 作为最后一个 quote 块的 children
              const converted = internalToBlockNote([child] as any);
              if (converted.length > 0 && quoteBlocks.length > 0) {
                const lastQuote = quoteBlocks[quoteBlocks.length - 1];
                lastQuote.children.push(...converted);
              } else if (converted.length > 0) {
                // 如果没有前置段落，作为独立块
                quoteBlocks.push(...converted);
              }
            } else {
              // 其他类型递归转换作为独立块
              // 注意：非 quote 类型的子块不会继承当前 groupId
              const converted = internalToBlockNote([child] as any);
              quoteBlocks.push(...converted);
            }
          });
        }

        // 返回空 quote 块如果没有内容
        if (quoteBlocks.length === 0) {
          return [
            {
              ...common,
              type: "quote",
              props: { groupId },
              content: [],
              children: [],
            },
          ];
        }

        // 返回所有 quote 块作为同级块
        return quoteBlocks;
      }

      // E2.3: ContainerBlock - VitePress 容器块
      case "container": {
        const containerBlock = block as any;
        return [
          {
            ...common,
            type: "container",
            props: {
              containerType: containerBlock.props?.containerType || "tip",
            },
            // 内容直接使用 content，第一行可能是标题
            content: internalInlineToBlockNote(containerBlock.content || []),
            children: undefined,
          },
        ];
      }

      // E2.3: MathBlock - LaTeX 公式块
      case "math": {
        const mathBlock = block as any;
        return [
          {
            ...common,
            type: "math",
            props: {
              formula: mathBlock.props?.formula || "",
            },
            content: undefined,
            children: undefined,
          },
        ];
      }

      // E2.3: MermaidBlock - Mermaid 图表块
      case "mermaid": {
        const mermaidBlock = block as any;
        return [
          {
            ...common,
            type: "mermaid",
            props: {
              code: mermaidBlock.props?.code || "",
            },
            content: undefined,
            children: undefined,
          },
        ];
      }

      // E2.4: VueComponentBlock - Vue 组件块
      case "vueComponent": {
        const vueBlock = block as any;
        // 将 attributes 对象序列化为 JSON 字符串存储
        const attributesJson =
          typeof vueBlock.props?.attributes === "object"
            ? JSON.stringify(vueBlock.props.attributes)
            : vueBlock.props?.attributesJson || "{}";
        return [
          {
            ...common,
            type: "vueComponent",
            props: {
              componentName: vueBlock.props?.componentName || "",
              attributesJson,
              selfClosing: vueBlock.props?.selfClosing ?? true,
            },
            content: undefined,
            children: undefined,
          },
        ];
      }

      // E2.4: IncludeBlock - 文件包含块
      case "include": {
        const includeBlock = block as any;
        // 将 lineRange 对象转换为字符串格式
        let lineRangeStr = "";
        if (includeBlock.props?.lineRange) {
          if (typeof includeBlock.props.lineRange === "object") {
            const { start, end } = includeBlock.props.lineRange;
            if (start || end) {
              lineRangeStr = `${start || ""}-${end || ""}`;
            }
          } else {
            lineRangeStr = includeBlock.props.lineRange;
          }
        }
        return [
          {
            ...common,
            type: "include",
            props: {
              path: includeBlock.props?.path || "",
              lineRange: lineRangeStr,
              region: includeBlock.props?.region || "",
            },
            content: undefined,
            children: undefined,
          },
        ];
      }

      // E2.5: ShikiCodeBlock - 高级代码块
      case "shikiCode": {
        return [
          {
            ...common,
            type: "shikiCode",
            props: {
              code: block.props.code,
              language: block.props.language,
              filename: block.props.filename || "",
              highlightLines: block.props.highlightLines || "",
              showLineNumbers: block.props.showLineNumbers || false,
              startLineNumber: block.props.startLineNumber || 1,
              tabs: block.props.tabs || "[]",
              activeTabIndex: block.props.activeTabIndex || 0,
            },
            content: undefined,
            children: undefined,
          },
        ];
      }

      default:
        // 后备：转为段落
        return [
          {
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
          },
        ];
    }
  });
}

/**
 * BlockNote Block -> Internal Block Model
 *
 * 将 BlockNote 编辑器的输出转换回 E0 的 Block 模型
 */
export function blockNoteToInternal(blocks: BlockNoteBlock[]): InternalBlock[] {
  // 预处理：合并相邻的同 groupId quote 块
  // 使用 _mergedSequence 保持内容和嵌套引用的原始顺序
  const mergedBlocks: BlockNoteBlock[] = [];

  for (let i = 0; i < blocks.length; i++) {
    const block = blocks[i];

    if (block.type === "quote") {
      const currentGroupId = (block.props as any)?.groupId;

      // 检查是否可以与上一个块合并
      if (mergedBlocks.length > 0) {
        const lastMerged = mergedBlocks[mergedBlocks.length - 1];
        const lastGroupId = (lastMerged.props as any)?.groupId;

        if (
          lastMerged.type === "quote" &&
          currentGroupId &&
          currentGroupId === lastGroupId
        ) {
          // 合并：将当前块的内容和 children 按顺序追加
          const existingSequence = (lastMerged as any)._mergedSequence || [
            { type: "content", content: lastMerged.content },
            ...(lastMerged.children || []).map((c: any) => ({
              type: "child",
              child: c,
            })),
          ];

          // 追加当前块的内容
          const newItems: any[] = [{ type: "content", content: block.content }];
          // 追加当前块的 children（保持顺序）
          if (block.children && block.children.length > 0) {
            block.children.forEach((c: any) => {
              newItems.push({ type: "child", child: c });
            });
          }

          const mergedQuote = {
            ...lastMerged,
            _mergedSequence: [...existingSequence, ...newItems],
            // 清空 children，因为现在都在 sequence 里
            children: [],
          };
          mergedBlocks[mergedBlocks.length - 1] = mergedQuote as any;
          continue;
        }
      }
    }

    mergedBlocks.push(block);
  }

  return mergedBlocks.map((block) => {
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

      // E2.3: ContainerBlock 反向转换
      case "container": {
        return {
          ...common,
          type: "container" as const,
          props: {
            containerType: (block.props.containerType as string) || "tip",
          },
          // 直接使用 content，第一行可能是标题
          content: blockNoteInlineToInternal(block.content as unknown[]),
        };
      }

      // E2.3: MathBlock 反向转换
      case "math": {
        return {
          ...common,
          type: "math" as const,
          props: {
            formula: (block.props.formula as string) || "",
          },
        };
      }

      // E2.3: MermaidBlock 反向转换
      case "mermaid": {
        return {
          ...common,
          type: "mermaid" as const,
          props: {
            code: (block.props.code as string) || "",
          },
        };
      }

      // E2.4: VueComponentBlock 反向转换
      case "vueComponent": {
        // 将 attributesJson 解析回 attributes 对象
        let attributes: Record<string, string | number | boolean> = {};
        const jsonStr = block.props.attributesJson as string;
        if (jsonStr) {
          try {
            attributes = JSON.parse(jsonStr);
          } catch {
            attributes = {};
          }
        }
        return {
          ...common,
          type: "vueComponent" as const,
          props: {
            componentName: (block.props.componentName as string) || "",
            attributes,
            selfClosing: (block.props.selfClosing as boolean) ?? true,
          },
        };
      }

      // E2.4: IncludeBlock 反向转换
      case "include": {
        // 将 lineRange 字符串解析回对象
        const rangeStr = block.props.lineRange as string;
        let lineRange: { start?: number; end?: number } | undefined;
        if (rangeStr) {
          const match = rangeStr.match(/^(\d+)?-(\d+)?$/);
          if (match) {
            lineRange = {
              start: match[1] ? parseInt(match[1]) : undefined,
              end: match[2] ? parseInt(match[2]) : undefined,
            };
          }
        }
        return {
          ...common,
          type: "include" as const,
          props: {
            path: (block.props.path as string) || "",
            lineRange,
            region: (block.props.region as string) || undefined,
          },
        };
      }

      // E2.5: ShikiCodeBlock 反向转换
      case "shikiCode": {
        return {
          ...common,
          type: "shikiCode" as const,
          props: {
            code: (block.props.code as string) || "",
            language: (block.props.language as string) || "text",
            filename: (block.props.filename as string) || "",
            highlightLines: (block.props.highlightLines as string) || "",
            showLineNumbers: (block.props.showLineNumbers as boolean) || false,
            startLineNumber: (block.props.startLineNumber as number) || 1,
            tabs: (block.props.tabs as string) || "[]",
            activeTabIndex: (block.props.activeTabIndex as number) || 0,
          },
        };
      }

      // E2.4: QuoteBlock 反向转换 (List-item model)
      case "quote": {
        // 处理合并后的 quote 块（保持内容和嵌套引用的原始顺序）
        const children: any[] = [];
        let paragraphIndex = 0;

        // 检查是否有合并的序列（保持顺序）
        const mergedSequence = (block as any)._mergedSequence;

        if (mergedSequence && Array.isArray(mergedSequence)) {
          // 处理合并后的序列，保持原始顺序
          mergedSequence.forEach((item: any) => {
            if (item.type === "content") {
              const contentArray = item.content as unknown[];
              if (contentArray && contentArray.length > 0) {
                children.push({
                  id: `${common.id}-p${paragraphIndex++}`,
                  type: "paragraph" as const,
                  content: blockNoteInlineToInternal(contentArray),
                });
              }
            } else if (item.type === "child") {
              // 嵌套的 quote 块，递归转换
              const nestedInternal = blockNoteToInternal([item.child] as any);
              children.push(...nestedInternal);
            }
          });
        } else {
          // 单个 quote 块的 inline content 作为一个段落
          const rawContent = (block.content as unknown[]) || [];
          if (rawContent.length > 0) {
            children.push({
              id: `${common.id}-p${paragraphIndex++}`,
              type: "paragraph" as const,
              content: blockNoteInlineToInternal(rawContent),
            });
          }

          // 处理 BlockNote 的嵌套 children（嵌套 quote）
          if (block.children && block.children.length > 0) {
            const nestedInternal = blockNoteToInternal(block.children as any);
            children.push(...nestedInternal);
          }
        }

        // 确保至少有一个空段落
        if (children.length === 0) {
          children.push({
            id: `${common.id}-p0`,
            type: "paragraph" as const,
            content: [],
          });
        }

        return {
          ...common,
          type: "quote" as const,
          children,
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
