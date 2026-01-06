import { createReactBlockSpec } from "@blocknote/react";
import { useRef, useEffect } from "react";
import { initMathLive } from "./MathLiveUtils";
import { MathfieldElement } from "mathlive";
import { blockRegistry } from "../BlockCapabilities";
import { Icon } from "@iconify/react";
import React from "react";

// 初始化 MathLive
initMathLive();

declare global {
  namespace JSX {
    interface IntrinsicElements {
      "math-field": any;
    }
  }
}

export const MathBlock = createReactBlockSpec(
  {
    type: "math",
    propSchema: {
      formula: {
        default: "",
      },
    },
    content: "none",
  },
  {
    render: (props) => {
      const formula = props.block.props.formula as string;
      const ref = useRef<MathfieldElement>(null);

      // 实时更新 Block 属性
      const handleInput = (e: React.SyntheticEvent<MathfieldElement>) => {
        const val = (e.target as MathfieldElement).value;
        props.editor.updateBlock(props.block, {
          props: { formula: val },
        });
      };
      // 注册执行器到 Registry (用于 StaticToolbar 调用)
      // 像 ShikiCodeBlock 一样，在 mount 时注册，而不是在 focus 时
      useEffect(() => {
        const blockId = props.block.id;

        blockRegistry.registerExecutor(blockId, "formula", {
          execute: (val: string) => {
            if (ref.current) {
              ref.current.value = val;
              props.editor.updateBlock(props.block, {
                props: { formula: val },
              });
            }
          },
          isActive: () => true,
          getValue: () => ref.current?.value || "",
        });

        // 注册 MathLive 专属动作
        blockRegistry.registerExecutor(blockId, "toggleKeyboard", {
          execute: () => {
            ref.current?.executeCommand("toggleVirtualKeyboard");
          },
          isActive: () => false,
        });

        blockRegistry.registerExecutor(blockId, "toggleMenu", {
          execute: (e: React.MouseEvent) => {
            // e is the click event from the toolbar button
            if (ref.current && e && e.currentTarget) {
              const button = e.currentTarget as HTMLElement;
              const rect = button.getBoundingClientRect();
              // Show menu below the button
              ref.current.showMenu({
                location: {
                  x: rect.left,
                  y: rect.bottom + 5,
                },
                modifiers: {
                  alt: false,
                  control: false,
                  shift: false,
                  meta: false,
                },
              });
            } else {
              // Fallback if no event provided (e.g. keyboard shortcut?)
              ref.current?.executeCommand("toggleContextMenu");
            }
          },
          isActive: () => false,
        });

        return () => {
          blockRegistry.unregisterExecutor(blockId, "formula");
          blockRegistry.unregisterExecutor(blockId, "toggleKeyboard");
          blockRegistry.unregisterExecutor(blockId, "toggleMenu");
        };
      }, [props.block.id]);

      // 处理 math-field 获焦时，更新 BlockNote 的 selection 到此块
      const handleFocus = () => {
        // 使用 blockRegistry 辅助方法同步选区
        blockRegistry.focusBlock(props.editor, props.block.id);
      };

      return (
        <div className="w-full my-2 relative" data-math-block>
          {/* @ts-ignore */}
          <math-field
            ref={ref}
            // 使用 CSS 类控制样式 (见 style.css)
            // block-math 样式让它居中显示
            class="w-full p-2 text-lg text-center"
            style={{
              display: "block",
              width: "100%",
            }}
            onInput={handleInput}
            onFocus={handleFocus}
            onMouseDown={(e: MouseEvent) => e.stopPropagation()}
            onClick={(e: MouseEvent) => e.stopPropagation()}
          >
            {formula}
            {/* @ts-ignore */}
          </math-field>
        </div>
      );
    },
  }
);

blockRegistry.register("math", {
  icon: React.createElement(Icon, {
    icon: "lucide:sigma",
    className: "w-4 h-4",
  }),
  label: "公式块",
  supportedStyles: [],
  actions: [
    {
      type: "button",
      id: "toggleKeyboard",
      label: "键盘",
      icon: <Icon icon="lucide:keyboard" />,
    },
    {
      type: "button",
      id: "toggleMenu",
      label: "菜单",
      icon: <Icon icon="lucide:menu" />,
    },
  ],
});
