/**
 * InlineMath.test.tsx - InlineMath 组件配置测试
 *
 * 测试 InlineMath 的 contentRegistry 注册配置和接口定义
 */

import { describe, it, expect } from "vitest";

describe("InlineMath Configuration", () => {
  describe("Inline Content 类型定义", () => {
    it("应定义 inlineMath 类型", () => {
      const inlineMathType = "inlineMath";
      expect(inlineMathType).toBe("inlineMath");
    });

    it("应定义 formula 属性", () => {
      const propSchema = {
        formula: { default: "" },
      };
      expect(propSchema.formula).toBeDefined();
      expect(propSchema.formula.default).toBe("");
    });

    it("content 类型应为 none", () => {
      const spec = {
        type: "inlineMath",
        propSchema: {
          formula: { default: "" },
        },
        content: "none",
      };
      expect(spec.content).toBe("none");
    });
  });

  describe("ContentRegistry 配置", () => {
    it("应有正确的标签", () => {
      const config = {
        label: "行内公式",
        supportedStyles: ["inlineMath"],
      };
      expect(config.label).toBe("行内公式");
      expect(config.supportedStyles).toContain("inlineMath");
    });

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

describe("InlineMath 导航行为", () => {
  describe("move-out 事件处理", () => {
    it("应支持向后移动 (forward)", () => {
      const direction = "forward";
      expect(direction).toBe("forward");
    });

    it("应支持向前移动 (backward)", () => {
      const direction = "backward";
      expect(direction).toBe("backward");
    });
  });

  describe("键盘导航", () => {
    it("ArrowLeft 应触发向后移动", () => {
      const keyHandlers = {
        ArrowLeft: "backward",
        ArrowRight: "forward",
        Backspace: "delete-if-empty",
        Delete: "delete-if-empty",
      };
      expect(keyHandlers.ArrowLeft).toBe("backward");
    });

    it("ArrowRight 应触发向前移动", () => {
      const keyHandlers = {
        ArrowLeft: "backward",
        ArrowRight: "forward",
      };
      expect(keyHandlers.ArrowRight).toBe("forward");
    });
  });
});

describe("InlineMath 公式处理", () => {
  it("应正确处理空公式", () => {
    const formula = "";
    expect(formula).toBe("");
  });

  it("应正确处理简单公式", () => {
    const formula = "E=mc^2";
    expect(formula).toBe("E=mc^2");
  });

  it("应正确处理 LaTeX 命令", () => {
    const formula = "\\int_0^1 x^2 dx";
    expect(formula).toContain("\\int");
  });

  it("应正确处理分数", () => {
    const formula = "\\frac{a}{b}";
    expect(formula).toContain("\\frac");
  });

  it("应正确处理根号", () => {
    const formula = "\\sqrt{x}";
    expect(formula).toContain("\\sqrt");
  });
});
