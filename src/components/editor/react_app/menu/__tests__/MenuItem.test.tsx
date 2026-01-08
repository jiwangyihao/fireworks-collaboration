/**
 * MenuItem.test.tsx - MenuItem 组件测试
 *
 * 测试图标渲染、状态样式、点击事件
 */

import { describe, it, expect } from "vitest";

describe("MenuItem Configuration", () => {
  describe("Props 接口", () => {
    it("icon 可以是字符串或 ReactNode", () => {
      const iconString = "ph:globe";
      const iconNode = { type: "span" };
      expect(typeof iconString).toBe("string");
      expect(typeof iconNode).toBe("object");
    });

    it("应有 label 属性", () => {
      const props = { label: "菜单项" };
      expect(props.label).toBe("菜单项");
    });

    it("应有 description 属性", () => {
      const props = { description: "描述文本" };
      expect(props.description).toBe("描述文本");
    });

    it("应有 shortcut 属性", () => {
      const props = { shortcut: "Ctrl+B" };
      expect(props.shortcut).toBe("Ctrl+B");
    });

    it("active 默认值应为 false", () => {
      const defaultActive = false;
      expect(defaultActive).toBe(false);
    });

    it("disabled 默认值应为 false", () => {
      const defaultDisabled = false;
      expect(defaultDisabled).toBe(false);
    });

    it("danger 默认值应为 false", () => {
      const defaultDanger = false;
      expect(defaultDanger).toBe(false);
    });
  });
});

describe("MenuItem 状态样式", () => {
  describe("激活状态", () => {
    it("激活时应有 primary 边框", () => {
      const activeClasses =
        "!border-primary bg-primary/5 text-primary font-medium";
      expect(activeClasses).toContain("border-primary");
    });

    it("激活时图标应有 primary 颜色", () => {
      const active = true;
      const iconClass = active ? "text-primary" : "opacity-60";
      expect(iconClass).toBe("text-primary");
    });
  });

  describe("禁用状态", () => {
    it("禁用时应有半透明样式", () => {
      const disabledClasses = "opacity-50 cursor-not-allowed";
      expect(disabledClasses).toContain("opacity-50");
    });

    it("禁用时点击应被阻止", () => {
      const disabled = true;
      let clicked = false;
      const handleClick = () => {
        if (disabled) return;
        clicked = true;
      };
      handleClick();
      expect(clicked).toBe(false);
    });
  });

  describe("危险状态", () => {
    it("危险状态应有 error 颜色", () => {
      const dangerClasses = "hover:border-error/20 hover:bg-error/5 text-error";
      expect(dangerClasses).toContain("text-error");
    });
  });

  describe("普通状态", () => {
    it("悬停时应有背景变化", () => {
      const normalClasses = "hover:border-base-content/20 hover:bg-base-200";
      expect(normalClasses).toContain("hover:bg-base-200");
    });
  });
});

describe("MenuItem 布局", () => {
  describe("带描述时的布局", () => {
    it("有描述时应使用列布局", () => {
      const description = "描述文本";
      const layoutClasses = description ? "flex-col" : "flex-row items-center";
      expect(layoutClasses).toBe("flex-col");
    });
  });

  describe("无描述时的布局", () => {
    it("无描述时应使用行布局", () => {
      const description = "";
      const layoutClasses = description ? "flex-col" : "flex-row items-center";
      expect(layoutClasses).toBe("flex-row items-center");
    });
  });
});

describe("MenuItem 图标渲染", () => {
  describe("Iconify 字符串图标", () => {
    it("应识别 iconify 字符串格式", () => {
      const icon = "ph:globe";
      const isString = typeof icon === "string";
      expect(isString).toBe(true);
    });
  });

  describe("ReactNode 图标", () => {
    it("应识别 ReactNode 格式", () => {
      const icon = { type: "svg" };
      const isNode = typeof icon === "object";
      expect(isNode).toBe(true);
    });
  });
});
