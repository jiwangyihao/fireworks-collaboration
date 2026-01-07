/**
 * BaseMenu.tsx - Menu container component
 *
 * Provides consistent menu styling (background, border, shadow, padding).
 */

import React, { type ReactNode, forwardRef } from "react";

export interface BaseMenuProps {
  children: ReactNode;
  className?: string;
  size?: "xs" | "sm" | "md";
  maxHeight?: string; // e.g. "max-h-64"
}

export const BaseMenu = forwardRef<HTMLUListElement, BaseMenuProps>(
  ({ children, className = "", size = "xs", maxHeight = "max-h-64" }, ref) => {
    return (
      <ul
        ref={ref}
        className={`
          menu bg-base-100 rounded-xl shadow-xl border border-base-content/10
          p-1.5 gap-0.5 list-none m-0
          flex flex-col flex-nowrap
          overflow-x-hidden overflow-y-auto
          menu-${size}
          ${maxHeight}
          ${className}
        `
          .trim()
          .replace(/\s+/g, " ")}
      >
        {children}
      </ul>
    );
  }
);

BaseMenu.displayName = "BaseMenu";

export default BaseMenu;
