/**
 * 自定义侧边菜单组件
 *
 * 从 ContentRegistry 动态获取 SideMenu Actions
 * 实现组件自治，无需硬编码各块类型的操作
 */

import { SideMenuExtension } from "@blocknote/core/extensions";
import {
  DragHandleMenu,
  RemoveBlockItem,
  useBlockNoteEditor,
  useComponentsContext,
  useExtensionState,
} from "@blocknote/react";
import { Icon } from "@iconify/react";
import {
  contentRegistry,
  type SideMenuActionDefinition,
} from "./ContentRegistry";
import { ReactNode } from "react";

// 单个 SideMenu Action 菜单项
function SideMenuActionItem({
  action,
  block,
}: {
  action: SideMenuActionDefinition;
  block: any;
}) {
  const editor = useBlockNoteEditor();
  const Components = useComponentsContext()!;

  const isActive = action.isActive ? action.isActive(block) : false;

  const handleClick = () => {
    if (isActive) return;
    action.execute(block, editor);
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
        <Icon icon={action.icon} width={16} height={16} />
        <span style={{ flex: 1 }}>{action.label}</span>
        {isActive && <Icon icon="lucide:check" width={14} height={14} />}
      </span>
    </Components.Generic.Menu.Item>
  );
}

// 动态 SideMenu Actions 容器
function DynamicSideMenuActions() {
  const Components = useComponentsContext()!;

  const block = useExtensionState(SideMenuExtension, {
    selector: (state) => state?.block,
  });

  if (!block) return null;

  const actions = contentRegistry.getSideMenuActions(block.type);
  if (!actions.length) return null;

  return (
    <>
      <Components.Generic.Menu.Divider />
      {actions.map((action) => (
        <SideMenuActionItem key={action.id} action={action} block={block} />
      ))}
    </>
  );
}

// 自定义 Drag Handle 菜单
export function CustomDragHandleMenu() {
  return (
    <DragHandleMenu>
      <RemoveBlockItem>删除</RemoveBlockItem>
      <DynamicSideMenuActions />
    </DragHandleMenu>
  );
}

export default CustomDragHandleMenu;
