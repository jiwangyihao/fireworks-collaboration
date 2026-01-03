/**
 * MermaidBlock - Mermaid 图表块
 *
 * 支持 Mermaid 图表的可视化编辑与预览，采用语雀风格的编辑界面
 * 使用 DaisyUI 组件
 */

import { createReactBlockSpec } from "@blocknote/react";
import { useState, useEffect, useRef, useId, useCallback } from "react";
import mermaid from "mermaid";
import CodeMirror from "@uiw/react-codemirror";
import { mermaid as mermaidLang } from "codemirror-lang-mermaid";

// 初始化 Mermaid
mermaid.initialize({
  startOnLoad: false,
  theme: "default",
  securityLevel: "loose",
});

const defaultCode = `graph TD
    A[开始] --> B{判断}
    B -->|是| C[处理]
    B -->|否| D[结束]
    C --> D`;

export const MermaidBlock = createReactBlockSpec(
  {
    type: "mermaid",
    propSchema: {
      code: {
        default: defaultCode,
      },
    },
    content: "none",
  },
  {
    render: (props) => {
      const code = props.block.props.code as string;
      const [isEditing, setIsEditing] = useState(!code);
      const [localCode, setLocalCode] = useState(code);
      const [svg, setSvg] = useState("");
      const [error, setError] = useState<string | null>(null);
      const uniqueId = useId().replace(/:/g, "_");
      const containerRef = useRef<HTMLDivElement>(null);
      // Removed textareaRef

      // 同步外部 code 变化
      useEffect(() => {
        setLocalCode(code);
      }, [code]);

      // 渲染 Mermaid 图表
      useEffect(() => {
        const codeToRender = isEditing ? localCode : code;
        if (!codeToRender) {
          setSvg("");
          return;
        }

        const renderDiagram = async () => {
          try {
            setError(null);
            await mermaid.parse(codeToRender);
            const { svg: renderedSvg } = await mermaid.render(
              `mermaid-${uniqueId}-${Date.now()}`,
              codeToRender
            );
            setSvg(renderedSvg);
          } catch (e) {
            const errorMessage =
              e instanceof Error ? e.message : "Mermaid 语法错误";
            setError(errorMessage);
            setSvg("");
          }
        };

        const timer = setTimeout(renderDiagram, 300);
        return () => clearTimeout(timer);
      }, [isEditing, localCode, code, uniqueId]);

      // 保存代码
      const handleSave = useCallback(() => {
        props.editor.updateBlock(props.block, {
          props: { code: localCode },
        });
        setIsEditing(false);
      }, [props.editor, props.block, localCode]);

      // 取消编辑
      const handleCancel = useCallback(() => {
        setLocalCode(code);
        setIsEditing(false);
      }, [code]);

      // 编辑模式
      if (isEditing) {
        return (
          <div className="w-full border border-base-300 rounded-lg overflow-hidden my-2 bg-base-100">
            {/* 图表预览区 */}
            <div className="w-full min-h-32 flex items-center justify-center p-4 bg-base-100/50">
              {error ? (
                <span className="text-error text-sm whitespace-pre-wrap font-mono">
                  {error}
                </span>
              ) : svg ? (
                <div dangerouslySetInnerHTML={{ __html: svg }} />
              ) : (
                <span className="text-base-content/50">输入代码后显示预览</span>
              )}
            </div>

            {/* 代码输入区 */}
            <div className="w-full border-t border-base-300 flex flex-col">
              {/* 标题栏 */}
              <div className="flex justify-between items-center px-3 py-2 bg-base-200 border-b border-base-300/50">
                <span className="text-xs font-bold text-base-content/70 uppercase tracking-wide">
                  Mermaid Editor
                </span>
                <a
                  href="https://mermaid.js.org/syntax/flowchart.html"
                  target="_blank"
                  rel="noopener noreferrer"
                  className="link link-primary text-xs no-underline hover:underline"
                >
                  语法参考 ↗
                </a>
              </div>

              <div
                className="w-full text-base"
                onKeyDown={(e) => {
                  if (e.key === "Escape") {
                    handleCancel();
                  } else if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) {
                    e.preventDefault();
                    handleSave();
                  }
                }}
              >
                <CodeMirror
                  value={localCode}
                  height="200px"
                  extensions={[mermaidLang()]}
                  onChange={(value) => setLocalCode(value)}
                  className="font-mono text-sm"
                  basicSetup={{
                    lineNumbers: true,
                    highlightActiveLineGutter: true,
                    foldGutter: true,
                  }}
                  // Theme could be passed here if needed, default is usually ok
                />
              </div>

              {/* 底部操作栏 */}
              <div className="flex justify-end items-center gap-2 p-2 bg-base-200 border-t border-base-300/50">
                <button className="btn btn-ghost btn-xs" onClick={handleCancel}>
                  取消
                </button>
                <button className="btn btn-primary btn-xs" onClick={handleSave}>
                  确定
                  <kbd className="kbd kbd-xs ml-1 bg-primary-content/20 text-primary-content border-none">
                    Ctrl
                  </kbd>
                  <kbd className="kbd kbd-xs bg-primary-content/20 text-primary-content border-none">
                    ↵
                  </kbd>
                </button>
              </div>
            </div>
          </div>
        );
      }

      // 预览模式
      return (
        <div
          ref={containerRef}
          className="w-full rounded-lg cursor-pointer hover:bg-base-200 transition-colors"
          onClick={() => setIsEditing(true)}
        >
          {error ? (
            <div className="text-error text-center">
              Mermaid 语法错误: {error}
            </div>
          ) : svg ? (
            <div
              className="flex justify-center"
              dangerouslySetInnerHTML={{ __html: svg }}
            />
          ) : (
            <div className="text-center text-base-content/50">
              点击添加 Mermaid 图表
            </div>
          )}
        </div>
      );
    },
  }
);
