/**
 * ContainerBlock - VitePress 容器块
 *
 * 支持 :::tip, :::warning, :::danger, :::details, :::info, :::note 容器
 * 标题和内容统一放入 contentRef，第一行为标题
 */

import { createReactBlockSpec } from "@blocknote/react";
import { useState } from "react";

// 容器类型定义（与 VitePress 官方一致）
export const containerTypes = {
  tip: {
    label: "提示",
    icon: "lucide:lightbulb",
    className: "container-block-tip",
  },
  info: {
    label: "信息",
    icon: "lucide:info",
    className: "container-block-info",
  },
  warning: {
    label: "警告",
    icon: "lucide:triangle-alert",
    className: "container-block-warning",
  },
  danger: {
    label: "危险",
    icon: "lucide:flame",
    className: "container-block-danger",
  },
  details: {
    label: "详情",
    icon: "lucide:list-collapse",
    className: "container-block-details",
  },
} as const;

export type ContainerTypeKey = keyof typeof containerTypes;

export const ContainerBlock = createReactBlockSpec(
  {
    type: "container",
    propSchema: {
      containerType: {
        default: "tip" as ContainerTypeKey,
      },
    },
    content: "inline",
  },
  {
    render: (props) => {
      const containerType = props.block.props.containerType as ContainerTypeKey;
      const [isOpen, setIsOpen] = useState(containerType !== "details");

      const typeConfig = containerTypes[containerType] || containerTypes.tip;

      return (
        <div
          className={`w-full border-l-4 rounded-r-lg p-4 relative ${typeConfig.className}`}
        >
          {/* details 类型的折叠按钮 - 右上角绝对定位 */}
          {containerType === "details" && (
            <button
              type="button"
              onClick={() => setIsOpen(!isOpen)}
              className="btn btn-xs btn-ghost absolute top-1/2 right-2 -translate-y-1/2 opacity-60 hover:opacity-100"
            >
              {isOpen ? "▼" : "▶"}
            </button>
          )}

          {/* 内容区域 */}
          {(isOpen || containerType !== "details") && (
            <div
              className="inline-content container-content"
              ref={props.contentRef}
            />
          )}
        </div>
      );
    },
  }
);
