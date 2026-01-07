/**
 * DropdownMenu.tsx - Triggered dropdown menu composition
 *
 * Combines BasePopover + BaseMenu for button-triggered dropdowns.
 */

import React, { type ReactNode, useMemo } from "react";
import { BasePopover, type PopoverPlacement } from "./BasePopover";
import { BaseMenu } from "./BaseMenu";

export interface DropdownMenuProps {
  isOpen: boolean;
  triggerElement?: HTMLElement | null;
  triggerRect?: DOMRect | null;
  position?: "bottom-left" | "bottom-center" | "bottom-right";
  width?: number | "trigger" | "auto";
  offset?: number;
  children: ReactNode;
  onClose?: () => void;
}

export const DropdownMenu: React.FC<DropdownMenuProps> = ({
  isOpen,
  triggerElement,
  triggerRect,
  position = "bottom-left",
  width = "auto",
  offset = 4,
  children,
  onClose,
}) => {
  // Map position to placement
  const placement: PopoverPlacement = useMemo(() => {
    switch (position) {
      case "bottom-center":
        return "bottom-center";
      case "bottom-right":
        return "bottom-end";
      default:
        return "bottom-start";
    }
  }, [position]);

  return (
    <BasePopover
      isOpen={isOpen}
      triggerElement={triggerElement}
      triggerRect={triggerRect}
      placement={placement}
      width={width}
      offset={offset}
      onClickOutside={onClose}
    >
      <BaseMenu>{children}</BaseMenu>
    </BasePopover>
  );
};

export default DropdownMenu;
