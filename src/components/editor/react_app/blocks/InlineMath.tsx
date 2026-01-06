// @ts-nocheck
/**
 * InlineMath - LaTeX 行内公式 (MathLive Integration)
 *
 * 使用 MathLive <math-field> 提供所见即所得的编辑体验
 */

import { createReactInlineContentSpec } from "@blocknote/react";
import { useLayoutEffect, useRef, useEffect, useState } from "react";
import { initMathLive } from "./MathLiveUtils";
import { MathfieldElement } from "mathlive";
import { Icon } from "@iconify/react"; // Use Iconify
import { NodeSelection } from "prosemirror-state";
import { blockRegistry } from "../BlockCapabilities";

// 初始化 MathLive
initMathLive();

// 为 React 声明自定义元素类型
declare global {
  namespace JSX {
    interface IntrinsicElements {
      "math-field": any;
    }
  }
}

export const InlineMath = createReactInlineContentSpec(
  {
    type: "inlineMath",
    propSchema: {
      formula: {
        default: "",
      },
    },
    content: "none",
  },
  {
    render: (props) => {
      // 兼容旧数据，formula 对应 latex
      const latex = (props.inlineContent.props.formula as string) || "";
      const mathfieldRef = useRef<MathfieldElement>(null);
      const spanRef = useRef<HTMLSpanElement>(null);

      // 编辑器实例
      const editor = props.editor as any;
      const view = editor._tiptapEditor?.view;

      // 处理焦点导航 (Arrow keys move out)
      useLayoutEffect(() => {
        const mf = mathfieldRef.current;
        if (!mf) return;

        // Note: mathlive fires 'focus-out' or custom events for navigation?
        // User example used 'move-out'. Let's verify standard Custom Element events or MathLive specifics.
        // MathLive documents 'move-out' event.

        const handleMoveOut = (e: CustomEvent<{ direction: string }>) => {
          if (!view) return;

          // Find position in Prosemirror
          // We look for the position of the wrapper span
          const pos = view.posAtDOM(spanRef.current as Node, 0);
          if (pos === null || typeof pos === "undefined") {
            return;
          }

          if (e.detail.direction === "forward") {
            // Move cursor to after the node
            // The node size is 1 (inline content atom)
            mf.blur();
            // Focus just after this node.
            // posAtDOM usually returns position BEFORE the node if strict?
            // Or inside?
            // Since inline content is an atom (content: "none"), pos points to it.
            // We want pos + 1.
            editor._tiptapEditor.commands.focus(pos + 1);
          } else if (e.detail.direction === "backward") {
            mf.blur();
            editor._tiptapEditor.commands.focus(pos);
          }
        };

        (mf as unknown as HTMLElement).addEventListener(
          "move-out",
          handleMoveOut as EventListener
        );
        return () => {
          (mf as unknown as HTMLElement).removeEventListener(
            "move-out",
            handleMoveOut as EventListener
          );
        };
      }, [view, editor]);

      // 监听外部光标进入事件 (Move In)
      // 使用捕获阶段 (Capture Phase) 以便在 ProseMirror 处理之前拦截事件
      useEffect(() => {
        if (!view) return;

        const handleKeyDown = (e: KeyboardEvent) => {
          const dom = spanRef.current;
          // 确保组件已挂载且在视图中
          if (!dom || !view.dom.contains(dom)) return;

          // 仅处理左右方向键
          if (e.key !== "ArrowLeft" && e.key !== "ArrowRight") return;

          const { selection } = view.state;
          if (!selection.empty) return; // 只处理光标状态

          // 获取当前节点位置
          // 尝试使用 getPos (如果 props 中有提供，BlockNote 内部实现可能不同)
          // 这里回退到 posAtDOM，更健壮的方式是获取精确位置
          // 对于 Atom Node (inline-block)，posAtDOM(dom) 通常返回节点开始位置
          let pos = -1;
          try {
            // 0 偏移量通常指向节点开始或内部
            pos = view.posAtDOM(dom, 0);
          } catch (err) {
            return;
          }

          if (pos < 0) return;

          // 修正：对于行内原子节点，posAtDOM 有时可能返回父节点偏移。
          // 检查 nodeAt(pos) 是否是我们自己
          const nodeAtPos = view.state.doc.nodeAt(pos);
          // 如果 nodeAtPos 不是 inlineMath，尝试调整 (有时偏离 1)
          // 但通常 view.posAtDOM(range) 更准。

          /* 
             关键修复：
             当光标在节点前时，cursor == pos
             当光标在节点后时，cursor == pos + 1 (节点大小为1)
          */

          const cursor = selection.$from.pos;
          const mf = mathfieldRef.current;
          if (!mf) return;

          // 关键修复：如果您已经在编辑这个 MathField，不要拦截
          // 否则会导致由于 PM selection 未更新而产生的死循环 (例如按右键一直重置为 Start)
          if (document.activeElement === mf) return;

          // ArrowLeft: 当光标在节点 **后面** (pos + 1) 时，拦截并进入
          if (e.key === "ArrowLeft") {
            if (cursor === pos + 1) {
              e.preventDefault();
              e.stopPropagation();
              mf.focus();
              // 使用 setTimeout 延迟一小段时间，确保 MathLive 内部的状态重置已完成
              setTimeout(() => {
                mf.executeCommand("moveToMathfieldEnd");
              }, 10);
              return;
            }
          }

          // ArrowRight: 当光标在节点 **前面** (pos) 时，拦截并进入
          if (e.key === "ArrowRight") {
            if (cursor === pos) {
              e.preventDefault();
              e.stopPropagation();
              mf.focus();
              setTimeout(() => {
                mf.executeCommand("moveToMathfieldStart");
              }, 10);
              return;
            }
          }
        };

        // 使用 true 开启捕获阶段
        view.dom.addEventListener("keydown", handleKeyDown, true);
        return () => {
          view.dom.removeEventListener("keydown", handleKeyDown, true);
        };
      }, [view]);

      // 处理输入更新
      const handleInput = (evt: React.SyntheticEvent<MathfieldElement>) => {
        const target = evt.target as MathfieldElement;
        const value = target.value;

        // 更新 Prosemirror 节点
        // 我们需要找到当前节点的位置
        if (!view || !spanRef.current) return;

        // 使用 tr.setNodeAttribute (性能更好，无需 state re-render logic via props dependency loop)
        // 但是我们需要防抖吗？MathLive 更新很快。
        // 直接更新可能导致光标跳动？
        // MathLive component manages its own internal state/cursor.
        // We only push to PM on change. PM update might re-render us?
        // If PM re-renders us with new prop, MathLive might verify value matches.
        // MathLive handles `value` prop changes gracefully usually.

        // Find position
        // copy findNodePos logic for robustness
        let pos = view.posAtDOM(spanRef.current, 0);

        // 简单的位置修正逻辑
        // 如果 pos 指向内容内部，尝试修正
        if (pos !== null) {
          // Check if node at pos is us.
          // If not, maybe search around or trust posAtDOM for atom.
          // Direct dispatch
          // Avoid creating a new history step for every keystroke?
          // Maybe throttle? Or let PM handle it.
          // For now direct update.

          // Check if value changed
          if (value !== latex) {
            const { tr } = view.state;
            // 需要确保 pos 准确指向 Node
            // 实际上 posAtDOM 对于 inline node 可能返回它前面的位置。
            // 简单的检查：
            const node = view.state.doc.nodeAt(pos);
            if (node && node.type.name === "inlineMath") {
              view.dispatch(tr.setNodeAttribute(pos, "formula", value));
            } else {
              // Fallback scan?
              // 暂且忽略，假设 pos 准确 (Atomic Node at pos)
            }
          }
        }
      };

      // 注册块能力

      // 焦点管理 & Registry Reporting
      const wasActiveRef = useRef(false);

      useEffect(() => {
        if (!view || !spanRef.current) return;

        const checkActive = () => {
          const dom = spanRef.current;
          if (!dom) return;

          // Check if selection is on this node
          const pos = view.posAtDOM(dom, 0);
          if (pos === null) return;

          const { from, to } = view.state.selection;
          const node = view.state.doc.nodeAt(pos);

          // Relaxed check:
          // 1. Precise PM NodeSelection
          // 关键：只用 isFocused 判断是否激活
          // isNodeSelection 可能在 blur 时仍为 true（PM selection 还没更新）

          // Debug: Check active element
          // console.log("InlineMath checkActive", document.activeElement);

          const isFocused =
            document.activeElement === mathfieldRef.current ||
            // Allow focus to be on body temporarily (during clicks)??
            // Or better: rely on the fact that if we preventDefault on toolbar, activeElement won't change.
            // But if user clicks OUTSIDE editor, we want to blur.

            // Check if active element is within the toolbar?
            document.activeElement?.closest(".editor-toolbar");

          const isMathFieldFocused =
            document.activeElement === mathfieldRef.current;

          if (isFocused) {
            // 只设置类型，不需要执行器（没有输入框了）
            blockRegistry.setActiveInline("inlineMath", {
              toggleKeyboard: {
                execute: () => {
                  mathfieldRef.current?.executeCommand("toggleVirtualKeyboard");
                },
                isActive: () => false,
              },
              toggleMenu: {
                execute: (e: any) => {
                  if (mathfieldRef.current && e?.currentTarget) {
                    const button = e.currentTarget as HTMLElement;
                    const rect = button.getBoundingClientRect();
                    mathfieldRef.current.showMenu({
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
                    mathfieldRef.current?.executeCommand("toggleContextMenu");
                  }
                },
                isActive: () => false,
              },
            });
            wasActiveRef.current = true;
          } else if (wasActiveRef.current) {
            // 只有当这个实例之前是激活的，才清除

            blockRegistry.setActiveInline(null);
            wasActiveRef.current = false;
          }
        };

        const unsubscribe = editor.onSelectionChange(checkActive);

        // 也监听 math-field 的 focus/blur 事件
        const mf = mathfieldRef.current;
        if (mf) {
          mf.addEventListener("focus", checkActive);
          mf.addEventListener("blur", checkActive);
        }

        return () => {
          unsubscribe();
          if (mf) {
            mf.removeEventListener("focus", checkActive);
            mf.removeEventListener("blur", checkActive);
          }
          // 组件卸载时，如果还是激活状态，清除
          if (wasActiveRef.current) {
            blockRegistry.setActiveInline(null);
          }
        };
      }, [view, editor]);

      // Override Focus to sync selection
      const handleFocus = () => {
        if (!view || !spanRef.current) return;
        const pos = view.posAtDOM(spanRef.current, 0);
        if (pos !== null) {
          // Force NodeSelection
          const tr = view.state.tr.setSelection(
            NodeSelection.create(view.state.doc, pos)
          );
          view.dispatch(tr);
        }
      };

      return (
        <span
          ref={spanRef}
          className="inline-math-node inline-block mx-1 align-middle"
          data-inline-math
        >
          <math-field
            ref={mathfieldRef}
            class="px-1 bg-gray-100 rounded cursor-pointer min-w-[20px] inline-block text-center"
            style={{
              display: "inline-block",
              minWidth: "20px",
            }}
            onInput={handleInput}
            // Sync Focus to PM Selection
            onFocus={handleFocus}
            onMouseDown={(e: MouseEvent) => e.stopPropagation()}
            onClick={(e: MouseEvent) => e.stopPropagation()}
          >
            {latex}
          </math-field>
        </span>
      );
    },
  }
);

// Register capabilities globally for InlineMath
blockRegistry.register("inlineMath", {
  label: "行内公式",
  icon: <Icon icon="lucide:function-square" />,
  supportedStyles: ["inlineMath"],
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
