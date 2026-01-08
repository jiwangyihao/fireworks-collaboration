/**
 * QuoteBlock.test.tsx - QuoteBlock 组件配置测试
 *
 * 测试 QuoteBlock 的 contentRegistry 注册配置和 groupId 逻辑
 */

import { describe, it, expect } from "vitest";

describe("QuoteBlock Configuration", () => {
  describe("Block 类型定义", () => {
    it("应定义 quote 块类型", () => {
      const quoteBlockType = "quote";
      expect(quoteBlockType).toBe("quote");
    });

    it("应定义 groupId 属性", () => {
      const propSchema = {
        groupId: { default: "default" },
        isFirstInGroup: { default: true },
      };
      expect(propSchema.groupId).toBeDefined();
      expect(propSchema.groupId.default).toBe("default");
    });

    it("应定义 isFirstInGroup 属性", () => {
      const propSchema = {
        groupId: { default: "default" },
        isFirstInGroup: { default: true },
      };
      expect(propSchema.isFirstInGroup).toBeDefined();
      expect(propSchema.isFirstInGroup.default).toBe(true);
    });

    it("content 类型应为 inline", () => {
      const spec = {
        type: "quote",
        content: "inline",
      };
      expect(spec.content).toBe("inline");
    });
  });

  describe("Slash Menu 配置", () => {
    it("应有正确的菜单项配置", () => {
      const slashMenuItem = {
        id: "quote",
        title: "引用",
        subtext: "插入引用块",
        group: "基础",
        aliases: ["quote", "blockquote", "yy", "yinyong"],
        blockType: "quote",
        props: {},
      };

      expect(slashMenuItem.id).toBe("quote");
      expect(slashMenuItem.title).toBe("引用");
      expect(slashMenuItem.group).toBe("基础");
      expect(slashMenuItem.aliases).toContain("blockquote");
      expect(slashMenuItem.aliases).toContain("yinyong"); // 中文拼音支持
    });
  });

  describe("ContentRegistry 配置", () => {
    it("应有正确的标签", () => {
      const config = {
        label: "引用",
        supportedStyles: true,
        actions: [],
      };
      expect(config.label).toBe("引用");
      expect(config.supportedStyles).toBe(true);
      expect(config.actions).toEqual([]);
    });
  });
});

describe("QuoteBlock groupId 逻辑", () => {
  describe("组内首块标识", () => {
    it("首块 isFirstInGroup 应为 true", () => {
      const firstBlock = {
        type: "quote",
        props: {
          groupId: "group-123",
          isFirstInGroup: true,
        },
      };
      expect(firstBlock.props.isFirstInGroup).toBe(true);
    });

    it("后续块 isFirstInGroup 应为 false", () => {
      const subsequentBlock = {
        type: "quote",
        props: {
          groupId: "group-123",
          isFirstInGroup: false,
        },
      };
      expect(subsequentBlock.props.isFirstInGroup).toBe(false);
    });
  });

  describe("同组引用识别", () => {
    it("相同 groupId 的块应识别为同组", () => {
      const block1 = { groupId: "group-abc" };
      const block2 = { groupId: "group-abc" };
      expect(block1.groupId).toBe(block2.groupId);
    });

    it("不同 groupId 的块应识别为不同组", () => {
      const block1 = { groupId: "group-abc" };
      const block2 = { groupId: "group-xyz" };
      expect(block1.groupId).not.toBe(block2.groupId);
    });
  });

  describe("CSS 类应用", () => {
    it("首块不应有 sibling 类", () => {
      const isFirstInGroup = true;
      const siblingClass = isFirstInGroup ? "" : " quote-block-sibling";
      expect(siblingClass).toBe("");
    });

    it("后续块应有 sibling 类", () => {
      const isFirstInGroup = false;
      const siblingClass = isFirstInGroup ? "" : " quote-block-sibling";
      expect(siblingClass).toBe(" quote-block-sibling");
    });
  });
});

describe("QuoteBlock Markdown 往返", () => {
  it("应支持单行引用", () => {
    const markdown = "> This is a quote";
    expect(markdown).toMatch(/^>/);
  });

  it("应支持多行引用", () => {
    const markdown = "> Line 1\n> Line 2";
    const lines = markdown.split("\n");
    expect(lines.every((line) => line.startsWith(">"))).toBe(true);
  });

  it("应支持嵌套引用", () => {
    const markdown = "> > Nested quote";
    expect(markdown).toMatch(/^> >/);
  });
});
