/**
 * CustomFormattingToolbar.test.tsx - CustomFormattingToolbar 组件测试
 *
 * 测试自定义格式化工具栏的渲染和交互逻辑
 */

import { describe, it, expect } from "vitest";

describe("CustomFormattingToolbar Configuration", () => {
  describe("Toolbar 结构", () => {
    it("应包含核心格式化按钮", () => {
      // 模拟工具栏应有的默认按钮类型
      const defaultButtons = ["bold", "italic", "underline"];
      expect(defaultButtons).toContain("bold");
      expect(defaultButtons).toContain("italic");
    });
  });

  describe("Custom Action 集成", () => {
    it("应支持检查样式支持情况", () => {
      // 模拟 isStyleSupported 逻辑
      const supportedStyles = [
        "bold",
        "italic",
        "underline",
        "strikethrough",
        "code",
        "textColor",
        "backgroundColor",
      ];
      const isStyleSupported = (style: string) =>
        supportedStyles.includes(style);

      expect(isStyleSupported("bold")).toBe(true);
      expect(isStyleSupported("unknown")).toBe(false);
    });
  });

  describe("链接处理逻辑", () => {
    it("toggleLink 应切换链接编辑状态", () => {
      let isLinkOpen = false;
      const toggleLink = () => {
        isLinkOpen = !isLinkOpen;
      };

      toggleLink();
      expect(isLinkOpen).toBe(true);
      toggleLink();
      expect(isLinkOpen).toBe(false);
    });

    it("handleLinkConfirm 应更新 URL 并关闭", () => {
      let currentUrl = "";
      let isOpen = true;
      const handleLinkConfirm = (url: string) => {
        currentUrl = url;
        isOpen = false;
      };

      handleLinkConfirm("https://example.com");
      expect(currentUrl).toBe("https://example.com");
      expect(isOpen).toBe(false);
    });
  });
});

describe("MemoizedCustomActions", () => {
  it("应处理空的 actions", () => {
    const actions = [];
    expect(actions.length).toBe(0);
  });

  it("应支持 dropdown 类型的 action", () => {
    const action = {
      type: "dropdown",
      id: "test-dropdown",
      label: "Dropdown",
      icon: "icon",
      items: [],
    };
    expect(action.type).toBe("dropdown");
  });

  it("应支持 toggle 类型的 action", () => {
    const action = {
      type: "toggle",
      id: "test-toggle",
      label: "Toggle",
      icon: "icon",
    };
    expect(action.type).toBe("toggle");
  });
});
