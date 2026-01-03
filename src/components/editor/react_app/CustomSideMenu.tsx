/**
 * 自定义侧边菜单组件
 *
 * 扩展 BlockNote 的原生侧边菜单，为 Container 块添加类型切换功能
 */

import { SideMenuExtension } from "@blocknote/core/extensions";
import {
  DragHandleMenu,
  RemoveBlockItem,
  BlockColorsItem,
  useBlockNoteEditor,
  useComponentsContext,
  useExtensionState,
} from "@blocknote/react";
import { Icon } from "@iconify/react";
import { containerTypes, ContainerTypeKey } from "./blocks/ContainerBlock";
import { ReactNode } from "react";

// 容器类型切换菜单项组件
function ContainerTypeItem({
  containerType,
  children,
}: {
  containerType: ContainerTypeKey;
  children: ReactNode;
}) {
  const editor = useBlockNoteEditor();
  const Components = useComponentsContext()!;

  const block = useExtensionState(SideMenuExtension, {
    selector: (state) => state?.block,
  });

  if (!block || block.type !== "container") {
    return null;
  }

  const currentType = (block.props as any).containerType as ContainerTypeKey;
  const isActive = currentType === containerType;

  const handleClick = () => {
    if (isActive) return;

    const currentContent = (block.content || []) as any[];
    const currentConfig = containerTypes[currentType];

    // 检查是否使用默认标题
    let updateContent = undefined;

    if (currentContent && currentContent.length > 0) {
      const firstNode = currentContent[0];
      if (firstNode.type === "text") {
        const currentText = firstNode.text;
        const label = currentConfig.label;

        if (currentText && currentText.startsWith(label)) {
          const remainder = currentText.slice(label.length);
          if (remainder.length === 0 || /^\s/.test(remainder)) {
            const newLabel = containerTypes[containerType].label;
            const newText = newLabel + remainder;

            updateContent = [
              { ...firstNode, text: newText },
              ...currentContent.slice(1),
            ];
          }
        }
      }
    }

    editor.updateBlock(block, {
      props: { containerType },
      ...(updateContent ? { content: updateContent as any } : {}),
    });
  };

  return (
    <Components.Generic.Menu.Item onClick={handleClick}>
      <span
        style={{
          display: "flex",
          alignItems: "center",
          gap: "8px",
          width: "100%",
        }}
      >
        <Icon
          icon={containerTypes[containerType].icon}
          width={16}
          height={16}
        />
        <span style={{ flex: 1 }}>{children}</span>
        {isActive && <Icon icon="lucide:check" width={14} height={14} />}
      </span>
    </Components.Generic.Menu.Item>
  );
}

// 容器类型分隔器 - 检查是否是 container 块
function ContainerDivider() {
  const Components = useComponentsContext()!;

  const block = useExtensionState(SideMenuExtension, {
    selector: (state) => state?.block,
  });

  if (!block || block.type !== "container") {
    return null;
  }

  return <Components.Generic.Menu.Divider />;
}

// 自定义 Drag Handle 菜单
export function CustomDragHandleMenu() {
  return (
    <DragHandleMenu>
      <RemoveBlockItem>删除</RemoveBlockItem>
      <BlockColorsItem>颜色</BlockColorsItem>
      <ContainerDivider />
      {Object.entries(containerTypes).map(([key, config]) => (
        <ContainerTypeItem key={key} containerType={key as ContainerTypeKey}>
          {config.label}
        </ContainerTypeItem>
      ))}
    </DragHandleMenu>
  );
}

export default CustomDragHandleMenu;
