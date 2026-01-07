import { getDefaultReactSlashMenuItems } from "@blocknote/react";
import { Icon } from "@iconify/react";

export const getCustomSlashMenuItems = (editor: any) => {
  /**
   * 创建块插入器工厂函数
   * 简化 SlashMenu 项的 onItemClick 定义
   */
  const createBlockInserter = (
    blockType: string,
    props: Record<string, any> = {},
    moveCursor: boolean = false
  ) => {
    return () => {
      editor.insertBlocks(
        [{ type: blockType, props } as any],
        editor.getTextCursorPosition().block,
        "after"
      );
      if (moveCursor) {
        editor.setTextCursorPosition(
          editor.getTextCursorPosition().nextBlock!,
          "end"
        );
      }
    };
  };

  // 1. Filter default items
  const defaultItems = getDefaultReactSlashMenuItems(editor).filter(
    (item) =>
      item.title !== "Code Block" &&
      item.title !== "代码块" &&
      !item.title.startsWith("Heading") &&
      !item.title.startsWith("标题") &&
      !item.title.includes("可折叠") && // Added filter for Chinese "Collapsible"
      item.title !== "Quote" &&
      item.title !== "引用"
  );

  const customItems = [
    // Math Block
    {
      title: "数学公式",
      onItemClick: createBlockInserter("math", { formula: "" }, true),
      aliases: ["math", "formula", "latex", "gs", "gongshi", "shuxue"],
      group: "高级功能",
      icon: <Icon icon="lucide:sigma" />,
      subtext: "插入数学公式块",
    },
    // Mermaid Block
    {
      title: "Mermaid 图表",
      onItemClick: createBlockInserter("mermaid", { code: "" }, true),
      aliases: ["mermaid", "flowchart", "diagram", "mm", "tubiao", "liucheng"],
      group: "高级功能",
      icon: <Icon icon="lucide:network" />,
      subtext: "插入 Mermaid 图表",
    },
    // E2.5: Shiki Code Block
    {
      title: "代码块",
      onItemClick: createBlockInserter("shikiCode", {
        code: "",
        language: "text",
      }),
      aliases: ["code", "pre", "shiki", "daima"],
      group: "基础",
      icon: <Icon icon="lucide:code-2" />,
      subtext: "插入代码块",
    },
    // E2.4: Quote Block
    {
      title: "引用",
      onItemClick: createBlockInserter("quote", {}),
      aliases: ["quote", "blockquote", "yy", "yinyong"],
      group: "基础",
      icon: <Icon icon="lucide:quote" />,
      subtext: "插入引用块",
    },
    // Container: Tip
    {
      title: "提示",
      onItemClick: createBlockInserter("container", { containerType: "tip" }),
      aliases: ["tip", "ts", "hint", "tishi"],
      group: "容器",
      icon: <Icon icon="lucide:lightbulb" />,
      subtext: "插入提示容器",
    },
    // Container: Info
    {
      title: "信息",
      onItemClick: createBlockInserter("container", { containerType: "info" }),
      aliases: ["info", "xx", "xinxi"],
      group: "容器",
      icon: <Icon icon="lucide:info" />,
      subtext: "插入信息容器",
    },
    // Container: Warning
    {
      title: "警告",
      onItemClick: createBlockInserter("container", {
        containerType: "warning",
      }),
      aliases: ["warning", "jg", "jinggao"],
      group: "容器",
      icon: <Icon icon="lucide:triangle-alert" />,
      subtext: "插入警告容器",
    },
    // Container: Danger
    {
      title: "危险",
      onItemClick: createBlockInserter("container", {
        containerType: "danger",
      }),
      aliases: ["danger", "wx", "weixian"],
      group: "容器",
      icon: <Icon icon="lucide:flame" />,
      subtext: "插入危险容器",
    },
    // Container: Details
    {
      title: "折叠详情",
      onItemClick: createBlockInserter("container", {
        containerType: "details",
      }),
      aliases: ["details", "xq", "collapse", "xiangqing", "zhedie"],
      group: "容器",
      icon: <Icon icon="lucide:list-collapse" />,
      subtext: "插入详情折叠块",
    },
    // E2.4: Vue Component Block
    {
      title: "Vue 组件",
      onItemClick: createBlockInserter("vueComponent", {
        componentName: "",
        attributesJson: "{}",
        selfClosing: true,
      }),
      aliases: ["vue", "component", "zj", "zujian"],
      group: "VitePress",
      icon: <Icon icon="mdi:vuejs" />,
      subtext: "插入 Vue 组件标签",
    },
    // E2.4: Include Block
    {
      title: "文件包含",
      onItemClick: createBlockInserter("include", {
        path: "",
        lineRange: "",
        region: "",
      }),
      aliases: ["include", "import", "bh", "baohan", "yinyong"],
      group: "VitePress",
      icon: <Icon icon="mdi:file-import" />,
      subtext: "插入文件包含指令",
    },
  ];

  const finalItems = [...defaultItems, ...customItems];

  // Define group order (using Chinese keys)
  const groupOrder = [
    "标题",
    "基础",
    "媒体",
    "容器",
    "高级功能",
    "VitePress",
    "其他",
  ];

  // Sort items
  return finalItems.sort((a, b) => {
    const groupA = a.group || "其他";
    const groupB = b.group || "其他";

    const indexA = groupOrder.indexOf(groupA);
    const indexB = groupOrder.indexOf(groupB);

    // If both groups are in our order list, compare indices
    if (indexA !== -1 && indexB !== -1) {
      return indexA - indexB;
    }

    // If one is in the list, it comes first
    if (indexA !== -1) return -1;
    if (indexB !== -1) return 1;

    // Fallback: sort alphabetically for unknown groups
    return groupA.localeCompare(groupB);
  });
};
