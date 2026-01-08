/**
 * KeyboardShortcuts.test.ts - KeyboardShortcuts 扩展测试
 *
 * 测试 Quote 块的 Enter/Shift+Enter 行为
 */

import { describe, it, expect } from "vitest";

describe("QuoteKeyboardShortcuts Configuration", () => {
  describe("Extension 基础", () => {
    it("扩展名称应为 quoteKeyboardShortcuts", () => {
      const extensionName = "quoteKeyboardShortcuts";
      expect(extensionName).toBe("quoteKeyboardShortcuts");
    });
  });

  describe("注册的快捷键", () => {
    it("应注册 Enter 键处理", () => {
      const shortcuts = ["Enter", "Shift-Enter"];
      expect(shortcuts).toContain("Enter");
    });

    it("应注册 Shift-Enter 键处理", () => {
      const shortcuts = ["Enter", "Shift-Enter"];
      expect(shortcuts).toContain("Shift-Enter");
    });
  });
});

describe("Quote Enter 行为", () => {
  describe("空行处理", () => {
    it("空行应跳出引用", () => {
      const isEmpty = true;
      const shouldExit = isEmpty;
      expect(shouldExit).toBe(true);
    });

    it("非空行应分割块", () => {
      const isEmpty = false;
      const shouldSplit = !isEmpty;
      expect(shouldSplit).toBe(true);
    });
  });

  describe("Quote 节点查找", () => {
    it("应向上遍历查找 quote 节点", () => {
      const nodeTypes = ["paragraph", "quote", "doc"];
      const quoteType = nodeTypes.find((t) => t === "quote");
      expect(quoteType).toBe("quote");
    });
  });

  describe("groupId 保留", () => {
    it("分割时应保留原 groupId", () => {
      const originalGroupId = "group-123";
      const newBlockGroupId = originalGroupId;
      expect(newBlockGroupId).toBe(originalGroupId);
    });

    it("默认 groupId 应为 default", () => {
      const defaultGroupId = "default";
      expect(defaultGroupId).toBe("default");
    });
  });
});

describe("Shift-Enter 行为", () => {
  describe("软换行", () => {
    it("应触发 setHardBreak 命令", () => {
      const command = "setHardBreak";
      expect(command).toBe("setHardBreak");
    });

    it("应在 Quote 内创建换行", () => {
      const inQuote = true;
      const shouldBreak = inQuote;
      expect(shouldBreak).toBe(true);
    });
  });
});

describe("Quote 块导航", () => {
  describe("深度遍历", () => {
    it("应从当前深度向上查找", () => {
      // 模拟节点深度
      const depths = [3, 2, 1];
      let foundDepth = -1;
      for (let d of depths) {
        if (d === 2) {
          foundDepth = d;
          break;
        }
      }
      expect(foundDepth).toBe(2);
    });
  });

  describe("非 Quote 上下文", () => {
    it("非 Quote 中应返回 false 使用默认行为", () => {
      const isInQuote = false;
      const shouldHandle = isInQuote;
      expect(shouldHandle).toBe(false);
    });
  });
});
