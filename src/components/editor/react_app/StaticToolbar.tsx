/**
 * StaticToolbar.tsx - 静态格式化工具栏 (E2.5)
 *
 * 接收 BlockNote editor 实例作为 prop
 * 始终显示在编辑器上方
 */

import "./StaticToolbar.css";
import type { BlockNoteEditor } from "@blocknote/core";
import React, {
  useState,
  useEffect,
  useMemo,
  memo,
  useCallback,
  useRef,
  type RefObject,
} from "react";
import { createPortal } from "react-dom";
import { Icon } from "@iconify/react";
import { BlockTypeDropdown } from "./BlockTypeDropdown";
import {
  getBlockCapabilities,
  getRegisteredBlockTypes,
  blockRegistry,
  type BlockActionDefinition,
} from "./BlockCapabilities";
import {
  ToolbarDropdown,
  ToolbarInput,
  ToolbarToggle,
} from "./ToolbarControls";

interface StaticToolbarProps {
  editor: BlockNoteEditor<any, any, any>;
}

// 工具栏按钮组件 (Memoized to prevent icon re-render)
export const ToolbarButton = memo(function ToolbarButton({
  onClick,
  isActive = false,
  disabled = false,
  title,
  onMouseDown,
  children,
}: {
  onClick: (e: React.MouseEvent) => void;
  isActive?: boolean;
  disabled?: boolean;
  title: string;
  onMouseDown?: (e: React.MouseEvent) => void;
  children: React.ReactNode;
}) {
  return (
    <button
      onMouseDown={onMouseDown}
      className={`static-toolbar-btn ${isActive ? "active" : ""}`}
      onClick={onClick}
      disabled={disabled}
      title={title}
      type="button"
    >
      <span className="w-4 h-4 flex items-center justify-center">
        {children}
      </span>
    </button>
  );
});

// 链接编辑弹出层
export const LinkPopover = memo(function LinkPopover({
  isOpen,
  currentUrl,
  onConfirm,
  onRemove,
  onClose,
  containerRef,
  portal = false,
}: {
  isOpen: boolean;
  currentUrl: string;
  onConfirm: (url: string) => void;
  onRemove: () => void;
  onClose: () => void;
  containerRef?: RefObject<HTMLElement>;
  portal?: boolean;
}) {
  const [url, setUrl] = useState(currentUrl);
  const inputRef = useRef<HTMLInputElement>(null);
  const [position, setPosition] = useState<{
    top: number;
    left: number;
  } | null>(null);

  useEffect(() => {
    if (isOpen) {
      setUrl(currentUrl);

      // Calculate position if portal is enabled
      if (portal && containerRef?.current) {
        const rect = containerRef.current.getBoundingClientRect();
        setPosition({
          top: rect.bottom + 5, // 5px gap
          left: rect.left + rect.width / 2,
        });
      }

      setTimeout(() => inputRef.current?.focus(), 50);
    }
  }, [isOpen, currentUrl, portal, containerRef]);

  // Update position on scroll/resize if open and specific to portal
  useEffect(() => {
    if (!isOpen || !portal || !containerRef?.current) return;

    const updatePos = () => {
      const rect = containerRef.current!.getBoundingClientRect();
      setPosition({
        top: rect.bottom + 5,
        left: rect.left + rect.width / 2,
      });
    };

    window.addEventListener("scroll", updatePos, true);
    window.addEventListener("resize", updatePos);

    return () => {
      window.removeEventListener("scroll", updatePos, true);
      window.removeEventListener("resize", updatePos);
    };
  }, [isOpen, portal, containerRef]);

  if (!isOpen) return null;

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (url.trim()) {
      onConfirm(url.trim());
    }
  };

  const content = (
    <div
      className="link-popover"
      style={
        portal && position
          ? {
              position: "fixed",
              top: position.top,
              left: position.left,
              transform: "translateX(-50%)",
              marginTop: 0,
              zIndex: 99999,
            }
          : undefined
      }
    >
      <form onSubmit={handleSubmit} className="link-popover-form">
        <input
          ref={inputRef}
          type="url"
          value={url}
          onChange={(e) => setUrl(e.target.value)}
          placeholder="https://example.com"
          className="link-popover-input"
        />
        <button type="submit" className="link-popover-btn confirm" title="确认">
          <Icon icon="lucide:check" />
        </button>
        {currentUrl && (
          <button
            type="button"
            className="link-popover-btn remove"
            onClick={onRemove}
            title="移除链接"
          >
            <Icon icon="lucide:trash-2" />
          </button>
        )}
        <button
          type="button"
          className="link-popover-btn close"
          onClick={onClose}
          title="取消"
        >
          <Icon icon="lucide:x" />
        </button>
      </form>
    </div>
  );

  if (portal) {
    return createPortal(content, document.body);
  }

  return content;
});

