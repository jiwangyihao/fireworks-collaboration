/**
 * SlashMenuItems.tsx - 自定义 SlashMenu 项
 *
 * 从 ContentRegistry 动态获取已注册的 SlashMenu 项
 * 替代原先的硬编码定义，实现组件自治
 */

import { getDefaultReactSlashMenuItems } from "@blocknote/react";
import { contentRegistry } from "./ContentRegistry";

export const getCustomSlashMenuItems = (editor: any) => {
  /**
   * 创建块插入器工厂函数
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

  // 1. Filter default items (remove those we've re-implemented)
  const defaultItems = getDefaultReactSlashMenuItems(editor).filter(
    (item) =>
      item.title !== "Code Block" &&
      item.title !== "代码块" &&
      !item.title.startsWith("Heading") &&
      !item.title.startsWith("标题") &&
      !item.title.includes("可折叠") &&
      item.title !== "Quote" &&
      item.title !== "引用"
  );

  // 2. 从 Registry 获取自定义项
  const registeredItems = contentRegistry.getSlashMenuItems();

  // 3. 转换为 SlashMenu 格式
  const customItems = registeredItems.map((item) => ({
    title: item.title,
    subtext: item.subtext,
    icon: item.icon,
    group: item.group,
    aliases: item.aliases,
    onItemClick: createBlockInserter(
      item.blockType,
      item.props || {},
      item.moveCursor || false
    ),
  }));

  const finalItems = [...defaultItems, ...customItems];

  // Define group order
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

    if (indexA !== -1 && indexB !== -1) {
      return indexA - indexB;
    }

    if (indexA !== -1) return -1;
    if (indexB !== -1) return 1;

    return groupA.localeCompare(groupB);
  });
};
