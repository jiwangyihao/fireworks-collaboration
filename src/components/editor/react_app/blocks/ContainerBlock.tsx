/**
 * ContainerBlock - VitePress 容器块
 *
 * 支持 :::tip, :::warning, :::danger, :::details, :::info, :::note 容器
 * 标题和内容统一放入 contentRef，第一行为标题
 */

import { createReactBlockSpec } from "@blocknote/react";
import React, { useState, useEffect, useRef } from "react";
import { contentRegistry, iconify } from "../ContentRegistry";
import { Icon } from "@iconify/react";

// 容器类型定义（与 VitePress 官方一致）
export const containerTypes = {
  tip: {
    label: "提示",
    icon: "lucide:lightbulb",
    className: "container-block-tip",
    aliases: ["tip", "ts", "hint", "tishi"],
  },
  info: {
    label: "信息",
    icon: "lucide:info",
    className: "container-block-info",
    aliases: ["info", "xx", "xinxi"],
  },
  warning: {
    label: "警告",
    icon: "lucide:triangle-alert",
    className: "container-block-warning",
    aliases: ["warning", "jg", "jinggao"],
  },
  danger: {
    label: "危险",
    icon: "lucide:flame",
    className: "container-block-danger",
    aliases: ["danger", "wx", "weixian"],
  },
  details: {
    label: "详情",
    icon: "lucide:list-collapse",
    className: "container-block-details",
    aliases: ["details", "xq", "collapse", "xiangqing", "zhedie"],
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

      // 注册 Executor
      // 使用 ref 避免闭包过期导致 executor 重新注册或获取旧值
      const containerTypeRef = useRef(containerType);
      useEffect(() => {
        containerTypeRef.current = containerType;
      }, [containerType]);

      useEffect(() => {
        contentRegistry.registerExecutor(props.block.id, "containerType", {
          getValue: () => containerTypeRef.current || "tip",
          execute: (val: any) => {
            const block = props.editor.getBlock(props.block.id);
            if (!block) return;

            const newType = val as ContainerTypeKey;
            const currentType = block.props.containerType as ContainerTypeKey;

            // 检查是否需要更新默认标题
            let updateContent = undefined;
            const currentContent = block.content;

            if (
              currentContent &&
              Array.isArray(currentContent) &&
              currentContent.length > 0
            ) {
              const currentConfig = containerTypes[currentType];
              const newConfig = containerTypes[newType];

              const firstNode = currentContent[0];
              if (firstNode.type === "text") {
                const currentText = (firstNode as any).text;
                const label = currentConfig.label;

                if (currentText && currentText.startsWith(label)) {
                  const remainder = currentText.slice(label.length);
                  // 如果剩余部分为空或以空白字符开头，则认为是默认标题，进行替换
                  if (remainder.length === 0 || /^\s/.test(remainder)) {
                    const newText = newConfig.label + remainder;
                    updateContent = [
                      { ...firstNode, text: newText },
                      ...currentContent.slice(1),
                    ];
                  }
                }
              }
            }

            props.editor.updateBlock(block, {
              props: { containerType: val },
              ...(updateContent ? { content: updateContent as any } : {}),
            });

            // 修复切换类型后光标丢失的问题
            // 延时聚焦以确保 DOM 更新完成
            setTimeout(() => {
              props.editor.focus();
              contentRegistry.focusBlock(props.editor, props.block.id, "end");
            }, 10);
          },
          getOptions: () =>
            Object.entries(containerTypes).map(([value, config]) => ({
              value,
              label: config.label,
              icon: React.createElement(Icon, {
                icon: config.icon,
                className: "w-4 h-4",
              }),
            })),
          isActive: () => false,
        });
      }, [props.block.id, props.editor]);

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

// 注册容器块：Toolbar + SlashMenu + SideMenu
contentRegistry.register("container", {
  icon: iconify("lucide:box-select"),
  label: "容器",
  supportedStyles: true,

  // Toolbar Actions
  actions: [
    {
      type: "dropdown",
      id: "containerType",
      label: "类型",
      icon: iconify("lucide:palette"),
    },
  ],

  // SlashMenu Items (一个类型对应多个菜单项)
  slashMenuItems: Object.entries(containerTypes).map(([key, config]) => ({
    id: `container-${key}`,
    title: config.label,
    subtext: `插入${config.label}容器`,
    icon: iconify(config.icon),
    group: "容器",
    aliases: [...config.aliases],
    blockType: "container",
    props: { containerType: key },
  })),

  // SideMenu Actions
  sideMenuActions: Object.entries(containerTypes).map(([key, config]) => ({
    id: `container-type-${key}`,
    label: config.label,
    icon: config.icon,
    isActive: (block) => (block.props as any).containerType === key,
    execute: (block, editor) => {
      const currentType = (block.props as any)
        .containerType as ContainerTypeKey;
      const currentContent = (block.content || []) as any[];
      const currentConfig = containerTypes[currentType];

      // 检查是否使用默认标题
      let updateContent = undefined;

      if (currentContent && currentContent.length > 0) {
        const firstNode = currentContent[0];
        if (firstNode.type === "text") {
          const currentText = firstNode.text;
          const label = currentConfig.label;

          if (currentText && currentText.startsWith(label)) {
            const remainder = currentText.slice(label.length);
            if (remainder.length === 0 || /^\s/.test(remainder)) {
              const newLabel = containerTypes[key as ContainerTypeKey].label;
              const newText = newLabel + remainder;

              updateContent = [
                { ...firstNode, text: newText },
                ...currentContent.slice(1),
              ];
            }
          }
        }
      }

      editor.updateBlock(block, {
        props: { containerType: key },
        ...(updateContent ? { content: updateContent as any } : {}),
      });
    },
  })),
});