// memoized Custom Actions Component to prevent re-rendering when props are stable
export const MemoizedCustomActions = memo(function CustomActions({
  actions,
  targetBlockId,
  hasStandardActions,
  tick,
}: {
  actions: BlockActionDefinition[];
  targetBlockId: string | undefined;
  hasStandardActions: boolean;
  tick?: number;
}) {
  if (!actions || actions.length === 0) return null;
  if (!targetBlockId) return null;

  return (
    <>
      {hasStandardActions && (
        <div className="h-4 w-[1px] bg-gray-200 mx-1 self-center" />
      )}
      {actions.map((action) => {
        switch (action.type) {
          case "dropdown":
            return (
              <ToolbarDropdown
                key={action.id}
                icon={action.icon}
                label={action.label}
                value={
                  blockRegistry.getActionValue(targetBlockId, action.id) || ""
                }
                options={blockRegistry.getActionOptions(
                  targetBlockId,
                  action.id
                )}
                onChange={(val) =>
                  blockRegistry.executeAction(targetBlockId, action.id, val)
                }
                iconOnly={action.iconOnly}
              />
            );

          case "input":
            return (
              <ToolbarInput
                key={action.id}
                icon={action.icon}
                label={action.label}
                value={
                  blockRegistry.getActionValue(targetBlockId, action.id) || ""
                }
                placeholder={action.placeholder}
                width={action.width}
                onChange={(val) =>
                  blockRegistry.executeAction(targetBlockId, action.id, val)
                }
                hideIcon={action.hideIcon}
              />
            );

          case "toggle":
            return (
              <ToolbarToggle
                key={action.id}
                icon={action.icon}
                label={action.label}
                isActive={blockRegistry.isActionActive(
                  targetBlockId,
                  action.id
                )}
                onChange={(val) =>
                  blockRegistry.executeAction(targetBlockId, action.id, val)
                }
              />
            );

          case "button":
          default:
            return (
              <ToolbarButton
                key={action.id}
                onClick={(e) =>
                  blockRegistry.executeAction(targetBlockId, action.id, e)
                }
                onMouseDown={(e: React.MouseEvent) => e.preventDefault()}
                isActive={blockRegistry.isActionActive(
                  targetBlockId,
                  action.id
                )}
                title={action.label}
              >
                {action.icon}
              </ToolbarButton>
            );
        }
      })}
    </>
  );
});

