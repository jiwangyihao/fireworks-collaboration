/**
 * useFormattingActions.ts - 格式化操作 Hook
 *
 * 提供统一的文本格式化函数，消除 StaticToolbar 和 CustomFormattingToolbar 的重复代码
 */

import { useCallback, useMemo } from "react";
import type { BlockNoteEditor } from "@blocknote/core";

export interface FormattingActions {
  // 基础样式切换
  toggleBold: () => void;
  toggleItalic: () => void;
  toggleUnderline: () => void;
  toggleStrike: () => void;
  toggleCode: () => void;

  // 链接操作
  createLink: (url: string) => void;
  removeLink: () => void;
  getSelectedLinkUrl: () => string;

  // 插入操作
  insertMath: () => void;

  // 块嵌套操作
  nestBlock: () => void;
  unnestBlock: () => void;
}

/**
 * 提供统一的格式化操作函数
 *
 * @param editor - BlockNote 编辑器实例
 * @returns 格式化操作函数集合
 */
export function useFormattingActions(
  editor: BlockNoteEditor<any, any, any>
): FormattingActions {
  // 基础样式切换
  const toggleBold = useCallback(
    () => editor.toggleStyles({ bold: true }),
    [editor]
  );
  const toggleItalic = useCallback(
    () => editor.toggleStyles({ italic: true }),
    [editor]
  );
  const toggleUnderline = useCallback(
    () => editor.toggleStyles({ underline: true }),
    [editor]
  );
  const toggleStrike = useCallback(
    () => editor.toggleStyles({ strike: true }),
    [editor]
  );
  const toggleCode = useCallback(
    () => editor.toggleStyles({ code: true }),
    [editor]
  );

  // 链接操作
  const createLink = useCallback(
    (url: string) => editor.createLink(url),
    [editor]
  );
  const removeLink = useCallback(
    () => editor.removeStyles({ link: true }),
    [editor]
  );
  const getSelectedLinkUrl = useCallback(
    () => editor.getSelectedLinkUrl() || "",
    [editor]
  );

  // 插入数学公式
  const insertMath = useCallback(() => {
    editor.insertInlineContent([
      {
        type: "inlineMath",
        props: { formula: "" },
      },
    ]);
  }, [editor]);

  // 块嵌套操作
  const nestBlock = useCallback(() => {
    // @ts-ignore - BlockNote 类型定义不完整
    if (editor.canNestBlock ? editor.canNestBlock() : true) {
      // @ts-ignore
      if (editor.nestBlock) {
        // @ts-ignore
        editor.nestBlock();
      }
    }
  }, [editor]);

  const unnestBlock = useCallback(() => {
    // @ts-ignore - BlockNote 类型定义不完整
    if (editor.unnestBlock) {
      // @ts-ignore
      editor.unnestBlock();
    }
  }, [editor]);

  return useMemo(
    () => ({
      toggleBold,
      toggleItalic,
      toggleUnderline,
      toggleStrike,
      toggleCode,
      createLink,
      removeLink,
      getSelectedLinkUrl,
      insertMath,
      nestBlock,
      unnestBlock,
    }),
    [
      toggleBold,
      toggleItalic,
      toggleUnderline,
      toggleStrike,
      toggleCode,
      createLink,
      removeLink,
      getSelectedLinkUrl,
      insertMath,
      nestBlock,
      unnestBlock,
    ]
  );
}

export default useFormattingActions;
