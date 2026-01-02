/**
 * BlockNote React 编辑器核心组件
 *
 * 这是 React 组件，将通过 veaury 包装后在 Vue 中使用
 */

import "@blocknote/core/fonts/inter.css";
import "@blocknote/mantine/style.css";

import { useCreateBlockNote } from "@blocknote/react";
import { BlockNoteView } from "@blocknote/mantine";
import { customSchema } from "./schema";
import zh from "./locale-zh";
import type { Block } from "@blocknote/core";
import { useEffect } from "react";

interface BlockNoteEditorProps {
  initialContent?: Block<typeof customSchema.blockSchema, any, any>[];
  onChange?: (
    blocks: Block<typeof customSchema.blockSchema, any, any>[]
  ) => void;
  onReady?: (editor: ReturnType<typeof useCreateBlockNote>) => void;
  editable?: boolean;
}

export function BlockNoteEditor({
  initialContent,
  onChange,
  onReady,
  editable = true,
}: BlockNoteEditorProps) {
  // 创建编辑器实例
  const editor = useCreateBlockNote({
    schema: customSchema,
    dictionary: zh,
    initialContent: initialContent as any,
  });

  // 通知父组件编辑器已就绪（仅在挂载时调用一次）
  useEffect(() => {
    if (onReady && editor) {
      onReady(editor);
    }
  }, [onReady, editor]);

  // 监听初始内容变化（用于切换文档）
  useEffect(() => {
    if (initialContent && editor) {
      try {
        editor.replaceBlocks(editor.document, initialContent as any);
      } catch (error) {
        console.error("Failed to replace blocks:", error);
      }
    }
  }, [initialContent, editor]);

  return (
    <BlockNoteView
      editor={editor}
      editable={editable}
      onChange={() => {
        if (onChange) {
          onChange(editor.document);
        }
      }}
      theme="light"
    />
  );
}

export default BlockNoteEditor;
