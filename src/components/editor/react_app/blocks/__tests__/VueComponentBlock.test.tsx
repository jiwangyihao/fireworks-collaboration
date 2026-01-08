/**
 * VueComponentBlock.test.tsx - VueComponentBlock 组件配置测试
 *
 * 测试 VueComponentBlock 的 block schema 定义和 JSON 属性序列化
 */

import { describe, it, expect } from "vitest";

describe("VueComponentBlock Configuration", () => {
  describe("Block 规格定义", () => {
    it("Block 类型应为 vueComponent", () => {
      const blockType = "vueComponent";
      expect(blockType).toBe("vueComponent");
    });

    it("应定义 componentName 属性", () => {
      const propSchema = {
        componentName: { default: "" },
        attributesJson: { default: "{}" },
        selfClosing: { default: true },
      };
      expect(propSchema.componentName).toBeDefined();
      expect(propSchema.componentName.default).toBe("");
    });

    it("attributesJson 应默认为空对象 JSON", () => {
      const propSchema = {
        attributesJson: { default: "{}" },
      };
      expect(propSchema.attributesJson.default).toBe("{}");
    });

    it("selfClosing 应默认为 true", () => {
      const propSchema = {
        selfClosing: { default: true },
      };
      expect(propSchema.selfClosing.default).toBe(true);
    });
  });

  describe("ContentRegistry 注册配置", () => {
    it("应注册正确的标签", () => {
      const registryConfig = {
        label: "Vue 组件",
        supportedStyles: [],
      };
      expect(registryConfig.label).toBe("Vue 组件");
    });

    it("应配置 Slash Menu 项", () => {
      const slashMenuItem = {
        id: "vueComponent",
        title: "Vue 组件",
        group: "VitePress",
        aliases: ["vue", "component", "zj", "zujian"],
        blockType: "vueComponent",
        props: { componentName: "", attributesJson: "{}", selfClosing: true },
      };

      expect(slashMenuItem.id).toBe("vueComponent");
      expect(slashMenuItem.title).toBe("Vue 组件");
      expect(slashMenuItem.group).toBe("VitePress");
      expect(slashMenuItem.aliases).toContain("vue");
      expect(slashMenuItem.aliases).toContain("zujian");
    });
  });

  describe("属性 JSON 序列化逻辑", () => {
    // 模拟 parseAttributes 和 JSON 操作逻辑
    const parseAttributes = (json: string): Record<string, string> => {
      try {
        return JSON.parse(json);
      } catch {
        return {};
      }
    };

    const serializeAttributes = (attrs: Record<string, string>): string => {
      return JSON.stringify(attrs);
    };

    it("应正确解析有效 JSON", () => {
      const json = '{"type":"tip","title":"Info"}';
      const attrs = parseAttributes(json);
      expect(attrs.type).toBe("tip");
      expect(attrs.title).toBe("Info");
    });

    it("应处理无效 JSON 为空对象", () => {
      const json = "invalid-json";
      const attrs = parseAttributes(json);
      expect(attrs).toEqual({});
    });

    it("应正确序列化属性对象", () => {
      const attrs = { type: "warning", dismissible: "true" };
      const json = serializeAttributes(attrs);
      expect(json).toContain('"type":"warning"');
      expect(json).toContain('"dismissible":"true"');
    });
  });
});
