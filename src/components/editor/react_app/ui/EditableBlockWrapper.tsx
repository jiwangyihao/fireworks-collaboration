/**
 * EditableBlockWrapper.tsx - 可编辑块通用包装组件
 *
 * 提供统一的编辑/预览双模式 UI，包括：
 * - 键盘快捷键处理 (Escape 取消, Ctrl+Enter 保存)
 * - 底部操作栏 (取消/确定按钮)
 * - 一致的边框样式
 */

import React, { useCallback, type ReactNode } from "react";

export interface EditableBlockWrapperProps {
  /** 是否处于编辑模式 */
  isEditing: boolean;
  /** 保存回调 */
  onSave: () => void;
  /** 取消回调 */
  onCancel: () => void;
  /** 进入编辑模式回调 */
  onEnterEdit?: () => void;
  /** 编辑模式下的内容 */
  editingContent: ReactNode;
  /** 预览模式下的内容 */
  previewContent: ReactNode;
  /** 编辑模式下的标题栏（可选） */
  editingHeader?: ReactNode;
  /** 容器类名 */
  className?: string;
  /** 边框颜色类（默认 border-base-300） */
  borderColor?: string;
  /** 是否禁用保存按钮 */
  saveDisabled?: boolean;
  /** 保存按钮文本（默认"确定"） */
  saveLabel?: string;
  /** 取消按钮文本（默认"取消"） */
  cancelLabel?: string;
}

/**
 * 可编辑块通用包装组件
 *
 * 使用示例：
 * ```tsx
 * <EditableBlockWrapper
 *   isEditing={isEditing}
 *   onSave={handleSave}
 *   onCancel={handleCancel}
 *   onEnterEdit={() => setIsEditing(true)}
 *   editingContent={<CodeMirror ... />}
 *   previewContent={<div>Preview</div>}
 * />
 * ```
 */
export const EditableBlockWrapper: React.FC<EditableBlockWrapperProps> = ({
  isEditing,
  onSave,
  onCancel,
  onEnterEdit,
  editingContent,
  previewContent,
  editingHeader,
  className = "",
  borderColor = "border-base-300",
  saveDisabled = false,
  saveLabel = "确定",
  cancelLabel = "取消",
}) => {
  // 键盘快捷键处理
  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        onCancel();
      } else if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) {
        e.preventDefault();
        if (!saveDisabled) {
          onSave();
        }
      }
    },
    [onCancel, onSave, saveDisabled]
  );

  // 编辑模式
  if (isEditing) {
    return (
      <div
        className={`w-full border ${borderColor} rounded-lg overflow-hidden bg-base-100 ${className}`}
        onKeyDown={handleKeyDown}
      >
        {/* 可选的标题栏 */}
        {editingHeader}

        {/* 编辑内容区 */}
        <div className="w-full">{editingContent}</div>

        {/* 底部操作栏 */}
        <div className="flex justify-end items-center gap-2 p-2 bg-base-200 border-t border-base-300/50">
          <button className="btn btn-ghost btn-xs" onClick={onCancel}>
            {cancelLabel}
          </button>
          <button
            className="btn btn-primary btn-xs"
            onClick={onSave}
            disabled={saveDisabled}
          >
            {saveLabel}
            <kbd className="kbd kbd-xs ml-1 bg-primary-content/20 text-primary-content border-none">
              Ctrl
            </kbd>
            <kbd className="kbd kbd-xs bg-primary-content/20 text-primary-content border-none">
              ↵
            </kbd>
          </button>
        </div>
      </div>
    );
  }

  // 预览模式
  return (
    <div
      className={`w-full cursor-pointer hover:bg-base-200 transition-colors ${className}`}
      onClick={onEnterEdit}
    >
      {previewContent}
    </div>
  );
};

export default EditableBlockWrapper;
