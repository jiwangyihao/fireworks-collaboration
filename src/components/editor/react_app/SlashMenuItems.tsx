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
    // E2.5: Shiki Code Block
    {
      title: "代码块 (Pro)",
      onItemClick: () => {
        editor.insertBlocks(
          [
            {
              type: "shikiCode",
              props: { code: "", language: "text" },
            },
          ],
          editor.getTextCursorPosition().block,
          "after"
        );
      },
      aliases: ["code", "pre", "shiki", "daima"],
      group: "Advanced",
      icon: <Icon icon="lucide:code-2" />,
      subtext: "插入支持高亮和行号的代码块",
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
    // E2.4: Quote Block
    {
      title: "引用",
      onItemClick: () => {
        editor.insertBlocks(
          [{ type: "quote", props: {} }],
          editor.getTextCursorPosition().block,
          "after"
        );
      },
      aliases: ["quote", "blockquote", "yy", "yinyong"],
      group: "Basic blocks",
      icon: <Icon icon="lucide:quote" />,
      subtext: "插入引用块",
    },
    // E2.4: Vue Component Block
    {
      title: "Vue 组件",
      onItemClick: () => {
        editor.insertBlocks(
          [
            {
              type: "vueComponent",
              props: {
                componentName: "",
                attributesJson: "{}",
                selfClosing: true,
              },
            },
          ],
          editor.getTextCursorPosition().block,
          "after"
        );
      },
      aliases: ["vue", "component", "zj", "zujian"],
      group: "VitePress",
      icon: <Icon icon="mdi:vuejs" />,
      subtext: "插入 Vue 组件标签",
    },
    // E2.4: Include Block
    {
      title: "文件包含",
      onItemClick: () => {
        editor.insertBlocks(
          [
            {
              type: "include",
              props: { path: "", lineRange: "", region: "" },
            },
          ],
          editor.getTextCursorPosition().block,
          "after"
        );
      },
      aliases: ["include", "import", "bh", "baohan", "yinyong"],
      group: "VitePress",
      icon: <Icon icon="mdi:file-import" />,
      subtext: "插入文件包含指令",
    },
  ];

  return [...defaultItems, ...customItems];
};
