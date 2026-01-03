import { getDefaultReactSlashMenuItems } from "@blocknote/react";
import { Icon } from "@iconify/react";

export const getCustomSlashMenuItems = (editor: any) => {
  const defaultItems = getDefaultReactSlashMenuItems(editor);

  const customItems = [
    // Math Block
    {
      title: "数学公式",
      onItemClick: () => {
        editor.insertBlocks(
          [{ type: "math", props: { formula: "" } } as any],
          editor.getTextCursorPosition().block,
          "after"
        );
        editor.setTextCursorPosition(
          editor.getTextCursorPosition().nextBlock!,
          "end"
        );
      },
      aliases: ["math", "formula", "latex", "gs", "gongshi", "shuxue"],
      group: "Advanced",
      icon: <Icon icon="lucide:sigma" />,
      subtext: "插入数学公式块",
    },
    // Mermaid Block
    {
      title: "Mermaid 图表",
      onItemClick: () => {
        editor.insertBlocks(
          [{ type: "mermaid", props: { code: "" } }],
          editor.getTextCursorPosition().block,
          "after"
        );
        editor.setTextCursorPosition(
          editor.getTextCursorPosition().nextBlock!,
          "end"
        );
      },
      aliases: ["mermaid", "flowchart", "diagram", "mm", "tubiao", "liucheng"],
      group: "Advanced",
      icon: <Icon icon="lucide:network" />,
      subtext: "插入 Mermaid 图表",
    },
    // Container: Tip
    {
      title: "提示",
      onItemClick: () => {
        editor.insertBlocks(
          [{ type: "container", props: { containerType: "tip" } }],
          editor.getTextCursorPosition().block,
          "after"
        );
      },
      aliases: ["tip", "ts", "hint", "tishi"],
      group: "Containers",
      icon: <Icon icon="lucide:lightbulb" />,
      subtext: "插入提示容器",
    },
    // Container: Info
    {
      title: "信息",
      onItemClick: () => {
        editor.insertBlocks(
          [{ type: "container", props: { containerType: "info" } }],
          editor.getTextCursorPosition().block,
          "after"
        );
      },
      aliases: ["info", "xx", "xinxi"],
      group: "Containers",
      icon: <Icon icon="lucide:info" />,
      subtext: "插入信息容器",
    },
    // Container: Warning
    {
      title: "警告",
      onItemClick: () => {
        editor.insertBlocks(
          [{ type: "container", props: { containerType: "warning" } }],
          editor.getTextCursorPosition().block,
          "after"
        );
      },
      aliases: ["warning", "jg", "jinggao"],
      group: "Containers",
      icon: <Icon icon="lucide:triangle-alert" />,
      subtext: "插入警告容器",
    },
    // Container: Danger
    {
      title: "危险",
      onItemClick: () => {
        editor.insertBlocks(
          [{ type: "container", props: { containerType: "danger" } }],
          editor.getTextCursorPosition().block,
          "after"
        );
      },
      aliases: ["danger", "wx", "weixian"],
      group: "Containers",
      icon: <Icon icon="lucide:flame" />,
      subtext: "插入危险容器",
    },
    // Container: Details
    {
      title: "折叠详情",
      onItemClick: () => {
        editor.insertBlocks(
          [{ type: "container", props: { containerType: "details" } }],
          editor.getTextCursorPosition().block,
          "after"
        );
      },
      aliases: ["details", "xq", "collapse", "xiangqing", "zhedie"],
      group: "Containers",
      icon: <Icon icon="lucide:list-collapse" />,
      subtext: "插入详情折叠块",
    },
  ];

  return [...defaultItems, ...customItems];
};
