/**
 * BasePopover.tsx - React Portal-based popover component
 *
 * Mirrors the Vue BasePopover.vue with 12-point positioning support.
 */

import React, {
  useRef,
  useEffect,
  useState,
  useCallback,
  type ReactNode,
} from "react";
import { createPortal } from "react-dom";

export type PopoverPlacement =
  | "top-start"
  | "top-center"
  | "top-end"
  | "bottom-start"
  | "bottom-center"
  | "bottom-end"
  | "left-start"
  | "left-center"
  | "left-end"
  | "right-start"
  | "right-center"
  | "right-end";

export interface BasePopoverProps {
  isOpen: boolean;
  triggerElement?: HTMLElement | null;
  triggerRect?: DOMRect | null;
  placement?: PopoverPlacement;
  offset?: number;
  width?: number | "trigger" | "auto";
  zIndex?: number;
  children: ReactNode;
  onClickOutside?: () => void;
}

export const BasePopover: React.FC<BasePopoverProps> = ({
  isOpen,
  triggerElement,
  triggerRect,
  placement = "bottom-start",
  offset = 4,
  width = "auto",
  zIndex = 99999,
  children,
  onClickOutside,
}) => {
  const contentRef = useRef<HTMLDivElement>(null);
  const [style, setStyle] = useState<React.CSSProperties>({
    top: 0,
    left: 0,
    width: "auto",
    opacity: 0,
  });

  const updatePosition = useCallback(() => {
    if (!isOpen) return;

    const rect = triggerRect || triggerElement?.getBoundingClientRect();
    if (!rect) return;

    const popoverEl = contentRef.current;
    const popWidth = popoverEl?.offsetWidth || 0;
    const popHeight = popoverEl?.offsetHeight || 0;

    const scrollTop = window.scrollY || document.documentElement.scrollTop;
    const scrollLeft = window.scrollX || document.documentElement.scrollLeft;

    let top = 0;
    let left = 0;

    const [side, align] = placement.split("-");

    // Vertical Position (Top/Bottom)
    if (side === "top") {
      top = rect.top - popHeight - offset + scrollTop;
    } else if (side === "bottom") {
      top = rect.bottom + offset + scrollTop;
    } else {
      // Left/Right: vertically aligned based on 'align'
      if (align === "start") {
        top = rect.top + scrollTop;
      } else if (align === "center") {
        top = rect.top + rect.height / 2 - popHeight / 2 + scrollTop;
      } else if (align === "end") {
        top = rect.bottom - popHeight + scrollTop;
      }
    }

    // Horizontal Position (Left/Right)
    if (side === "left") {
      left = rect.left - popWidth - offset + scrollLeft;
    } else if (side === "right") {
      left = rect.right + offset + scrollLeft;
    } else {
      // Top/Bottom: horizontally aligned based on 'align'
      if (align === "start") {
        left = rect.left + scrollLeft;
      } else if (align === "center") {
        left = rect.left + rect.width / 2 - popWidth / 2 + scrollLeft;
      } else if (align === "end") {
        left = rect.right - popWidth + scrollLeft;
      }
    }

    // Width Handling
    let w: string | number = "auto";
    if (width === "trigger") {
      w = rect.width;
    } else if (typeof width === "number") {
      w = width;
    }

    setStyle({
      position: "fixed",
      top,
      left,
      width: w,
      zIndex,
      opacity: 1,
    });
  }, [isOpen, triggerElement, triggerRect, placement, offset, width, zIndex]);

  // Update position on open and when dependencies change
  useEffect(() => {
    if (isOpen) {
      // Use requestAnimationFrame to ensure DOM is rendered before measuring
      requestAnimationFrame(() => {
        updatePosition();
      });
    }
  }, [isOpen, updatePosition]);

  // Resize listener
  useEffect(() => {
    if (!isOpen) return;

    const handleResize = () => updatePosition();
    window.addEventListener("resize", handleResize);
    window.addEventListener("scroll", handleResize, true);

    return () => {
      window.removeEventListener("resize", handleResize);
      window.removeEventListener("scroll", handleResize, true);
    };
  }, [isOpen, updatePosition]);

  // Click outside handler
  useEffect(() => {
    if (!isOpen || !onClickOutside) return;

    const handleClickOutside = (event: MouseEvent) => {
      const target = event.target as Node;
      if (
        contentRef.current &&
        !contentRef.current.contains(target) &&
        triggerElement &&
        !triggerElement.contains(target)
      ) {
        onClickOutside();
      }
    };

    document.addEventListener("mousedown", handleClickOutside);
    return () => document.removeEventListener("mousedown", handleClickOutside);
  }, [isOpen, onClickOutside, triggerElement]);

  if (!isOpen) return null;

  return createPortal(
    <div ref={contentRef} style={style} className="popover-content">
      {children}
    </div>,
    document.body
  );
};

export default BasePopover;
