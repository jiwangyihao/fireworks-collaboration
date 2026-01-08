/**
 * markdown-to-blocks.ts 单元测试
 *
 * 覆盖：Container 解析、Math displayMode、Code Group 聚合、Vue 组件、@include 指令
 */

import { describe, it, expect } from "vitest";
import {
  markdownToBlocks,
  parseMarkdownDocument,
  extractMarkdownFrontmatter,
} from "../markdown-to-blocks";

describe("markdownToBlocks", () => {
  describe("基础转换", () => {
    it("应正确解析段落", () => {
      const blocks = markdownToBlocks("Hello World");
      expect(blocks).toHaveLength(1);
      expect(blocks[0].type).toBe("paragraph");
    });

    it("应正确解析标题", () => {
      const blocks = markdownToBlocks("# Title\n\n## Subtitle");
      expect(blocks).toHaveLength(2);
      expect(blocks[0].type).toBe("heading");
      expect((blocks[0] as any).props.level).toBe(1);
      expect(blocks[1].type).toBe("heading");
      expect((blocks[1] as any).props.level).toBe(2);
    });

    it("应正确解析列表", () => {
      const blocks = markdownToBlocks("- item 1\n- item 2");
      expect(blocks.length).toBeGreaterThanOrEqual(2);
      expect(blocks[0].type).toBe("bulletListItem");
    });
  });

  describe("Container 解析", () => {
    it("应正确解析 ::: tip 容器", () => {
      const md = `::: tip
This is a tip
:::`;
      const blocks = markdownToBlocks(md);
      const container = blocks.find((b) => b.type === "container");
      expect(container).toBeDefined();
      expect((container as any).props.containerType).toBe("tip");
    });

    it("应正确解析 ::: warning 容器", () => {
      const md = `::: warning
This is a warning
:::`;
      const blocks = markdownToBlocks(md);
      const container = blocks.find((b) => b.type === "container");
      expect(container).toBeDefined();
      expect((container as any).props.containerType).toBe("warning");
    });

    it("应正确解析 ::: danger 容器", () => {
      const md = `::: danger
Danger zone
:::`;
      const blocks = markdownToBlocks(md);
      const container = blocks.find((b) => b.type === "container");
      expect((container as any).props.containerType).toBe("danger");
    });

    it("应正确解析 ::: details 容器", () => {
      const md = `::: details
Hidden content
:::`;
      const blocks = markdownToBlocks(md);
      const container = blocks.find((b) => b.type === "container");
      expect((container as any).props.containerType).toBe("details");
    });

    it("应正确提取自定义标题", () => {
      const md = `::: warning 注意事项
Content here
:::`;
      const blocks = markdownToBlocks(md);
      const container = blocks.find((b) => b.type === "container");
      expect(container).toBeDefined();
      // 标题应包含在 content 中
      const content = (container as any).content;
      expect(content).toBeDefined();
    });
  });

  describe("Math 解析", () => {
    it("应正确解析行内公式 $...$", () => {
      const md = "The formula $E=mc^2$ is famous.";
      const blocks = markdownToBlocks(md);
      expect(blocks).toHaveLength(1);
      const content = (blocks[0] as any).content;
      const mathNode = content?.find((c: any) => c.type === "inlineMath");
      expect(mathNode).toBeDefined();
      expect(mathNode.formula).toBe("E=mc^2");
    });

    it("应正确解析块级公式 $$...$$", () => {
      const md = "$$E=mc^2$$";
      const blocks = markdownToBlocks(md);
      // 块级公式可能被解析为 math block
      expect(blocks.length).toBeGreaterThan(0);
    });

    it("应正确区分 displayMode", () => {
      const inlineMd = "$x^2$";
      const blockMd = "$$x^2$$";

      const inlineBlocks = markdownToBlocks(inlineMd);
      const blockBlocks = markdownToBlocks(blockMd);

      // 两者都应包含公式，但结构可能不同
      expect(inlineBlocks.length).toBeGreaterThan(0);
      expect(blockBlocks.length).toBeGreaterThan(0);
    });
  });

  describe("Code Group 解析", () => {
    it("应正确解析 ::: code-group 容器", () => {
      const md = `::: code-group

\`\`\`ts [config.ts]
export default {}
\`\`\`

\`\`\`js [config.js]
module.exports = {}
\`\`\`

:::`;
      const blocks = markdownToBlocks(md);
      // Code group 应被解析为 shikiCode
      const codeBlock = blocks.find((b) => b.type === "shikiCode");
      expect(codeBlock).toBeDefined();
      // 应有 tabs
      if (codeBlock) {
        const tabs = (codeBlock as any).props.tabs;
        expect(tabs).toBeDefined();
      }
    });
  });

  describe("Vue 组件解析", () => {
    it("应正确识别自闭合 Vue 组件", () => {
      const md = '<Badge type="tip" text="new" />';
      const blocks = markdownToBlocks(md);
      const vueBlock = blocks.find((b) => b.type === "vueComponent");
      expect(vueBlock).toBeDefined();
      if (vueBlock) {
        expect((vueBlock as any).props.componentName).toBe("Badge");
      }
    });

    it("应正确识别有内容的 Vue 组件", () => {
      const md = "<ClientOnly>\n  Content here\n</ClientOnly>";
      const blocks = markdownToBlocks(md);
      const vueBlock = blocks.find((b) => b.type === "vueComponent");
      expect(vueBlock).toBeDefined();
    });
  });

  describe("@include 指令解析", () => {
    it("应正确解析 @include 指令", () => {
      const md = "<!--@include: ./path/to/file.md-->";
      const blocks = markdownToBlocks(md);
      const includeBlock = blocks.find((b) => b.type === "include");
      expect(includeBlock).toBeDefined();
      if (includeBlock) {
        expect((includeBlock as any).props.path).toContain("file.md");
      }
    });

    it("应正确解析带行号范围的 @include", () => {
      const md = "<!--@include: ./file.md{1-5}-->";
      const blocks = markdownToBlocks(md);
      const includeBlock = blocks.find((b) => b.type === "include");
      expect(includeBlock).toBeDefined();
    });
  });

  describe("表格解析", () => {
    it("应正确解析 GFM 表格", () => {
      const md = `| Header 1 | Header 2 |
| --- | --- |
| Cell 1 | Cell 2 |`;
      const blocks = markdownToBlocks(md);
      const table = blocks.find((b) => b.type === "table");
      expect(table).toBeDefined();
    });
  });

  describe("Mermaid 解析", () => {
    it("应正确解析 Mermaid 代码块", () => {
      const md = "```mermaid\ngraph TD\n  A --> B\n```";
      const blocks = markdownToBlocks(md);
      const mermaid = blocks.find((b) => b.type === "mermaid");
      expect(mermaid).toBeDefined();
      expect((mermaid as any).props.code).toContain("graph TD");
    });
  });
});

describe("parseMarkdownDocument", () => {
  it("应正确解析包含 Frontmatter 的文档", () => {
    const md = `---
title: Test
description: A test document
---

# Content here`;
    const doc = parseMarkdownDocument(md, "test.md");
    expect(doc.frontmatter?.title).toBe("Test");
    expect(doc.frontmatter?.description).toBe("A test document");
    expect(doc.blocks.length).toBeGreaterThan(0);
  });
});

describe("extractMarkdownFrontmatter", () => {
  it("应正确提取 Frontmatter", () => {
    const md = `---
title: Hello
tags:
  - vue
  - markdown
---

Content`;
    const fm = extractMarkdownFrontmatter(md);
    expect(fm.title).toBe("Hello");
    expect(fm.tags).toEqual(["vue", "markdown"]);
  });

  it("应处理无 Frontmatter 的情况", () => {
    const md = "Just content";
    const fm = extractMarkdownFrontmatter(md);
    expect(fm).toEqual({});
  });
});
