/**
 * blocknote-adapter.ts 单元测试
 *
 * 覆盖：Table 双向转换、Quote groupId 逻辑、InlineContent 样式映射
 */

import { describe, it, expect } from "vitest";
import {
  internalToBlockNote,
  blockNoteToInternal,
  loadMarkdownToEditor,
  saveEditorToMarkdown,
} from "../blocknote-adapter";
import type { HeadingBlock, ParagraphBlock } from "@/types/block";
import { generateBlockId } from "@/types/block";

describe("internalToBlockNote", () => {
  describe("基础转换", () => {
    it("应正确转换段落", () => {
      const internal = [
        {
          id: generateBlockId(),
          type: "paragraph" as const,
          content: [{ type: "text" as const, text: "Hello" }],
        },
      ];
      const bn = internalToBlockNote(internal as any);
      expect(bn).toHaveLength(1);
      expect(bn[0].type).toBe("paragraph");
    });

    it("应正确转换标题", () => {
      const internal = [
        {
          id: generateBlockId(),
          type: "heading" as const,
          props: { level: 2 as const },
          content: [{ type: "text" as const, text: "Title" }],
        },
      ];
      const bn = internalToBlockNote(internal as any);
      expect(bn[0].type).toBe("heading");
      expect(bn[0].props.level).toBe(2);
    });
  });

  describe("InlineContent 样式映射", () => {
    it("应正确映射粗体样式 (strong)", () => {
      const internal = [
        {
          id: generateBlockId(),
          type: "paragraph" as const,
          content: [
            {
              type: "strong" as const,
              children: [{ type: "text" as const, text: "Bold" }],
            },
          ],
        },
      ];
      const bn = internalToBlockNote(internal as any);
      const content = bn[0].content as any[];
      expect(content[0].styles?.bold).toBe(true);
    });

    it("应正确映射斜体样式 (emphasis)", () => {
      const internal = [
        {
          id: generateBlockId(),
          type: "paragraph" as const,
          content: [
            {
              type: "emphasis" as const,
              children: [{ type: "text" as const, text: "Italic" }],
            },
          ],
        },
      ];
      const bn = internalToBlockNote(internal as any);
      const content = bn[0].content as any[];
      expect(content[0].styles?.italic).toBe(true);
    });

    it("应正确映射链接", () => {
      const internal = [
        {
          id: generateBlockId(),
          type: "paragraph" as const,
          content: [
            {
              type: "link" as const,
              href: "https://example.com",
              children: [{ type: "text" as const, text: "Link" }],
            },
          ],
        },
      ];
      const bn = internalToBlockNote(internal as any);
      const content = bn[0].content as any[];
      expect(content[0].type).toBe("link");
      expect(content[0].href).toBe("https://example.com");
    });

    it("应正确映射行内公式", () => {
      const internal = [
        {
          id: generateBlockId(),
          type: "paragraph" as const,
          content: [{ type: "inlineMath" as const, formula: "E=mc^2" }],
        },
      ];
      const bn = internalToBlockNote(internal as any);
      const content = bn[0].content as any[];
      expect(content[0].type).toBe("inlineMath");
      expect(content[0].props.formula).toBe("E=mc^2");
    });
  });

  describe("Container 转换", () => {
    it("应正确转换 Container 块", () => {
      const internal = [
        {
          id: generateBlockId(),
          type: "container" as const,
          props: { containerType: "tip" as const },
          content: [{ type: "text" as const, text: "Tip content" }],
        },
      ];
      const bn = internalToBlockNote(internal as any);
      expect(bn[0].type).toBe("container");
      expect(bn[0].props.containerType).toBe("tip");
    });
  });

  describe("Math 块转换", () => {
    it("应正确转换 Math 块", () => {
      const internal = [
        {
          id: generateBlockId(),
          type: "math" as const,
          props: { formula: "x^2", display: "block" as const },
        },
      ];
      const bn = internalToBlockNote(internal as any);
      expect(bn[0].type).toBe("math");
      expect(bn[0].props.formula).toBe("x^2");
    });
  });

  describe("ShikiCode 块转换", () => {
    it("应正确转换 ShikiCode 块", () => {
      const internal = [
        {
          id: generateBlockId(),
          type: "shikiCode" as const,
          props: {
            code: "console.log('hello')",
            language: "js",
            filename: "test.js",
          },
        },
      ];
      const bn = internalToBlockNote(internal as any);
      expect(bn[0].type).toBe("shikiCode");
      expect(bn[0].props.code).toBe("console.log('hello')");
      expect(bn[0].props.language).toBe("js");
    });
  });

  describe("Table 转换", () => {
    it("应正确转换表格结构", () => {
      const internal = [
        {
          id: generateBlockId(),
          type: "table" as const,
          props: {
            headerRow: {
              cells: [
                { content: [{ type: "text" as const, text: "A" }] },
                { content: [{ type: "text" as const, text: "B" }] },
              ],
            },
            rows: [
              {
                cells: [
                  { content: [{ type: "text" as const, text: "1" }] },
                  { content: [{ type: "text" as const, text: "2" }] },
                ],
              },
            ],
          },
        },
      ];
      const bn = internalToBlockNote(internal as any);
      expect(bn[0].type).toBe("table");
    });
  });

  describe("Quote 转换", () => {
    it("应正确转换引用块", () => {
      const internal = [
        {
          id: generateBlockId(),
          type: "quote" as const,
          children: [
            {
              id: generateBlockId(),
              type: "paragraph" as const,
              content: [{ type: "text" as const, text: "Quoted" }],
            },
          ],
        },
      ];
      const bn = internalToBlockNote(internal as any);
      // 引用块应被正确转换
      const quotes = bn.filter((b) => b.type === "quote");
      expect(quotes.length).toBeGreaterThan(0);
    });
  });
});

