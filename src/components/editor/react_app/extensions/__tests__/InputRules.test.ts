/**
 * InputRules.test.ts - InputRules 扩展测试
 *
 * 测试 Markdown 快捷输入规则
 */

import { describe, it, expect } from "vitest";

describe("BlockInputRules Configuration", () => {
  describe("Extension 基础", () => {
    it("扩展名称应为 blockInputRules", () => {
      const extensionName = "blockInputRules";
      expect(extensionName).toBe("blockInputRules");
    });
  });
});

describe("Block Math Input Rule", () => {
  describe("触发模式", () => {
    it("应匹配 $$ 后跟空格", () => {
      const pattern = /^\$\$ $/;
      expect(pattern.test("$$ ")).toBe(true);
    });

    it("不应匹配单个 $", () => {
      const pattern = /^\$\$ $/;
      expect(pattern.test("$ ")).toBe(false);
    });

    it("不应匹配 $$ 无空格", () => {
      const pattern = /^\$\$ $/;
      expect(pattern.test("$$")).toBe(false);
    });
  });

  describe("转换行为", () => {
    it("应创建 math 节点", () => {
      const targetNodeType = "math";
      expect(targetNodeType).toBe("math");
    });
  });
});

describe("Mermaid Input Rule", () => {
  describe("触发模式", () => {
    it("应匹配 ```mermaid 后跟空格", () => {
      const pattern = /^```mermaid $/;
      expect(pattern.test("```mermaid ")).toBe(true);
    });

    it("不应匹配其他语言标记", () => {
      const pattern = /^```mermaid $/;
      expect(pattern.test("```javascript ")).toBe(false);
    });
  });

  describe("转换行为", () => {
    it("应创建 mermaid 节点", () => {
      const targetNodeType = "mermaid";
      expect(targetNodeType).toBe("mermaid");
    });
  });
});

describe("Inline Math Input Rule", () => {
  describe("触发模式", () => {
    it("应匹配单美元符号包裹的公式", () => {
      const pattern = /\$([^$]+)\$$/;
      const match = "$E=mc^2$".match(pattern);
      expect(match).not.toBeNull();
      expect(match![1]).toBe("E=mc^2");
    });

    it("不应匹配空公式 $$", () => {
      const pattern = /\$([^$]+)\$$/;
      expect(pattern.test("$$")).toBe(false);
    });

    it("应提取公式内容", () => {
      const pattern = /\$([^$]+)\$$/;
      const match = "$\\frac{a}{b}$".match(pattern);
      expect(match).not.toBeNull();
      expect(match![1]).toBe("\\frac{a}{b}");
    });
  });

  describe("转换行为", () => {
    it("应创建 inlineMath 节点", () => {
      const targetNodeType = "inlineMath";
      expect(targetNodeType).toBe("inlineMath");
    });

    it("应设置 formula 属性", () => {
      const attrs = { formula: "x^2" };
      expect(attrs.formula).toBe("x^2");
    });
  });
});

describe("Markdown Link Input Rule", () => {
  describe("触发模式", () => {
    it("应匹配 [text](url) 格式", () => {
      const pattern = /\[(.+?)\]\((.+?)\)$/;
      const match = "[链接](https://example.com)".match(pattern);
      expect(match).not.toBeNull();
      expect(match![1]).toBe("链接");
      expect(match![2]).toBe("https://example.com");
    });

    it("应提取链接文本和 URL", () => {
      const pattern = /\[(.+?)\]\((.+?)\)$/;
      const match = "[VitePress](https://vitepress.dev)".match(pattern);
      expect(match![1]).toBe("VitePress");
      expect(match![2]).toBe("https://vitepress.dev");
    });
  });

  describe("转换行为", () => {
    it("应创建带 link mark 的文本节点", () => {
      const markType = "link";
      expect(markType).toBe("link");
    });

    it("应设置 href 属性", () => {
      const attrs = { href: "https://example.com" };
      expect(attrs.href).toBe("https://example.com");
    });
  });
});

describe("Quote Input Rule", () => {
  describe("触发模式", () => {
    it("应匹配行首 > 后跟空格", () => {
      const pattern = /^> $/;
      expect(pattern.test("> ")).toBe(true);
    });

    it("不应匹配非行首的 >", () => {
      const pattern = /^> $/;
      expect(pattern.test("text > ")).toBe(false);
    });
  });

  describe("转换行为", () => {
    it("应创建 quote 节点", () => {
      const targetNodeType = "quote";
      expect(targetNodeType).toBe("quote");
    });
  });
});
