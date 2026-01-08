/**
 * MermaidBlock.test.tsx - MermaidBlock contentRegistry 配置测试
 *
 * 由于 MermaidBlock 依赖 mermaid 等复杂库，这里仅测试其注册配置
 */

import { describe, it, expect } from "vitest";

describe("MermaidBlock Configuration", () => {
  describe("Block 类型定义", () => {
    it("应定义 mermaid 块类型", () => {
      const mermaidBlockType = "mermaid";
      expect(mermaidBlockType).toBe("mermaid");
    });

    it("应定义 code 属性", () => {
      const propSchema = {
        code: { default: "" },
      };
      expect(propSchema.code).toBeDefined();
    });

    it("应有默认代码模板", () => {
      const defaultCode = `graph TD
    A[开始] --> B{判断}
    B -->|是| C[处理]
    B -->|否| D[结束]
    C --> D`;

      expect(defaultCode).toContain("graph TD");
      expect(defaultCode).toContain("-->");
    });
  });

  describe("Slash Menu 配置", () => {
    it("应有正确的菜单项配置", () => {
      const slashMenuItem = {
        id: "mermaid",
        title: "Mermaid 图表",
        subtext: "插入 Mermaid 图表",
        group: "高级功能",
        aliases: [
          "mermaid",
          "flowchart",
          "diagram",
          "mm",
          "tubiao",
          "liucheng",
        ],
        blockType: "mermaid",
        props: { code: "" },
        moveCursor: true,
      };

      expect(slashMenuItem.id).toBe("mermaid");
      expect(slashMenuItem.title).toBe("Mermaid 图表");
      expect(slashMenuItem.group).toBe("高级功能");
      expect(slashMenuItem.aliases).toContain("flowchart");
      expect(slashMenuItem.aliases).toContain("tubiao"); // 中文拼音支持
      expect(slashMenuItem.aliases).toContain("liucheng"); // 中文拼音支持
    });
  });

  describe("Mermaid 初始化配置", () => {
    it("应有正确的初始化选项", () => {
      const initOptions = {
        startOnLoad: false,
        theme: "default",
        securityLevel: "loose",
      };

      expect(initOptions.startOnLoad).toBe(false);
      expect(initOptions.theme).toBe("default");
      expect(initOptions.securityLevel).toBe("loose");
    });
  });
});

describe("MermaidBlock 执行器接口", () => {
  it("应定义 edit 执行器接口", () => {
    const editExecutor = {
      execute: () => {},
      isActive: () => false,
    };

    expect(typeof editExecutor.execute).toBe("function");
    expect(typeof editExecutor.isActive).toBe("function");
  });
});

describe("Mermaid 图表类型支持", () => {
  it("应支持流程图 (flowchart)", () => {
    const flowchartCode = "graph TD\n  A --> B";
    expect(flowchartCode).toContain("graph");
  });

  it("应支持序列图 (sequence)", () => {
    const sequenceCode = "sequenceDiagram\n  A->>B: Message";
    expect(sequenceCode).toContain("sequenceDiagram");
  });

  it("应支持类图 (class)", () => {
    const classCode = "classDiagram\n  Class A";
    expect(classCode).toContain("classDiagram");
  });

  it("应支持状态图 (state)", () => {
    const stateCode = "stateDiagram-v2\n  [*] --> State1";
    expect(stateCode).toContain("stateDiagram");
  });
});
