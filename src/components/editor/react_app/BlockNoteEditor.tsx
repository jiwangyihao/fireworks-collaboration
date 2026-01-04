/**
 * BlockNote React 编辑器核心组件
 *
 * 这是 React 组件，将通过 veaury 包装后在 Vue 中使用
 */

import "@blocknote/core/fonts/inter.css";
import "@blocknote/mantine/style.css";

import {
  useCreateBlockNote,
  SideMenu,
  SideMenuController,
  DragHandleButton,
  AddBlockButton,
  SuggestionMenuController,
} from "@blocknote/react";
import { BlockNoteView } from "@blocknote/mantine";
import { customSchema } from "./schema";
import zh from "./locale-zh";
import type { Block } from "@blocknote/core";
import { useEffect } from "react";
import { CustomDragHandleMenu } from "./CustomSideMenu";
import { BlockInputRules } from "./extensions/InputRules";
import { QuoteKeyboardShortcuts } from "./extensions/KeyboardShortcuts";
import { getCustomSlashMenuItems } from "./SlashMenuItems";
import { EditorContext, updateGlobalContext } from "./EditorContext";

interface BlockNoteEditorProps {
  initialContent?: Block<typeof customSchema.blockSchema, any, any>[];
  onChange?: (
    blocks: Block<typeof customSchema.blockSchema, any, any>[]
  ) => void;
  onReady?: (editor: ReturnType<typeof useCreateBlockNote>) => void;
  editable?: boolean;
  filePath?: string;
  projectRoot?: string;
  devServerPort?: number;
  devServerUrl?: string;
}

export function BlockNoteEditor({
  initialContent,
  onChange,
  onReady,
  editable = true,
  filePath,
  projectRoot,
  devServerPort,
  devServerUrl,
}: BlockNoteEditorProps) {
  // 创建编辑器实例
  const editor = useCreateBlockNote({
    schema: customSchema,
    dictionary: zh,
    initialContent: initialContent as any,
    // 注入自定义 Input Rules 和 Keyboard Shortcuts (Tiptap Extension)
    _tiptapOptions: {
      extensions: [BlockInputRules, QuoteKeyboardShortcuts],
    },
  });

  // 通知父组件编辑器已就绪（仅在挂载时调用一次）
  useEffect(() => {
    if (onReady && editor) {
      onReady(editor);
    }
  }, [onReady, editor]);

  // 监听初始内容变化（用于切换文档）
  useEffect(() => {
    // 只有当 initialContent 确实变化且不为空时才替换
    // 注意：这里需要深比较或依赖上层控制，避免重复渲染
    // 暂时简单处理：如果新内容与当前内容不同，才替换
    if (initialContent && editor) {
      try {
        // 注意：全量替换会导致光标丢失，通常只在切换文档时进行
        editor.replaceBlocks(editor.document, initialContent as any);
      } catch (error) {
        console.error("Failed to replace blocks:", error);
      }
    }
  }, [initialContent, editor]); // 这里其实 initialContent 引用变化就会触发

  // 同步 Context 到全局 Store (解决 Context 更新在 Block 中由于缓存不生效的问题)
  useEffect(() => {
    updateGlobalContext({
      filePath,
      projectRoot,
      devServerPort,
      devServerUrl,
    });
  }, [filePath, projectRoot, devServerPort, devServerUrl]);

  return (
    <EditorContext.Provider
      value={{ filePath, projectRoot, devServerPort, devServerUrl }}
    >
      <BlockNoteView
        editor={editor}
        editable={editable}
        onChange={() => {
          if (onChange) {
            onChange(editor.document);
          }
        }}
        theme="light"
        // 自定义 Side Menu
        sideMenu={false}
        slashMenu={false}
      >
        {/* 自定义侧边菜单，包含容器类型切换 */}
        <SideMenuController
          sideMenu={(props) => (
            <SideMenu
              {...props}
              dragHandleMenu={(props) => <CustomDragHandleMenu {...props} />}
            />
          )}
        />

        {/* 自定义 Slash Menu */}
        <SuggestionMenuController
          triggerCharacter={"/"}
          getItems={async (query) =>
            getCustomSlashMenuItems(editor).filter(
              (item) =>
                item.title.toLowerCase().includes(query.toLowerCase()) ||
                item.aliases?.some((alias) =>
                  alias.toLowerCase().includes(query.toLowerCase())
                )
            )
          }
        />
      </BlockNoteView>
    </EditorContext.Provider>
  );
}

export default BlockNoteEditor;
