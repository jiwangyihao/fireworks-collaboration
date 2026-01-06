/**
 * BlockTypeDropdown.tsx - 块类型选择下拉菜单
 *
 * 风格严格对齐 ShikiCodeBlock 的语言选择器
 */

import React, { useState, useRef, useEffect, memo } from "react";
import { RiArrowDownSLine } from "react-icons/ri";

export interface BlockTypeOption {
  value: string;
  label: string;
  icon: React.ReactNode;
  props?: Record<string, any>;
}

interface BlockTypeDropdownProps {
  currentType: string;
  currentProps?: Record<string, any>;
  items: BlockTypeOption[]; // 现在由外部动态传入
  onSelect: (type: string, props?: Record<string, any>) => void;
}

export const BlockTypeDropdown = memo(function BlockTypeDropdown({
  currentType,
  currentProps,
  items,
  onSelect,
}: BlockTypeDropdownProps) {
  const [isOpen, setIsOpen] = useState(false);
  const dropdownRef = useRef<HTMLDivElement>(null);

  // 计算当前显示的选中项
  const selectedOption =
    items.find((option) => {
      if (option.value === currentType) {
        // 检查 props 是否匹配 (主要针对 heading)
        if (
          currentType === "heading" &&
          currentProps?.level &&
          option.props?.level
        ) {
          return currentProps.level === option.props.level;
        }
        return true;
      }
      return false;
    }) || items[0];

  // 点击外部关闭
  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (
        dropdownRef.current &&
        !dropdownRef.current.contains(event.target as Node)
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

  const handleSelect = (option: BlockTypeOption) => {
    onSelect(option.value, option.props);
    setIsOpen(false);
  };

  /**
   * 样式说明：
   * 严格复用 ShikiCodeBlock.tsx 中的 CSS 类:
   * Trigger: btn btn-xs btn-ghost gap-1 h-6 min-h-0 px-2 font-normal text-gray-600 hover:bg-gray-200 border-transparent hover:border-gray-300
   * Dropdown: menu flex flex-col flex-nowrap p-1 shadow-lg bg-base-100 rounded-lg w-40 max-h-64 overflow-y-auto border border-base-200 text-xs gap-0.5
   */

  return (
    <div
      className="relative static-toolbar-dropdown"
      ref={dropdownRef}
      style={{ zIndex: isOpen ? 9999 : 20 }}
    >
      {/* 触发按钮 */}
      <div
        role="button"
        className="btn btn-xs btn-ghost gap-1 h-6 min-h-0 px-2 font-normal text-gray-600 hover:bg-gray-200 border-transparent hover:border-gray-300"
        onClick={() => setIsOpen(!isOpen)}
        title="更改块类型"
      >
        <span className="font-mono flex items-center gap-1.5 min-w-[5rem]">
          {selectedOption?.icon && (
            <span className="opacity-70 text-sm w-4 h-4 flex items-center justify-center flex-shrink-0">
              {selectedOption.icon}
            </span>
          )}
          <span>{selectedOption?.label || "未命名"}</span>
        </span>
        <RiArrowDownSLine
          className={`w-3 h-3 opacity-50 transition-transform ${isOpen ? "rotate-180" : ""}`}
        />
      </div>

      {isOpen && (
        <>
          <div
            className="fixed inset-0 z-50"
            onClick={() => setIsOpen(false)}
          ></div>
          <ul
            className="absolute top-full left-0 mt-1 z-[9999] menu flex flex-col flex-nowrap p-2 shadow-xl bg-white rounded-lg w-40 max-h-64 overflow-y-auto border border-gray-100 text-xs gap-0.5 transform translate-y-0"
            style={{ minWidth: "160px" }}
          >
            {" "}
            {/* Ensure min width to fit content */}
            {items.map((option) => (
              <li
                className="m-0! p-0!"
                key={`${option.value}-${JSON.stringify(option.props || {})}`}
              >
                <button
                  type="button"
                  onClick={() => handleSelect(option)}
                  className={`py-1.5 px-2 rounded-md border transition-all ${
                    // 使用相同的 active 样式
                    selectedOption?.value === option.value &&
                    JSON.stringify(selectedOption.props) ===
                      JSON.stringify(option.props)
                      ? "border-primary/30 bg-primary/5 text-primary font-medium"
                      : "border-transparent hover:border-base-content/20 hover:bg-base-200 text-base-content/80"
                  }`}
                  style={{
                    display: "flex",
                    alignItems: "center",
                    gap: "0.5rem",
                    width: "100%",
                  }}
                >
                  <span className="text-lg opacity-70 w-5 h-5 flex items-center justify-center flex-shrink-0">
                    {option.icon}
                  </span>
                  {option.label}
                </button>
              </li>
            ))}
          </ul>
        </>
      )}
    </div>
  );
});