describe("blockNoteToInternal", () => {
  describe("基础转换", () => {
    it("应正确转换段落", () => {
      const bn = [
        {
          id: "test-id",
          type: "paragraph",
          props: {},
          content: [{ type: "text", text: "Hello", styles: {} }],
          children: [],
        },
      ];
      const internal = blockNoteToInternal(bn);
      expect(internal).toHaveLength(1);
      expect(internal[0].type).toBe("paragraph");
    });

    it("应正确转换标题", () => {
      const bn = [
        {
          id: "test-id",
          type: "heading",
          props: { level: 3 },
          content: [{ type: "text", text: "H3", styles: {} }],
          children: [],
        },
      ];
      const internal = blockNoteToInternal(bn);
      expect(internal[0].type).toBe("heading");
      expect((internal[0] as any).props.level).toBe(3);
    });
  });

  describe("样式还原", () => {
    it("应正确还原粗体为 strong", () => {
      const bn = [
        {
          id: "test-id",
          type: "paragraph",
          props: {},
          content: [{ type: "text", text: "Bold", styles: { bold: true } }],
          children: [],
        },
      ];
      const internal = blockNoteToInternal(bn);
      const content = (internal[0] as any).content;
      expect(content[0].type).toBe("strong");
    });

    it("应正确还原斜体为 emphasis", () => {
      const bn = [
        {
          id: "test-id",
          type: "paragraph",
          props: {},
          content: [{ type: "text", text: "Italic", styles: { italic: true } }],
          children: [],
        },
      ];
      const internal = blockNoteToInternal(bn);
      const content = (internal[0] as any).content;
      expect(content[0].type).toBe("emphasis");
    });
  });

  describe("Quote groupId 合并", () => {
    it("应合并相邻同 groupId 的引用块", () => {
      const groupId = "test-group-123";
      const bn = [
        {
          id: "q1",
          type: "quote",
          props: { groupId, isFirstInGroup: true },
          content: [{ type: "text", text: "Line 1", styles: {} }],
          children: [],
        },
        {
          id: "q2",
          type: "quote",
          props: { groupId, isFirstInGroup: false },
          content: [{ type: "text", text: "Line 2", styles: {} }],
          children: [],
        },
      ];
      const internal = blockNoteToInternal(bn);
      // 同组引用应被合并为单个 quote block
      const quotes = internal.filter((b) => b.type === "quote");
      expect(quotes.length).toBe(1);
    });

    it("不同 groupId 的引用块应保持独立", () => {
      const bn = [
        {
          id: "q1",
          type: "quote",
          props: { groupId: "group-a", isFirstInGroup: true },
          content: [{ type: "text", text: "Quote A", styles: {} }],
          children: [],
        },
        {
          id: "q2",
          type: "quote",
          props: { groupId: "group-b", isFirstInGroup: true },
          content: [{ type: "text", text: "Quote B", styles: {} }],
          children: [],
        },
      ];
      const internal = blockNoteToInternal(bn);
      const quotes = internal.filter((b) => b.type === "quote");
      expect(quotes.length).toBe(2);
    });
  });

  describe("Table 转换", () => {
    it("应正确还原表格结构", () => {
      const bn = [
        {
          id: "table-1",
          type: "table",
          props: {},
          content: {
            type: "tableContent",
            rows: [
              {
                cells: [
                  {
                    type: "tableCell",
                    content: [{ type: "text", text: "Header", styles: {} }],
                    props: {},
                  },
                ],
              },
              {
                cells: [
                  {
                    type: "tableCell",
                    content: [{ type: "text", text: "Cell", styles: {} }],
                    props: {},
                  },
                ],
              },
            ],
          },
          children: [],
        },
      ];
      const internal = blockNoteToInternal(bn);
      expect(internal[0].type).toBe("table");
      expect((internal[0] as any).props.headerRow).toBeDefined();
      expect((internal[0] as any).props.rows).toBeDefined();
    });
  });

  describe("ShikiCode 转换", () => {
    it("应正确还原 ShikiCode 块", () => {
      const bn = [
        {
          id: "code-1",
          type: "shikiCode",
          props: {
            code: "let x = 1;",
            language: "ts",
            filename: "index.ts",
            highlightLines: "{1}",
            showLineNumbers: true,
          },
          content: undefined,
          children: [],
        },
      ];
      const internal = blockNoteToInternal(bn);
      expect(internal[0].type).toBe("shikiCode");
      expect((internal[0] as any).props.code).toBe("let x = 1;");
      expect((internal[0] as any).props.filename).toBe("index.ts");
    });
  });
});

