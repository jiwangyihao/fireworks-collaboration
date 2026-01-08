/**
 * Markdown 转换器单元测试
 *
 * 测试 Markdown ↔ Block 双向转换的正确性。
 */

import { describe, it, expect } from "vitest";
import {
  markdownToBlocks,
  blocksToMarkdown,
  parseMarkdownDocument,
  documentToMarkdown,
} from "../markdown-converter";
import type {
  HeadingBlock,
  ParagraphBlock,
  CodeBlockBlock,
  MathBlock,
  MermaidBlock,
  VueComponentBlock,
  IncludeBlock,
  BulletListItemBlock,
  NumberedListItemBlock,
  CheckListItemBlock,
  QuoteBlock,
  ImageBlock,
  TableBlock,
} from "../markdown-converter";

// ============================================================================
// Markdown → Block 测试
// ============================================================================

describe("markdownToBlocks", () => {
  describe("标准 Markdown 语法", () => {
    it("应正确解析标题", () => {
      const md = "# 一级标题\n\n## 二级标题\n\n### 三级标题";
      const blocks = markdownToBlocks(md);

      expect(blocks).toHaveLength(3);
      expect((blocks[0] as HeadingBlock).type).toBe("heading");
      expect((blocks[0] as HeadingBlock).props.level).toBe(1);
      expect((blocks[1] as HeadingBlock).props.level).toBe(2);
      expect((blocks[2] as HeadingBlock).props.level).toBe(3);
    });

    it("应正确解析段落", () => {
      const md = "这是一个段落。\n\n这是另一个段落。";
      const blocks = markdownToBlocks(md);

      expect(blocks).toHaveLength(2);
      expect(blocks[0].type).toBe("paragraph");
      expect(blocks[1].type).toBe("paragraph");
    });

    it("应正确解析无序列表", () => {
      const md = "- 项目一\n- 项目二\n- 项目三";
      const blocks = markdownToBlocks(md);

      expect(blocks).toHaveLength(3);
      blocks.forEach((block) => {
        expect(block.type).toBe("bulletListItem");
      });
    });

    it("应正确解析有序列表", () => {
      const md = "1. 第一项\n2. 第二项\n3. 第三项";
      const blocks = markdownToBlocks(md);

      expect(blocks).toHaveLength(3);
      blocks.forEach((block) => {
        expect(block.type).toBe("numberedListItem");
      });
      expect((blocks[0] as NumberedListItemBlock).props.start).toBe(1);
      expect((blocks[1] as NumberedListItemBlock).props.start).toBe(2);
    });

    it("应正确解析复选框列表", () => {
      const md = "- [x] 已完成\n- [ ] 未完成";
      const blocks = markdownToBlocks(md);

      expect(blocks).toHaveLength(2);
      expect((blocks[0] as CheckListItemBlock).type).toBe("checkListItem");
      expect((blocks[0] as CheckListItemBlock).props.checked).toBe(true);
      expect((blocks[1] as CheckListItemBlock).props.checked).toBe(false);
    });

    it("应正确解析代码块", () => {
      const md = "```typescript\nconst x = 1;\n```";
      const blocks = markdownToBlocks(md);

      expect(blocks).toHaveLength(1);
      // 代码块现在被解析为 shikiCode 类型
      expect(blocks[0].type).toBe("shikiCode");
      expect((blocks[0] as any).props.language).toBe("typescript");
      expect((blocks[0] as any).props.code).toBe("const x = 1;");
    });

    it("应正确解析引用块", () => {
      const md = "> 这是引用内容\n> 继续引用";
      const blocks = markdownToBlocks(md);

      expect(blocks).toHaveLength(1);
      expect((blocks[0] as QuoteBlock).type).toBe("quote");
      expect((blocks[0] as QuoteBlock).children).toBeDefined();
    });

    it("应正确解析图片", () => {
      const md = '![替代文本](https://example.com/image.png "图片标题")';
      const blocks = markdownToBlocks(md);

      expect(blocks).toHaveLength(1);
      expect((blocks[0] as ImageBlock).type).toBe("image");
      expect((blocks[0] as ImageBlock).props.src).toBe(
        "https://example.com/image.png"
      );
      expect((blocks[0] as ImageBlock).props.alt).toBe("替代文本");
      expect((blocks[0] as ImageBlock).props.title).toBe("图片标题");
    });

    it("应正确解析表格", () => {
      const md = "| 列1 | 列2 |\n| --- | --- |\n| A | B |\n| C | D |";
      const blocks = markdownToBlocks(md);

      expect(blocks).toHaveLength(1);
      expect((blocks[0] as TableBlock).type).toBe("table");
      expect((blocks[0] as TableBlock).props.headerRow.cells).toHaveLength(2);
      expect((blocks[0] as TableBlock).props.rows).toHaveLength(2);
    });

    it("应正确解析分割线", () => {
      const md = "段落一\n\n---\n\n段落二";
      const blocks = markdownToBlocks(md);

      expect(blocks).toHaveLength(3);
      expect(blocks[1].type).toBe("thematicBreak");
    });
  });

  describe("行内内容", () => {
    it("应正确解析加粗文本", () => {
      const md = "这是**加粗**文本";
      const blocks = markdownToBlocks(md);
      const paragraph = blocks[0] as ParagraphBlock;

      expect(paragraph.content.length).toBeGreaterThan(0);
      const hasStrong = paragraph.content.some((c) => c.type === "strong");
      expect(hasStrong).toBe(true);
    });

    it("应正确解析斜体文本", () => {
      const md = "这是*斜体*文本";
      const blocks = markdownToBlocks(md);
      const paragraph = blocks[0] as ParagraphBlock;

      const hasEmphasis = paragraph.content.some((c) => c.type === "emphasis");
      expect(hasEmphasis).toBe(true);
    });

    it("应正确解析行内代码", () => {
      const md = "使用 `const` 关键字";
      const blocks = markdownToBlocks(md);
      const paragraph = blocks[0] as ParagraphBlock;

      const hasCode = paragraph.content.some((c) => c.type === "code");
      expect(hasCode).toBe(true);
    });

    it("应正确解析链接", () => {
      const md = "访问 [GitHub](https://github.com)";
      const blocks = markdownToBlocks(md);
      const paragraph = blocks[0] as ParagraphBlock;

      const hasLink = paragraph.content.some(
        (c) => c.type === "link" && c.href === "https://github.com"
      );
      expect(hasLink).toBe(true);
    });
  });

  describe("VitePress 扩展语法", () => {
    it("应正确识别 Mermaid 代码块", () => {
      const md = "```mermaid\ngraph TD\n  A-->B\n```";
      const blocks = markdownToBlocks(md);

      expect(blocks).toHaveLength(1);
      expect((blocks[0] as MermaidBlock).type).toBe("mermaid");
      expect((blocks[0] as MermaidBlock).props.code).toContain("graph TD");
    });

    it("应正确解析块级数学公式", () => {
      const md = "$$\nE = mc^2\n$$";
      const blocks = markdownToBlocks(md);

      expect(blocks).toHaveLength(1);
      expect((blocks[0] as MathBlock).type).toBe("math");
      expect((blocks[0] as MathBlock).props.formula).toBe("E = mc^2");
      // 块级公式通过类型 'math' 隐式表示，无需 display 属性
    });

    it("应正确解析行内数学公式", () => {
      const md = "质能方程 $E = mc^2$ 很著名";
      const blocks = markdownToBlocks(md);
      const paragraph = blocks[0] as ParagraphBlock;

      const hasInlineMath = paragraph.content.some(
        (c) => c.type === "inlineMath"
      );
      expect(hasInlineMath).toBe(true);
    });

    it("应正确解析 Vue 组件", () => {
      const md = '<OList path="/数学学院/初等数论" />';
      const blocks = markdownToBlocks(md);

      expect(blocks).toHaveLength(1);
      expect((blocks[0] as VueComponentBlock).type).toBe("vueComponent");
      expect((blocks[0] as VueComponentBlock).props.componentName).toBe(
        "OList"
      );
      expect((blocks[0] as VueComponentBlock).props.attributes.path).toBe(
        "/数学学院/初等数论"
      );
    });

    it("应正确解析 @include 指令", () => {
      const md = "<!--@include: @/parts/wip.md-->";
      const blocks = markdownToBlocks(md);

      expect(blocks).toHaveLength(1);
      expect((blocks[0] as IncludeBlock).type).toBe("include");
      expect((blocks[0] as IncludeBlock).props.path).toBe("@/parts/wip.md");
    });

    it("应正确解析带行范围的 @include 指令", () => {
      const md = "<!--@include: ./file.md{5-10}-->";
      const blocks = markdownToBlocks(md);

      expect(blocks).toHaveLength(1);
      const include = blocks[0] as IncludeBlock;
      expect(include.props.lineRange?.start).toBe(5);
      expect(include.props.lineRange?.end).toBe(10);
    });

    it("应正确解析带区域的 @include 指令", () => {
      const md = "<!--@include: ./file.md#regionName-->";
      const blocks = markdownToBlocks(md);

      expect(blocks).toHaveLength(1);
      const include = blocks[0] as IncludeBlock;
      expect(include.props.region).toBe("regionName");
    });
  });
});

