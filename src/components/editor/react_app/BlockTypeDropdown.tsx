/**
 * BlockTypeDropdown.tsx - 块类型选择下拉菜单
 *
 * 风格严格对齐 ShikiCodeBlock 的语言选择器
 *
 * 已重构为使用 menu/ 组件库
 */

import React, { useState, useRef, memo } from "react";
import { RiArrowDownSLine } from "react-icons/ri";
import { DropdownMenu, MenuItem } from "./menu";

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
  const buttonRef = useRef<HTMLDivElement>(null);

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

  const handleSelect = (option: BlockTypeOption) => {
    onSelect(option.value, option.props);
    setIsOpen(false);
  };

  const isOptionActive = (option: BlockTypeOption) => {
    return (
      selectedOption?.value === option.value &&
      JSON.stringify(selectedOption.props) === JSON.stringify(option.props)
    );
  };

  return (
    <div
      className="relative static-toolbar-dropdown"
      style={{ zIndex: isOpen ? 9999 : 20 }}
    >
      {/* 触发按钮 */}
      <div
        ref={buttonRef}
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

      <DropdownMenu
        isOpen={isOpen}
        triggerElement={buttonRef.current}
        position="bottom-left"
        width={160}
        onClose={() => setIsOpen(false)}
      >
        {items.map((option) => (
          <MenuItem
            key={`${option.value}-${JSON.stringify(option.props || {})}`}
            icon={option.icon}
            label={option.label}
            active={isOptionActive(option)}
            onClick={() => handleSelect(option)}
          />
        ))}
      </DropdownMenu>
    </div>
  );
});