describe("loadMarkdownToEditor", () => {
  it("应完成 Markdown -> BlockNote 全链路转换", async () => {
    const md = "# Hello\n\nThis is a paragraph.";
    const blocks = await loadMarkdownToEditor(md);
    expect(blocks.length).toBeGreaterThan(0);
    expect(blocks[0].type).toBe("heading");
  });

  it("应正确处理容器块", async () => {
    const md = `::: tip
This is a tip
:::`;
    const blocks = await loadMarkdownToEditor(md);
    const container = blocks.find((b) => b.type === "container");
    expect(container).toBeDefined();
  });
});

describe("saveEditorToMarkdown", () => {
  it("应完成 BlockNote -> Markdown 全链路转换", async () => {
    const blocks = [
      {
        id: "p1",
        type: "paragraph",
        props: {},
        content: [{ type: "text", text: "Hello", styles: {} }],
        children: [],
      },
    ];
    const md = await saveEditorToMarkdown(blocks);
    expect(md).toContain("Hello");
  });

  it("应正确包含 Frontmatter", async () => {
    const blocks = [
      {
        id: "p1",
        type: "paragraph",
        props: {},
        content: [{ type: "text", text: "Content", styles: {} }],
        children: [],
      },
    ];
    const md = await saveEditorToMarkdown(blocks, { title: "Test" });
    expect(md).toContain("---");
    expect(md).toContain("title: Test");
  });
});