// ============================================================================
// Block → Markdown 测试
// ============================================================================

describe("blocksToMarkdown", () => {
  it("应正确序列化标题", () => {
    const blocks = markdownToBlocks("# 测试标题");
    const md = blocksToMarkdown(blocks);

    expect(md).toBe("# 测试标题");
  });

  it("应正确序列化段落", () => {
    const blocks = markdownToBlocks("这是段落内容");
    const md = blocksToMarkdown(blocks);

    expect(md).toBe("这是段落内容");
  });

  it("应正确序列化代码块", () => {
    const blocks = markdownToBlocks("```js\nconst x = 1;\n```");
    const md = blocksToMarkdown(blocks);

    expect(md).toContain("```js");
    expect(md).toContain("const x = 1;");
    expect(md).toContain("```");
  });

  it("应正确序列化无序列表", () => {
    const blocks = markdownToBlocks("- 项目一\n- 项目二");
    const md = blocksToMarkdown(blocks);

    expect(md).toContain("- 项目一");
    expect(md).toContain("- 项目二");
  });

  it("应正确序列化 Mermaid 块", () => {
    const blocks = markdownToBlocks("```mermaid\ngraph TD\n```");
    const md = blocksToMarkdown(blocks);

    expect(md).toContain("```mermaid");
    expect(md).toContain("graph TD");
  });

  it("应正确序列化数学公式", () => {
    const blocks = markdownToBlocks("$$\nE = mc^2\n$$");
    const md = blocksToMarkdown(blocks);

    expect(md).toContain("$$");
    expect(md).toContain("E = mc^2");
  });

  it("应正确序列化 Vue 组件", () => {
    const blocks = markdownToBlocks('<OList path="/test" />');
    const md = blocksToMarkdown(blocks);

    expect(md).toContain("<OList");
    expect(md).toContain('path="/test"');
    expect(md).toContain("/>");
  });

  it("应正确序列化 @include 指令", () => {
    const blocks = markdownToBlocks("<!--@include: @/parts/wip.md-->");
    const md = blocksToMarkdown(blocks);

    expect(md).toBe("<!--@include: @/parts/wip.md-->");
  });

  it("应正确序列化表格", () => {
    const blocks = markdownToBlocks("| A | B |\n| --- | --- |\n| 1 | 2 |");
    const md = blocksToMarkdown(blocks);

    expect(md).toContain("| A | B |");
    expect(md).toContain("| --- | --- |");
    expect(md).toContain("| 1 | 2 |");
  });
});

