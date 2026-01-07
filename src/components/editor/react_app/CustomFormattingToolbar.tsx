import type { BlockNoteEditor } from "@blocknote/core";
import { useMemo, useState, useEffect, useRef } from "react";
import { Icon } from "@iconify/react";
import {
  FormattingToolbar,
  BlockTypeSelect,
  FileCaptionButton,
  FileReplaceButton,
} from "@blocknote/react";
import {
  ToolbarButton,
  LinkPopover,
  MemoizedCustomActions,
} from "./StaticToolbar";
import { getRegisteredBlockTypes, blockRegistry } from "./BlockCapabilities";
import { useFormattingActions } from "./hooks";

interface CustomFormattingToolbarProps {
  editor: BlockNoteEditor<any, any, any>;
}

export function CustomFormattingToolbar({
  editor,
}: CustomFormattingToolbarProps) {
  // States similar to StaticToolbar
  const [tick, forceUpdate] = useState(0);
  const [activeStyles, setActiveStyles] = useState<Record<string, any>>({});
  const [isLinkActive, setIsLinkActive] = useState(false);
  const [isLinkPopoverOpen, setIsLinkPopoverOpen] = useState(false);
  const [currentLinkUrl, setCurrentLinkUrl] = useState("");
  const [currentBlockType, setCurrentBlockType] = useState("paragraph");
  const [editorSelectionBlock, setEditorSelectionBlock] = useState<any>(null);

  const linkWrapperRef = useRef<HTMLDivElement>(null);

  // Update logic
  useEffect(() => {
    const update = () => {
      // 1. Update Active Styles
      try {
        const styles = editor.getActiveStyles();
        setActiveStyles(styles || {});
      } catch (e) {
        setActiveStyles({});
      }

      // 2. Update Link Status
      const linkUrl = editor.getSelectedLinkUrl();
      setIsLinkActive(!!linkUrl);

      // 3. Update Current Block Info
      const block = editor.getTextCursorPosition()?.block;
      if (block) {
        setEditorSelectionBlock(block);
        if (block.type === "heading" && block.props.level) {
          setCurrentBlockType(`heading-${block.props.level}`);
        } else {
          setCurrentBlockType(block.type);
        }
      } else {
        setEditorSelectionBlock(null);
        setCurrentBlockType("paragraph"); // Fallback
      }

      forceUpdate((c) => c + 1);
    };

    const unsubscribeSelection = editor.onSelectionChange(update);
    const unsubscribeChange = editor.onChange(update);
    const unsubscribeRegistry = blockRegistry.subscribe(update);

    update();
    return () => {
      unsubscribeSelection();
      unsubscribeChange();
      unsubscribeRegistry();
    };
  }, [editor]);

  // Capabilities
  const capabilities = blockRegistry.get(currentBlockType);

  // Actions (from hook)
  const {
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
  } = useFormattingActions(editor);

  const toggleLink = () => {
    const existingUrl = getSelectedLinkUrl();
    setCurrentLinkUrl(existingUrl);
    setIsLinkPopoverOpen(true);
  };

  const handleLinkConfirm = (url: string) => {
    if (isLinkActive) removeLink();
    createLink(url);
    setIsLinkPopoverOpen(false);
  };

  const handleLinkRemove = () => {
    removeLink();
    setIsLinkPopoverOpen(false);
  };

  // Check style support
  const isStyleSupported = (style: string) => {
    const styles = capabilities.supportedStyles;
    if (styles === true) return true;
    if (styles === false) return false;
    if (Array.isArray(styles)) return styles.includes(style);
    return false;
  };

  const hasAnySupportedStyle =
    capabilities.supportedStyles === true ||
    (Array.isArray(capabilities.supportedStyles) &&
      capabilities.supportedStyles.length > 0);

  return (
    <FormattingToolbar>
      {/* Block Type Select - uses Iconify via registry */}
      <BlockTypeSelect
        key={"blockTypeSelect"}
        items={getRegisteredBlockTypes(editor).map((item) => ({
          name: item.label,
          type: item.value.startsWith("heading-") ? "heading" : item.value,
          props: item.props,
          icon: () => <>{item.icon}</>,
        }))}
      />

      <FileCaptionButton key={"fileCaptionButton"} />
      <FileReplaceButton key={"replaceFileButton"} />

      {/* Standard Styles */}
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
        </>
      )}

      {/* Nesting (Always show but disabled if not allowed) */}
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

      {/* More Standard Styles */}
      {hasAnySupportedStyle && (
        <>
          {isStyleSupported("link") && (
            <div className="link-button-wrapper relative" ref={linkWrapperRef}>
              <ToolbarButton
                onClick={toggleLink}
                isActive={isLinkActive || isLinkPopoverOpen}
                title="链接"
              >
                <Icon icon="lucide:link" />
              </ToolbarButton>
              {/* Use portal to escape floating toolbar clipping */}
              <LinkPopover
                isOpen={isLinkPopoverOpen}
                currentUrl={currentLinkUrl}
                onConfirm={handleLinkConfirm}
                onRemove={handleLinkRemove}
                onClose={() => setIsLinkPopoverOpen(false)}
                // @ts-ignore
                containerRef={linkWrapperRef}
                portal={true}
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

      {/* Custom Actions */}
      <MemoizedCustomActions
        actions={capabilities.actions || []}
        targetBlockId={editorSelectionBlock?.id}
        hasStandardActions={hasAnySupportedStyle}
        tick={tick}
      />
    </FormattingToolbar>
  );
}