export function StaticToolbar({ editor }: StaticToolbarProps) {
  // 1. Hooks (Top level)
  const [tick, forceUpdate] = useState(0);
  const [activeStyles, setActiveStyles] = useState<Record<string, any>>({});
  const [isLinkActive, setIsLinkActive] = useState(false);
  const [isLinkPopoverOpen, setIsLinkPopoverOpen] = useState(false);
  const [currentLinkUrl, setCurrentLinkUrl] = useState("");

  // 2. Active Inline Check (Top Priority)
  const activeInline = blockRegistry.getActiveInline();

  // 3. Editor Selection Block (Derived)
  let editorSelectionBlock;

  if (activeInline) {
    editorSelectionBlock = {
      id: "__active_inline__",
      type: activeInline.type,
      props: {},
      content: [],
      children: [],
    } as any;
  } else {
    try {
      editorSelectionBlock = editor.getTextCursorPosition()?.block;
    } catch {
      editorSelectionBlock = null;
    }
  }

  // 4. Derived current block info
  let currentBlockType = "paragraph";
  let currentBlockProps: Record<string, any> = {};

  const currentBlock = editorSelectionBlock;
  if (currentBlock) {
    if (currentBlock.type === "heading" && currentBlock.props.level) {
      currentBlockType = `heading-${currentBlock.props.level}`;
    } else {
      currentBlockType = currentBlock.type;
    }
    currentBlockProps = currentBlock.props || {};
  }

  // 5. Capabilities & BlockTypes
  // If activeInline, get capabilities for that type directly
  const capabilities = blockRegistry.get(currentBlockType || "paragraph");
  const blockTypes = useMemo(() => getRegisteredBlockTypes(editor), [editor]);

  // 监听编辑器变化
  useEffect(() => {
    const update = () => {
      // 1. 更新 Active Styles
      try {
        const styles = editor.getActiveStyles();
        setActiveStyles(styles || {});
      } catch (e) {
        setActiveStyles({});
      }

      // 3. 更新 Link 状态 (Standard BlockNote API)
      const linkUrl = editor.getSelectedLinkUrl();
      setIsLinkActive(!!linkUrl);

      forceUpdate((c) => c + 1);
    };

    const unsubscribeSelection = editor.onSelectionChange(update);
    const unsubscribeChange = editor.onChange(update);
    const unsubscribeRegistry = blockRegistry.subscribe(update);

    // Initial check
    update();

    return () => {
      unsubscribeSelection();
      unsubscribeChange();
      unsubscribeRegistry();
    };
  }, [editor]);

  // 格式化操作
  const toggleBold = () => editor.toggleStyles({ bold: true });
  const toggleItalic = () => editor.toggleStyles({ italic: true });
  const toggleUnderline = () => editor.toggleStyles({ underline: true });
  const toggleStrike = () => editor.toggleStyles({ strike: true });
  const toggleCode = () => editor.toggleStyles({ code: true });

  const toggleLink = () => {
    // 打开链接编辑弹出层
    const existingUrl = editor.getSelectedLinkUrl() || "";
    setCurrentLinkUrl(existingUrl);
    setIsLinkPopoverOpen(true);
  };

  const handleLinkConfirm = (url: string) => {
    if (isLinkActive) {
      // 更新现有链接 - 先移除再创建
      editor.removeStyles({ link: true });
    }
    editor.createLink(url);
    setIsLinkPopoverOpen(false);
  };

  const handleLinkRemove = () => {
    editor.removeStyles({ link: true });
    setIsLinkPopoverOpen(false);
  };

  const handleLinkClose = () => {
    setIsLinkPopoverOpen(false);
  };

  const insertMath = () => {
    editor.insertInlineContent([
      {
        type: "inlineMath",
        props: { formula: "" },
      },
    ]);
  };

  const nestBlock = () => {
    // @ts-ignore
    if (editor.canNestBlock ? editor.canNestBlock() : true) {
      // @ts-ignore
      if (editor.nestBlock) {
        // @ts-ignore
        editor.nestBlock();
      }
    }
  };

  const unnestBlock = () => {
    // @ts-ignore
    if (editor.unnestBlock) {
      // @ts-ignore
      editor.unnestBlock();
    }
  };

  // 处理块类型选择
  const handleBlockTypeSelect = (type: string, props?: Record<string, any>) => {
    const target = editorSelectionBlock;
    if (target) {
      const blockToUpdate = editor.getBlock(target.id);
      if (blockToUpdate) {
        try {
          // Normalize heading type
          let finalType = type;
          if (type.startsWith("heading-")) {
            finalType = "heading";
          }

          editor.updateBlock(blockToUpdate, { type: finalType as any, props });
        } catch (e) {
          console.error("Failed to update block type:", e);
        }
      }
    }
  };

  // 检查样式支持
  const isStyleSupported = (style: string) => {
    const styles = capabilities.supportedStyles;
    if (styles === true) return true;
    if (styles === false) return false;
    if (Array.isArray(styles)) return styles.includes(style);
    return false;
  };

  // 检查是否有任何支持的样式
  const hasAnySupportedStyle =
    capabilities.supportedStyles === true ||
    (Array.isArray(capabilities.supportedStyles) &&
      capabilities.supportedStyles.length > 0);

  return (
    <div className="static-toolbar">
      {/* 块类型选择 */}
      <BlockTypeDropdown
        currentType={currentBlockType}
        currentProps={currentBlockProps}
        items={blockTypes}
        onSelect={handleBlockTypeSelect}
      />

      {hasAnySupportedStyle ||
        (capabilities.actions && capabilities.actions.length > 0)}

      {/* 动态渲染支持的格式化按钮 */}
      {hasAnySupportedStyle && (
        <>
          {isStyleSupported("bold") && (
            <ToolbarButton
              onClick={toggleBold}
              isActive={activeStyles.bold === true}
              title="粗体 (Ctrl+B)"
            >
              <Icon icon="lucide:bold" />
            </ToolbarButton>
          )}

          {isStyleSupported("italic") && (
            <ToolbarButton
              onClick={toggleItalic}
              isActive={activeStyles.italic === true}
              title="斜体 (Ctrl+I)"
            >
              <Icon icon="lucide:italic" />
            </ToolbarButton>
          )}

          {isStyleSupported("underline") && (
            <ToolbarButton
              onClick={toggleUnderline}
              isActive={activeStyles.underline === true}
              title="下划线 (Ctrl+U)"
            >
              <Icon icon="lucide:underline" />
            </ToolbarButton>
          )}

          {isStyleSupported("strike") && (
            <ToolbarButton
              onClick={toggleStrike}
              isActive={activeStyles.strike === true}
              title="删除线"
            >
              <Icon icon="lucide:strikethrough" />
            </ToolbarButton>
          )}

          <ToolbarButton
            onClick={nestBlock}
            title="缩进"
            // @ts-ignore
            disabled={
              ![
                "bulletListItem",
                "numberedListItem",
                "checkListItem",
                "quote",
              ].includes(currentBlockType) ||
              (editor.canNestBlock ? !editor.canNestBlock() : false)
            }
          >
            <Icon icon="lucide:indent" />
          </ToolbarButton>

          <ToolbarButton
            onClick={unnestBlock}
            title="减少缩进"
            disabled={
              ![
                "bulletListItem",
                "numberedListItem",
                "checkListItem",
                "quote",
              ].includes(currentBlockType)
            }
          >
            <Icon icon="lucide:outdent" />
          </ToolbarButton>

          {isStyleSupported("link") && (
            <div className="link-button-wrapper">
              <ToolbarButton
                onClick={toggleLink}
                isActive={isLinkActive || isLinkPopoverOpen}
                title="链接"
              >
                <Icon icon="lucide:link" />
              </ToolbarButton>
              <LinkPopover
                isOpen={isLinkPopoverOpen}
                currentUrl={currentLinkUrl}
                onConfirm={handleLinkConfirm}
                onRemove={handleLinkRemove}
                onClose={handleLinkClose}
              />
            </div>
          )}

          <ToolbarButton
            onClick={insertMath}
            title="行内公式"
            isActive={blockRegistry.getActiveInline()?.type === "inlineMath"}
          >
            <Icon icon="lucide:sigma" />
          </ToolbarButton>

          {isStyleSupported("code") && (
            <ToolbarButton
              onClick={toggleCode}
              isActive={activeStyles.code === true}
              title="行内代码"
            >
              <Icon icon="lucide:code-2" />
            </ToolbarButton>
          )}
        </>
      )}

      {/* 渲染自定义操作 (Custom Actions) */}
      <MemoizedCustomActions
        actions={capabilities.actions || []}
        targetBlockId={editorSelectionBlock?.id}
        hasStandardActions={hasAnySupportedStyle}
        tick={tick}
      />
    </div>
  );
}

export default StaticToolbar;
