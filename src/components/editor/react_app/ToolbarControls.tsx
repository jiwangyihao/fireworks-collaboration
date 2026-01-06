/**
 * ToolbarControls.tsx - StaticToolbar 的扩展控件组件
 *
 * 提供：ToolbarDropdown, ToolbarInput, ToolbarToggle
 */

import React, { useState, useRef, useEffect, memo } from "react";
import { RiArrowDownSLine } from "react-icons/ri";
import type { DropdownOption } from "./BlockCapabilities";

// --- Toolbar Dropdown ---

interface ToolbarDropdownProps {
  icon: React.ReactNode;
  label: string;
  value: string;
  options: DropdownOption[];
  onChange: (value: string) => void;
  iconOnly?: boolean; // 只显示图标，不显示当前选中值
}

import { createPortal } from "react-dom";

// ... (other imports)

export const ToolbarDropdown = memo(function ToolbarDropdown({
  icon,
  label,
  value,
  options,
  onChange,
  iconOnly = false,
}: ToolbarDropdownProps) {
  const [isOpen, setIsOpen] = useState(false);
  const dropdownRef = useRef<HTMLDivElement>(null); // Ref for the button/wrapper
  const menuRef = useRef<HTMLUListElement>(null); // Ref for the portal menu
  const [position, setPosition] = useState<{
    top: number;
    left: number;
  } | null>(null);

  // Current selected option
  const selectedOption = options.find((opt) => opt.value === value);

  // Close when clicking outside
  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      // Check if click is inside button (dropdownRef) or menu (menuRef)
      if (
        dropdownRef.current &&
        !dropdownRef.current.contains(event.target as Node) &&
        menuRef.current &&
        !menuRef.current.contains(event.target as Node)
      ) {
        setIsOpen(false);
      }
    };

    if (isOpen) {
      document.addEventListener("mousedown", handleClickOutside);
    }
    return () => {
      document.removeEventListener("mousedown", handleClickOutside);
    };
  }, [isOpen]);

  // Update position when opening or scrolling/resizing
  const updatePosition = () => {
    if (dropdownRef.current) {
      const rect = dropdownRef.current.getBoundingClientRect();
      setPosition({
        top: rect.bottom + 5,
        left: rect.left,
      });
    }
  };

  useEffect(() => {
    if (isOpen) {
      updatePosition();
      window.addEventListener("scroll", updatePosition, true);
      window.addEventListener("resize", updatePosition);
    }
    return () => {
      window.removeEventListener("scroll", updatePosition, true);
      window.removeEventListener("resize", updatePosition);
    };
  }, [isOpen]);

  const menu = (
    <ul
      ref={menuRef}
      className="fixed mt-1 menu flex flex-col flex-nowrap p-1 shadow-xl bg-white rounded-lg w-36 max-h-64 overflow-y-auto border border-gray-200 text-xs gap-0.5"
      style={{
        zIndex: 99999,
        top: position?.top ?? 0,
        left: position?.left ?? 0,
      }}
    >
      {options.map((option) => (
        <li key={option.value} className="m-0! p-0!">
          <button
            type="button"
            onClick={() => {
              onChange(option.value);
              setIsOpen(false);
            }}
            className={`py-1.5 px-2 rounded-md border flex items-center gap-2 w-full transition-all ${
              option.value === value
                ? "border-primary/30 bg-primary/5 text-primary font-medium"
                : "border-transparent hover:border-base-content/20 hover:bg-base-200 text-base-content/80"
            }`}
          >
            {option.icon && <span className="opacity-70">{option.icon}</span>}
            {option.label}
          </button>
        </li>
      ))}
    </ul>
  );

  return (
    <div className="relative" ref={dropdownRef}>
      <button
        type="button"
        className="btn btn-xs btn-ghost gap-1 h-6 min-h-0 px-2 font-normal text-gray-600 hover:bg-gray-200 border-transparent hover:border-gray-300"
        onClick={() => setIsOpen(!isOpen)}
        title={label}
      >
        <span className="opacity-70 w-4 h-4 flex items-center justify-center flex-shrink-0">
          {icon}
        </span>
        {!iconOnly && (
          <span className="font-mono text-xs max-w-[4rem] truncate">
            {selectedOption?.label || value}
          </span>
        )}
        <RiArrowDownSLine
          className={`w-3 h-3 opacity-50 transition-transform ${
            isOpen ? "rotate-180" : ""
          }`}
        />
      </button>

      {isOpen && createPortal(menu, document.body)}
    </div>
  );
});

// --- Toolbar Input ---

interface ToolbarInputProps {
  icon: React.ReactNode;
  label: string;
  value: string;
  placeholder?: string;
  width?: string;
  onChange: (value: string) => void;
  hideIcon?: boolean;
}

export const ToolbarInput = memo(function ToolbarInput({
  icon,
  label,
  value,
  placeholder = "",
  width = "6rem",
  onChange,
  hideIcon = false,
}: ToolbarInputProps) {
  const [localValue, setLocalValue] = useState(value);

  // 同步外部值
  useEffect(() => {
    setLocalValue(value);
  }, [value]);

  // 防抖提交
  useEffect(() => {
    const timer = setTimeout(() => {
      if (localValue !== value) {
        onChange(localValue);
      }
    }, 300);
    return () => clearTimeout(timer);
  }, [localValue, value, onChange]);

  return (
    <div className="flex items-center gap-1">
      {!hideIcon && (
        <span
          className="opacity-50 w-4 h-4 flex items-center justify-center flex-shrink-0"
          title={label}
        >
          {icon}
        </span>
      )}
      <input
        type="text"
        className="input input-ghost input-xs h-6 min-h-0 px-1 focus:bg-white placeholder:text-gray-300 font-mono text-gray-600 focus:outline-none focus:ring-1 focus:ring-blue-200 rounded-sm"
        style={{ width }}
        placeholder={placeholder}
        value={localValue}
        onChange={(e) => setLocalValue(e.target.value)}
        onBlur={() => onChange(localValue)}
      />
    </div>
  );
});

// --- Toolbar Toggle ---

interface ToolbarToggleProps {
  icon: React.ReactNode;
  label: string;
  isActive: boolean;
  onChange: (value: boolean) => void;
}

export const ToolbarToggle = memo(function ToolbarToggle({
  icon,
  label,
  isActive,
  onChange,
}: ToolbarToggleProps) {
  return (
    <button
      type="button"
      className={`static-toolbar-btn ${isActive ? "active" : ""}`}
      onClick={() => onChange(!isActive)}
      title={label}
    >
      <span className="w-4 h-4 flex items-center justify-center">{icon}</span>
    </button>
  );
});
