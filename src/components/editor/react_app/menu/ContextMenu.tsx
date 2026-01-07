/**
 * ContextMenu.tsx - Right-click context menu component
 *
 * Positions at x/y coordinates (mouse position).
 */

import React, { type ReactNode, useMemo } from "react";
import { BasePopover } from "./BasePopover";
import { BaseMenu } from "./BaseMenu";

export interface ContextMenuProps {
  isOpen: boolean;
  x: number;
  y: number;
  zIndex?: number;
  children: ReactNode;
  onClose?: () => void;
}

export const ContextMenu: React.FC<ContextMenuProps> = ({
  isOpen,
  x,
  y,
  zIndex = 50,
  children,
  onClose,
}) => {
  // Create a virtual DOMRect from x/y coordinates
  const triggerRect = useMemo(
    (): DOMRect => ({
      top: y,
      bottom: y,
      left: x,
      right: x,
      width: 0,
      height: 0,
      x,
      y,
      toJSON: () => ({}),
    }),
    [x, y]
  );

  return (
    <BasePopover
      isOpen={isOpen}
      triggerRect={triggerRect}
      placement="bottom-start"
      offset={2}
      zIndex={zIndex}
      onClickOutside={onClose}
    >
      <BaseMenu className="min-w-[160px] max-w-xs">{children}</BaseMenu>
    </BasePopover>
  );
};

export default ContextMenu;