// ============================================================================
// 往返转换测试
// ============================================================================

describe("往返转换一致性", () => {
  const testCases = [
    { name: "标题", md: "# 测试标题" },
    { name: "段落", md: "这是一个段落" },
    { name: "代码块", md: "```js\nconst x = 1;\n```" },
    { name: "图片", md: "![alt](https://example.com/img.png)" },
    { name: "分割线", md: "---" },
    { name: "Mermaid", md: "```mermaid\ngraph TD\n```" },
    { name: "数学公式", md: "$$\nE = mc^2\n$$" },
    { name: "@include 指令", md: "<!--@include: @/parts/wip.md-->" },
  ];

  testCases.forEach(({ name, md }) => {
    it(`${name}: md → blocks → md 应保持语义一致`, () => {
      const blocks = markdownToBlocks(md);
      const result = blocksToMarkdown(blocks);

      // 规范化比较（去除多余空白）
      const normalize = (s: string) => s.trim().replace(/\s+/g, " ");
      expect(normalize(result)).toBe(normalize(md));
    });
  });
});

// ============================================================================
// Document 解析测试
// ============================================================================

describe("parseMarkdownDocument", () => {
  it("应正确解析包含 Frontmatter 的文档", () => {
    const md = `---
title: 测试文档
description: 这是描述
tags:
  - tag1
  - tag2
---

# 文档内容

这是正文。`;

    const doc = parseMarkdownDocument(md, "/test.md");

    expect(doc.path).toBe("/test.md");
    expect(doc.frontmatter.title).toBe("测试文档");
    expect(doc.frontmatter.description).toBe("这是描述");
    expect(doc.frontmatter.tags).toEqual(["tag1", "tag2"]);
    expect(doc.blocks.length).toBeGreaterThan(0);
  });

  it("应正确处理没有 Frontmatter 的文档", () => {
    const md = "# 简单文档\n\n这是内容。";

    const doc = parseMarkdownDocument(md, "/simple.md");

    expect(doc.frontmatter).toEqual({});
    expect(doc.blocks).toHaveLength(2);
  });
});

describe("documentToMarkdown", () => {
  it("应正确序列化包含 Frontmatter 的文档", () => {
    const md = `---
title: 测试
---

# 标题`;

    const doc = parseMarkdownDocument(md);
    const result = documentToMarkdown(doc);

    expect(result).toContain("---");
    expect(result).toContain("title: 测试");
    expect(result).toContain("# 标题");
  });
});

// ============================================================================
// 边界情况测试
// ============================================================================

describe("边界情况", () => {
  it("应正确处理空文档", () => {
    const blocks = markdownToBlocks("");
    expect(blocks).toEqual([]);
  });

  it("应正确处理仅包含空白的文档", () => {
    const blocks = markdownToBlocks("   \n\n   ");
    expect(blocks).toEqual([]);
  });

  it("应正确处理嵌套列表", () => {
    const md = "- 外层\n  - 内层一\n  - 内层二";
    const blocks = markdownToBlocks(md);

    expect(blocks.length).toBeGreaterThan(0);
    const firstItem = blocks[0] as BulletListItemBlock;
    expect(firstItem.type).toBe("bulletListItem");
    // 嵌套列表应作为子块
    if (firstItem.children) {
      expect(firstItem.children.length).toBeGreaterThan(0);
    }
  });
});
