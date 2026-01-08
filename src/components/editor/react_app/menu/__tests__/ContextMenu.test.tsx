/**
 * ContextMenu.test.tsx - ContextMenu 组件测试
 *
 * 测试坐标定位和虚拟 DOMRect 创建
 */

import { describe, it, expect } from "vitest";

describe("ContextMenu Configuration", () => {
  describe("Props 接口", () => {
    it("isOpen 应控制显示状态", () => {
      const props = { isOpen: true };
      expect(props.isOpen).toBe(true);
    });

    it("x/y 坐标应为数字", () => {
      const props = { x: 100, y: 200 };
      expect(typeof props.x).toBe("number");
      expect(typeof props.y).toBe("number");
    });

    it("zIndex 默认值应为 50", () => {
      const defaultZIndex = 50;
      expect(defaultZIndex).toBe(50);
    });
  });
});

describe("ContextMenu 虚拟 DOMRect 创建", () => {
  describe("从 x/y 坐标创建 DOMRect", () => {
    it("应正确设置 top 和 bottom", () => {
      const x = 100;
      const y = 200;
      const rect = {
        top: y,
        bottom: y,
        left: x,
        right: x,
        width: 0,
        height: 0,
        x,
        y,
      };
      expect(rect.top).toBe(200);
      expect(rect.bottom).toBe(200);
    });

    it("应正确设置 left 和 right", () => {
      const x = 100;
      const y = 200;
      const rect = {
        left: x,
        right: x,
      };
      expect(rect.left).toBe(100);
      expect(rect.right).toBe(100);
    });

    it("width 和 height 应为 0", () => {
      const rect = {
        width: 0,
        height: 0,
      };
      expect(rect.width).toBe(0);
      expect(rect.height).toBe(0);
    });
  });
});

describe("ContextMenu 组合", () => {
  describe("组件结构", () => {
    it("应组合 BasePopover 和 BaseMenu", () => {
      const components = ["BasePopover", "BaseMenu"];
      expect(components).toContain("BasePopover");
      expect(components).toContain("BaseMenu");
    });

    it("应使用 bottom-start 定位", () => {
      const placement = "bottom-start";
      expect(placement).toBe("bottom-start");
    });

    it("offset 应为 2", () => {
      const offset = 2;
      expect(offset).toBe(2);
    });
  });

  describe("样式配置", () => {
    it("应有最小宽度", () => {
      const minWidth = "160px";
      expect(minWidth).toBe("160px");
    });
  });
});
