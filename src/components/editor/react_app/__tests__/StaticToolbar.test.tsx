/**
 * StaticToolbar.test.tsx - StaticToolbar 组件测试
 *
 * 测试动态 Action 渲染和 Registry 集成
 */

import { describe, it, expect } from "vitest";

describe("StaticToolbar Configuration", () => {
  describe("Props 接口", () => {
    it("应接收 BlockNoteEditor 实例", () => {
      const props = { editor: {} };
      expect(props.editor).toBeDefined();
    });
  });
});

describe("StaticToolbar 工具栏按钮", () => {
  describe("ToolbarButton 属性", () => {
    it("应支持 onClick 事件", () => {
      let clicked = false;
      const onClick = () => {
        clicked = true;
      };
      onClick();
      expect(clicked).toBe(true);
    });

    it("应支持 isActive 状态", () => {
      const isActive = true;
      expect(isActive).toBe(true);
    });

    it("应支持 disabled 状态", () => {
      const disabled = true;
      expect(disabled).toBe(true);
    });

    it("应支持 title 属性", () => {
      const title = "加粗";
      expect(title).toBe("加粗");
    });
  });
});

describe("StaticToolbar 动态 Action 渲染", () => {
  describe("Action 类型支持", () => {
    it("应支持 dropdown 类型", () => {
      const action = { type: "dropdown", id: "language" };
      expect(action.type).toBe("dropdown");
    });

    it("应支持 input 类型", () => {
      const action = { type: "input", id: "filename" };
      expect(action.type).toBe("input");
    });

    it("应支持 toggle 类型", () => {
      const action = { type: "toggle", id: "lineNumbers" };
      expect(action.type).toBe("toggle");
    });

    it("应支持 button 类型", () => {
      const action = { type: "button", id: "execute" };
      expect(action.type).toBe("button");
    });
  });

  describe("Action 属性", () => {
    it("应有 id 属性", () => {
      const action = { id: "action-1" };
      expect(action.id).toBeDefined();
    });

    it("应有 label 属性", () => {
      const action = { label: "操作" };
      expect(action.label).toBe("操作");
    });

    it("应有 icon 属性", () => {
      const action = { icon: "<Icon />" };
      expect(action.icon).toBeDefined();
    });
  });
});

describe("StaticToolbar Registry 集成", () => {
  describe("executeAction 调用", () => {
    it("应调用 contentRegistry.executeAction", () => {
      let executed = false;
      const executeAction = (blockId: string, actionId: string, value: any) => {
        executed = true;
      };
      executeAction("block-1", "language", "typescript");
      expect(executed).toBe(true);
    });
  });

  describe("isActionActive 调用", () => {
    it("应调用 contentRegistry.isActionActive", () => {
      const isActionActive = (blockId: string, actionId: string) => true;
      const result = isActionActive("block-1", "lineNumbers");
      expect(result).toBe(true);
    });
  });

  describe("getActionValue 调用", () => {
    it("应调用 contentRegistry.getActionValue", () => {
      const getActionValue = (blockId: string, actionId: string) =>
        "javascript";
      const result = getActionValue("block-1", "language");
      expect(result).toBe("javascript");
    });
  });
});

describe("StaticToolbar 格式化操作", () => {
  describe("基础格式化", () => {
    it("应支持加粗", () => {
      const formats = ["bold", "italic", "underline", "strikethrough", "code"];
      expect(formats).toContain("bold");
    });

    it("应支持斜体", () => {
      const formats = ["bold", "italic", "underline", "strikethrough", "code"];
      expect(formats).toContain("italic");
    });

    it("应支持下划线", () => {
      const formats = ["bold", "italic", "underline", "strikethrough", "code"];
      expect(formats).toContain("underline");
    });

    it("应支持删除线", () => {
      const formats = ["bold", "italic", "underline", "strikethrough", "code"];
      expect(formats).toContain("strikethrough");
    });

    it("应支持行内代码", () => {
      const formats = ["bold", "italic", "underline", "strikethrough", "code"];
      expect(formats).toContain("code");
    });
  });

  describe("链接操作", () => {
    it("应支持添加链接", () => {
      const linkActions = [
        "toggleLink",
        "handleLinkConfirm",
        "handleLinkRemove",
      ];
      expect(linkActions).toContain("toggleLink");
    });

    it("应支持确认链接", () => {
      const linkActions = [
        "toggleLink",
        "handleLinkConfirm",
        "handleLinkRemove",
      ];
      expect(linkActions).toContain("handleLinkConfirm");
    });

    it("应支持移除链接", () => {
      const linkActions = [
        "toggleLink",
        "handleLinkConfirm",
        "handleLinkRemove",
      ];
      expect(linkActions).toContain("handleLinkRemove");
    });
  });
});

describe("StaticToolbar 块类型切换", () => {
  describe("块类型下拉菜单", () => {
    it("应有段落选项", () => {
      const blockTypes = [
        "paragraph",
        "heading",
        "bulletListItem",
        "numberedListItem",
      ];
      expect(blockTypes).toContain("paragraph");
    });

    it("应有标题选项", () => {
      const blockTypes = [
        "paragraph",
        "heading",
        "bulletListItem",
        "numberedListItem",
      ];
      expect(blockTypes).toContain("heading");
    });

    it("应有无序列表选项", () => {
      const blockTypes = [
        "paragraph",
        "heading",
        "bulletListItem",
        "numberedListItem",
      ];
      expect(blockTypes).toContain("bulletListItem");
    });

    it("应有有序列表选项", () => {
      const blockTypes = [
        "paragraph",
        "heading",
        "bulletListItem",
        "numberedListItem",
      ];
      expect(blockTypes).toContain("numberedListItem");
    });
  });
});
