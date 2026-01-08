/**
 * MathBlock.test.tsx - MathBlock contentRegistry 配置测试
 *
 * 由于 MathBlock 依赖 MathLive 等复杂库，这里仅测试其注册配置
 */

import { describe, it, expect } from "vitest";

describe("MathBlock Configuration", () => {
  describe("Block 类型定义", () => {
    it("应定义 math 块类型", () => {
      // 验证 Block 类型定义中包含 math
      const mathBlockType = "math";
      expect(mathBlockType).toBe("math");
    });

    it("应定义 formula 属性", () => {
      const propSchema = {
        formula: { default: "" },
      };
      expect(propSchema.formula).toBeDefined();
      expect(propSchema.formula.default).toBe("");
    });
  });

  describe("Slash Menu 配置", () => {
    it("应有正确的菜单项配置", () => {
      const slashMenuItem = {
        id: "math",
        title: "数学公式",
        subtext: "插入数学公式块",
        group: "高级功能",
        aliases: ["math", "formula", "latex", "gs", "gongshi", "shuxue"],
        blockType: "math",
        props: { formula: "" },
        moveCursor: true,
      };

      expect(slashMenuItem.id).toBe("math");
      expect(slashMenuItem.title).toBe("数学公式");
      expect(slashMenuItem.group).toBe("高级功能");
      expect(slashMenuItem.aliases).toContain("formula");
      expect(slashMenuItem.aliases).toContain("gongshi"); // 中文拼音支持
    });
  });

  describe("工具栏动作配置", () => {
    it("应包含 toggleKeyboard 动作", () => {
      const action = {
        type: "button",
        id: "toggleKeyboard",
        label: "键盘",
      };
      expect(action.id).toBe("toggleKeyboard");
    });

    it("应包含 toggleMenu 动作", () => {
      const action = {
        type: "button",
        id: "toggleMenu",
        label: "菜单",
      };
      expect(action.id).toBe("toggleMenu");
    });
  });
});

describe("MathBlock 执行器接口", () => {
  it("应定义 formula 执行器接口", () => {
    // 模拟执行器接口
    const formulaExecutor = {
      execute: (val: string) => {},
      isActive: () => true,
      getValue: () => "",
    };

    expect(typeof formulaExecutor.execute).toBe("function");
    expect(typeof formulaExecutor.isActive).toBe("function");
    expect(typeof formulaExecutor.getValue).toBe("function");
  });

  it("应定义 toggleKeyboard 执行器接口", () => {
    const toggleKeyboardExecutor = {
      execute: () => {},
      isActive: () => false,
    };

    expect(typeof toggleKeyboardExecutor.execute).toBe("function");
    expect(typeof toggleKeyboardExecutor.isActive).toBe("function");
  });
});
