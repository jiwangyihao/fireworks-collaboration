/**
 * blocks-to-markdown.ts 单元测试
 *
 * 覆盖：Container 序列化、Quote groupId 合并、Diff 标记保留、Table 序列化
 */

import { describe, it, expect } from "vitest";
import {
  blocksToMarkdown,
  documentToMarkdown,
  singleBlockToMarkdown,
} from "../blocks-to-markdown";
import type { Document } from "@/types/document";
import { generateBlockId } from "@/types/block";

describe("blocksToMarkdown", () => {
  describe("基础序列化", () => {
    it("应正确序列化段落", () => {
      const blocks = [
        {
          id: generateBlockId(),
          type: "paragraph" as const,
          content: [{ type: "text" as const, text: "Hello World" }],
        },
      ];
      const md = blocksToMarkdown(blocks as any);
      expect(md).toContain("Hello World");
    });

    it("应正确序列化标题", () => {
      const blocks = [
        {
          id: generateBlockId(),
          type: "heading" as const,
          props: { level: 2 as const },
          content: [{ type: "text" as const, text: "Title" }],
        },
      ];
      const md = blocksToMarkdown(blocks as any);
      expect(md).toContain("## Title");
    });

    it("应正确序列化带样式的文本", () => {
      const blocks = [
        {
          id: generateBlockId(),
          type: "paragraph" as const,
          content: [
            {
              type: "strong" as const,
              children: [{ type: "text" as const, text: "bold" }],
            },
            { type: "text" as const, text: " and " },
            {
              type: "emphasis" as const,
              children: [{ type: "text" as const, text: "italic" }],
            },
          ],
        },
      ];
      const md = blocksToMarkdown(blocks as any);
      expect(md).toContain("**bold**");
      expect(md).toContain("*italic*");
    });
  });

  describe("Container 序列化", () => {
    it("应正确序列化 tip 容器", () => {
      const blocks = [
        {
          id: generateBlockId(),
          type: "container" as const,
          props: { containerType: "tip" as const },
          content: [{ type: "text" as const, text: "提示\nThis is a tip" }],
        },
      ];
      const md = blocksToMarkdown(blocks as any);
      expect(md).toContain(":::tip");
      expect(md).toContain(":::");
    });

    it("应正确序列化 warning 容器", () => {
      const blocks = [
        {
          id: generateBlockId(),
          type: "container" as const,
          props: { containerType: "warning" as const },
          content: [{ type: "text" as const, text: "Warning content" }],
        },
      ];
      const md = blocksToMarkdown(blocks as any);
      expect(md).toContain(":::warning");
    });

    it("应正确序列化 details 容器", () => {
      const blocks = [
        {
          id: generateBlockId(),
          type: "container" as const,
          props: { containerType: "details" as const },
          content: [{ type: "text" as const, text: "详情\nHidden content" }],
        },
      ];
      const md = blocksToMarkdown(blocks as any);
      expect(md).toContain(":::details");
    });
  });

  describe("Math 序列化", () => {
    it("应正确序列化行内公式", () => {
      const blocks = [
        {
          id: generateBlockId(),
          type: "paragraph" as const,
          content: [
            { type: "text" as const, text: "The formula " },
            { type: "inlineMath" as const, formula: "E=mc^2" },
            { type: "text" as const, text: " is famous." },
          ],
        },
      ];
      const md = blocksToMarkdown(blocks as any);
      expect(md).toContain("$E=mc^2$");
    });

    it("应正确序列化块级公式 (displayMode)", () => {
      const blocks = [
        {
          id: generateBlockId(),
          type: "math" as const,
          props: {
            formula: "\\int_0^1 x^2 dx",
            display: "block" as const,
          },
        },
      ];
      const md = blocksToMarkdown(blocks as any);
      expect(md).toContain("$$");
      expect(md).toContain("\\int_0^1 x^2 dx");
    });
  });

  describe("Code Block 序列化", () => {
    it("应正确序列化 shikiCode 块", () => {
      const blocks = [
        {
          id: generateBlockId(),
          type: "shikiCode" as const,
          props: {
            code: "console.log('hello')",
            language: "js",
          },
        },
      ];
      const md = blocksToMarkdown(blocks as any);
      expect(md).toContain("```js");
      expect(md).toContain("console.log");
      expect(md).toContain("```");
    });

    it("应保留文件名标记", () => {
      const blocks = [
        {
          id: generateBlockId(),
          type: "shikiCode" as const,
          props: {
            code: "export default {}",
            language: "ts",
            filename: "config.ts",
          },
        },
      ];
      const md = blocksToMarkdown(blocks as any);
      expect(md).toContain("[config.ts]");
    });

    it("应保留行高亮标记", () => {
      const blocks = [
        {
          id: generateBlockId(),
          type: "shikiCode" as const,
          props: {
            code: "line1\nline2\nline3",
            language: "js",
            highlightLines: "{1,3}",
          },
        },
      ];
      const md = blocksToMarkdown(blocks as any);
      expect(md).toContain("{1,3}");
    });
  });

  describe("Quote 序列化", () => {
    it("应正确序列化引用块", () => {
      const blocks = [
        {
          id: generateBlockId(),
          type: "quote" as const,
          children: [
            {
              id: generateBlockId(),
              type: "paragraph" as const,
              content: [{ type: "text" as const, text: "Quoted text" }],
            },
          ],
        },
      ];
      const md = blocksToMarkdown(blocks as any);
      expect(md).toContain("> Quoted text");
    });
  });

  describe("Table 序列化", () => {
    it("应正确序列化表格", () => {
      const blocks = [
        {
          id: generateBlockId(),
          type: "table" as const,
          props: {
            headerRow: {
              cells: [
                { content: [{ type: "text" as const, text: "Header 1" }] },
                { content: [{ type: "text" as const, text: "Header 2" }] },
              ],
            },
            rows: [
              {
                cells: [
                  { content: [{ type: "text" as const, text: "Cell 1" }] },
                  { content: [{ type: "text" as const, text: "Cell 2" }] },
                ],
              },
            ],
          },
        },
      ];
      const md = blocksToMarkdown(blocks as any);
      expect(md).toContain("|");
      expect(md).toContain("Header 1");
      expect(md).toContain("---");
    });
  });

  describe("Vue 组件序列化", () => {
    it("应正确序列化自闭合 Vue 组件", () => {
      const blocks = [
        {
          id: generateBlockId(),
          type: "vueComponent" as const,
          props: {
            componentName: "Badge",
            attributes: { type: "tip", text: "new" },
            selfClosing: true,
          },
        },
      ];
      const md = blocksToMarkdown(blocks as any);
      expect(md).toContain("<Badge");
      expect(md).toContain("/>");
    });
  });

  describe("@include 序列化", () => {
    it("应正确序列化 @include 指令", () => {
      const blocks = [
        {
          id: generateBlockId(),
          type: "include" as const,
          props: {
            path: "./components/example.md",
          },
        },
      ];
      const md = blocksToMarkdown(blocks as any);
      expect(md).toContain("<!--@include:");
      expect(md).toContain("example.md");
    });

    it("应正确序列化带行号范围的 @include", () => {
      const blocks = [
        {
          id: generateBlockId(),
          type: "include" as const,
          props: {
            path: "./file.md",
            lineRange: { start: 1, end: 5 },
          },
        },
      ];
      const md = blocksToMarkdown(blocks as any);
      expect(md).toContain("{1-5}");
    });
  });
});

describe("documentToMarkdown", () => {
  it("应正确序列化包含 Frontmatter 的文档", () => {
    const doc: Document = {
      path: "test.md",
      frontmatter: { title: "Test", description: "A test" },
      blocks: [
        {
          id: generateBlockId(),
          type: "paragraph",
          content: [{ type: "text", text: "Content" }],
        } as any,
      ],
    };
    const md = documentToMarkdown(doc);
    expect(md).toContain("---");
    expect(md).toContain("title: Test");
    expect(md).toContain("Content");
  });
});

describe("singleBlockToMarkdown", () => {
  it("应正确序列化单个块", () => {
    const block = {
      id: generateBlockId(),
      type: "paragraph" as const,
      content: [{ type: "text" as const, text: "Single" }],
    };
    const md = singleBlockToMarkdown(block as any);
    expect(md).toContain("Single");
  });
});
