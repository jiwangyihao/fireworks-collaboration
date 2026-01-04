import { Extension } from "@tiptap/core";

export const QuoteKeyboardShortcuts = Extension.create({
  name: "quoteKeyboardShortcuts",

  addKeyboardShortcuts() {
    return {
      Enter: ({ editor }) => {
        const { state } = editor;
        const { selection } = state;
        const { $from } = selection;

        // 查找当前深度的 quote 节点
        let quoteNode: any = null;
        let quoteDepth = -1;

        // 向上查找 quote 节点
        for (let d = $from.depth; d > 0; d--) {
          const node = $from.node(d);
          if (node.type.name === "quote") {
            quoteNode = node;
            quoteDepth = d;
            break;
          }
        }

        if (quoteNode) {
          const parent = $from.parent;
          const isEmpty = parent.textContent.trim().length === 0;

          if (isEmpty) {
            // 空行：跳出引用
            return (
              editor.commands.lift("quote") ||
              editor.commands.setNode("paragraph")
            );
          } else {
            // 有内容：分割并保留 groupId
            // 获取当前 quote 的 groupId
            const currentGroupId = quoteNode.attrs.groupId || "default";

            // 使用 splitBlock，然后更新新块的 groupId
            const splitSuccess = editor.commands.splitBlock();

            if (splitSuccess) {
              // 分割后光标在新块中，更新当前块的 groupId 使其与原块一致
              editor.commands.command(({ tr, state }) => {
                const { $from: newFrom } = state.selection;

                // 查找新位置的 quote 节点
                for (let d = newFrom.depth; d > 0; d--) {
                  const node = newFrom.node(d);
                  if (node.type.name === "quote") {
                    const pos = newFrom.before(d);
                    // 更新节点属性，保留 groupId
                    tr.setNodeMarkup(pos, undefined, {
                      ...node.attrs,
                      groupId: currentGroupId,
                    });
                    break;
                  }
                }
                return true;
              });
            }

            return splitSuccess;
          }
        }

        // 非 QuoteBlock，使用默认行为
        return false;
      },

      // 处理 Shift+Enter：强制软换行
      "Shift-Enter": ({ editor }) => {
        return (editor.commands as any).setHardBreak();
      },
    };
  },
});
