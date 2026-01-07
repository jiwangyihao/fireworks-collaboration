/**
 * MenuItem.tsx - Standardized menu item component
 *
 * Supports icon, label, description, shortcut, and active/disabled states.
 */

import React, { type ReactNode, memo } from "react";
import { Icon } from "@iconify/react";

export interface MenuItemProps {
  /** Icon - can be an iconify string (e.g. "ph:globe") or a ReactNode */
  icon?: string | ReactNode;
  label?: string;
  description?: string;
  shortcut?: string;
  active?: boolean;
  disabled?: boolean;
  danger?: boolean;
  children?: ReactNode;
  rightContent?: ReactNode;
  onClick?: (e: React.MouseEvent) => void;
  className?: string;
}

export const MenuItem = memo(function MenuItem({
  icon,
  label,
  description,
  shortcut,
  active = false,
  disabled = false,
  danger = false,
  children,
  rightContent,
  onClick,
  className = "",
}: MenuItemProps) {
  const handleClick = (e: React.MouseEvent) => {
    if (disabled) return;
    onClick?.(e);
  };

  const baseClasses = `
    flex items-start gap-0.5 py-1.5 px-2 rounded-md border border-transparent
    transition-all w-full overflow-hidden cursor-pointer
  `;

  const stateClasses = active
    ? "!border-primary bg-primary/5 text-primary font-medium"
    : danger
      ? "hover:border-error/20 hover:bg-error/5 text-error hover:text-error"
      : "hover:border-base-content/20 hover:bg-base-200 text-base-content";

  const disabledClasses = disabled ? "opacity-50 cursor-not-allowed" : "";
  const layoutClasses = description ? "flex-col" : "flex-row items-center";

  return (
    <li className="w-full overflow-hidden m-0 p-0">
      <a
        role="button"
        className={`${baseClasses} ${stateClasses} ${disabledClasses} ${layoutClasses} ${className}`
          .trim()
          .replace(/\s+/g, " ")}
        onClick={handleClick}
      >
        <div className="flex items-center gap-2 w-full">
          {icon &&
            (typeof icon === "string" ? (
              <Icon
                icon={icon}
                className={`w-4 h-4 flex-shrink-0 ${active ? "text-primary" : "opacity-60"}`}
              />
            ) : (
              <span
                className={`w-4 h-4 flex-shrink-0 flex items-center justify-center ${active ? "text-primary" : "opacity-60"}`}
              >
                {icon}
              </span>
            ))}

          <span className="font-medium flex-1 truncate">
            {children || label}
          </span>

          {shortcut && (
            <span className="text-[10px] opacity-40 font-mono flex-shrink-0">
              {shortcut}
            </span>
          )}

          {rightContent}
        </div>

        {description && (
          <span className="text-[10px] opacity-60 leading-tight truncate w-full pl-6">
            {description}
          </span>
        )}
      </a>
    </li>
  );
});

export default MenuItem;
