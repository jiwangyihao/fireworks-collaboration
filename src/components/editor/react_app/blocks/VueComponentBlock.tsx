/**
 * VueComponentBlock - Vue 组件可视化编辑块
 *
 * 用于在编辑器中可视化编辑 VitePress 文档中的 Vue 组件标签
 * 例如: <OList :data="items" /> 或 <Badge type="tip">标签</Badge>
 */

import { createReactBlockSpec } from "@blocknote/react";
import React, { useState, useEffect, useRef, useMemo } from "react";
import { Icon } from "@iconify/react";
import { invoke } from "@tauri-apps/api/core";
import { useEditorContext, useGlobalEditorContext } from "../EditorContext";
import Markdown from "react-markdown";
import { blockRegistry } from "../BlockCapabilities";

// 组件信息类型
interface ComponentProp {
  name: string;
  type_name: string;
  description?: string;
  default?: string;
  optional: boolean;
}

interface VueComponent {
  name: string;
  path: string;
  description?: string;
  props: ComponentProp[];
}

interface ComponentTreeNode {
  name: string;
  path: string;
  children?: ComponentTreeNode[];
  component?: VueComponent;
}

// 创建 VueComponentBlock
// BlockNote propSchema 只支持 string | number | boolean，所以 attributes 需要序列化为 JSON 字符串
export const VueComponentBlock = createReactBlockSpec(
  {
    type: "vueComponent",
    propSchema: {
      componentName: {
        default: "" as const,
      },
      // 将 Record<string, string> 序列化为 JSON 字符串存储
      attributesJson: {
        default: "{}" as const,
      },
      selfClosing: {
        default: true as const,
      },
    },
    content: "none",
  },
  {
    render: (props) => {
      const { componentName, attributesJson, selfClosing } = props.block.props;
      const { projectRoot, devServerUrl } = useGlobalEditorContext();

      // 解析 JSON 属性
      const parseAttributes = (json: string): Record<string, string> => {
        try {
          return JSON.parse(json || "{}");
        } catch {
          return {};
        }
      };

      const attributes = parseAttributes(attributesJson);

      const [isEditing, setIsEditing] = useState(!componentName);
      const [localName, setLocalName] = useState(componentName);

      // 同步外部属性变化
      useEffect(() => {
        setLocalName(componentName);
      }, [componentName]);

      // 注册执行器
      useEffect(() => {
        const blockId = props.block.id;
        // 组件名输入
        blockRegistry.registerExecutor(blockId, "componentName", {
          execute: (name) => {
            // 更新 Block 属性
            props.editor.updateBlock(props.block, {
              props: { componentName: name },
            });
            setLocalName(name); // 同时更新本地状态
          },
          getValue: () => componentName,
          isActive: () => false,
        });

        // 编辑按钮
        blockRegistry.registerExecutor(blockId, "edit", {
          execute: () => setIsEditing(true),
          isActive: () => isEditing,
        });
      }, [props.block.id, props.editor, props.block, componentName, isEditing]);
      const [localAttrs, setLocalAttrs] =
        useState<Record<string, string>>(attributes);
      const [newAttrKey, setNewAttrKey] = useState("");
      const [newAttrValue, setNewAttrValue] = useState("");
      const [isComponentDropdownOpen, setIsComponentDropdownOpen] =
        useState(false);
      const componentDropdownRef = useRef<HTMLDivElement>(null);

      // 可用组件列表
      const [availableComponents, setAvailableComponents] = useState<
        VueComponent[]
      >([]);

      // 加载组件列表
      useEffect(() => {
        if (!projectRoot) return;
        invoke<VueComponent[]>("vitepress_get_components", { projectRoot })
          .then(setAvailableComponents)
          .catch((e) => {
            console.error("Failed to load components:", e);
            setAvailableComponents([]);
          });
      }, [projectRoot]);

      // 预览 URL 和状态
      const [previewUrl, setPreviewUrl] = useState<string>("");
      const [iframeHeight, setIframeHeight] = useState<number>(200);
      const iframeRef = useRef<HTMLIFrameElement>(null);
      const previewId = `vue_${props.block.id}`;

      // 监听 iframe 发送的高度消息
      useEffect(() => {
        const handleMessage = (event: MessageEvent) => {
          // 只处理匹配当前块的消息
          if (
            event.data?.type === "resize" &&
            event.data?.previewId === previewId &&
            event.data?.height
          ) {
            setIframeHeight(event.data.height);
          }
        };
        window.addEventListener("message", handleMessage);
        return () => window.removeEventListener("message", handleMessage);
      }, [previewId]);

      // 尝试从后端获取组件列表
      useEffect(() => {
        // TODO: 调用 Tauri 命令获取组件列表
        // invoke<ComponentInfo[]>("vitepress_scan_components").then(setAvailableComponents);

        // 暂时使用空列表，用户可手动输入
        setAvailableComponents([]);
      }, []);

      // 创建 Vue 组件预览文件
      useEffect(() => {
        if (!componentName || !projectRoot) {
          setPreviewUrl("");
          return;
        }

        // 构建组件标签
        const attrsString = Object.entries(attributes)
          .map(([key, value]) => {
            if (
              value.includes("{{") ||
              value.match(/^[a-zA-Z_][a-zA-Z0-9_]*$/)
            ) {
              return `:${key}="${value}"`;
            }
            return `${key}="${value}"`;
          })
          .join(" ");

        const vueContent = selfClosing
          ? `<${componentName}${attrsString ? " " + attrsString : ""} />`
          : `<${componentName}${attrsString ? " " + attrsString : ""}>...</${componentName}>`;

        // 只有当 devServerUrl 和 projectRoot 存在时才创建预览文件
        if (!devServerUrl || !projectRoot) {
          setPreviewUrl("");
          return;
        }

        // 创建预览文件
        invoke<string>("vitepress_create_preview", {
          projectRoot,
          previewId,
          content: vueContent,
          contentType: "vue",
        })
          .then((urlPath) => {
            // 移除 baseUrl 末尾的斜杠避免双斜杠
            const baseUrl = devServerUrl?.replace(/\/$/, "") || "";
            setPreviewUrl(`${baseUrl}${urlPath}`);
          })
          .catch((err) => {
            console.error("Failed to create preview:", err);
            setPreviewUrl("");
          });

        // 清理
        return () => {
          invoke("vitepress_delete_preview", { projectRoot, previewId }).catch(
            () => {}
          );
        };
      }, [
        componentName,
        attributesJson,
        selfClosing,
        projectRoot,
        devServerUrl,
      ]);

      // 递归构建组件树
      // eslint-disable-next-line react-hooks/exhaustive-deps
      const componentTree = useMemo(() => {
        const root: ComponentTreeNode[] = [];
        // 排序确保一致性
        const sortedComponents = [...availableComponents].sort((a, b) =>
          a.name.localeCompare(b.name)
        );

        sortedComponents.forEach((comp) => {
          // 路径处理: components/Foo.vue -> parts: ["Foo.vue"]
          // components/Folder/Bar.vue -> parts: ["Folder", "Bar.vue"]
          const parts = comp.path.replace(/\\/g, "/").split("/");
          let currentLevel = root;

          parts.forEach((part, index) => {
            const isFile = index === parts.length - 1;
            // 如果是文件，使用组件名作为节点名，否则使用文件夹名
            const nodeName = isFile ? comp.name : part;

            let node = currentLevel.find((n) => n.name === nodeName);
            if (!node) {
              node = {
                name: nodeName,
                path: parts.slice(0, index + 1).join("/"),
                children: isFile ? undefined : [],
                component: isFile ? comp : undefined,
              };
              currentLevel.push(node);
            }

            if (!isFile && node.children) {
              currentLevel = node.children;
            }
          });
        });
        return root;
      }, [availableComponents]);

      const ComponentTreeNode = ({
        node,
        onSelect,
        currentName,
      }: {
        node: ComponentTreeNode;
        onSelect: (comp: VueComponent) => void;
        currentName: string;
      }) => {
        const isSelected = node.component?.name === currentName;

        // 文件夹节点
        if (node.children) {
          return (
            <li>
              <div className="dropdown dropdown-hover dropdown-right p-0 !bg-transparent !border-none w-full">
                <div
                  tabIndex={0}
                  role="button"
                  className="grid grid-cols-[auto_1fr_auto] w-full items-center gap-2 py-1.5 px-2 rounded-md border border-transparent hover:border-base-content/20 hover:bg-base-200 transition-all text-base-content/70 font-medium"
                >
                  <Icon icon="ph:folder" className="h-4 w-4 text-warning" />
                  <span className="truncate text-left">{node.name}</span>
                  <Icon icon="ph:caret-right" className="h-3 w-3 opacity-50" />
                </div>
                <ul
                  tabIndex={0}
                  className="dropdown-content z-[2] menu menu-xs bg-base-100 rounded-xl w-48 shadow-xl border border-base-content/10 -ml-1 p-1.5"
                  style={{
                    top: "0",
                    transform: "translateY(0)",
                  }}
                >
                  {node.children.map((child) => (
                    <ComponentTreeNode
                      key={child.path}
                      node={child}
                      onSelect={onSelect}
                      currentName={currentName}
                    />
                  ))}
                </ul>
              </div>
            </li>
          );
        }

        // 组件文件节点
        return (
          <li>
            <button
              className={`grid grid-cols-[auto_1fr] w-full items-center gap-2 py-1.5 px-2 rounded-md border transition-all ${
                isSelected
                  ? "border-primary/30 bg-primary/5 text-primary font-medium"
                  : "border-transparent hover:border-base-content/20 hover:bg-base-200 text-base-content/80"
              }`}
              onClick={() => node.component && onSelect(node.component)}
            >
              <Icon
                icon="mdi:vuejs"
                className={`h-4 w-4 ${isSelected ? "text-primary" : "text-green-500"}`}
              />
              <div className="flex flex-col items-start min-w-0 overflow-hidden">
                <span className="truncate w-full text-left">{node.name}</span>
                {node.component?.description && (
                  <span className="text-[10px] opacity-60 truncate w-full text-left font-normal">
                    {node.component.description}
                  </span>
                )}
              </div>
            </button>
          </li>
        );
      };

      // 同步本地状态到 Block
      const updateBlock = (
        name: string,
        attrs: Record<string, string>,
        closing: boolean
      ) => {
        props.editor.updateBlock(props.block, {
          props: {
            componentName: name,
            attributesJson: JSON.stringify(attrs),
            selfClosing: closing,
          },
        });
      };

      // 添加属性
      const addAttribute = () => {
        if (!newAttrKey.trim()) return;
        const updated = { ...localAttrs, [newAttrKey]: newAttrValue };
        setLocalAttrs(updated);
        setNewAttrKey("");
        setNewAttrValue("");
      };

      // 删除属性
      const removeAttribute = (key: string) => {
        const updated = { ...localAttrs };
        delete updated[key];
        setLocalAttrs(updated);
      };

      // 完成编辑
      const finishEditing = () => {
        if (!localName.trim()) return;
        updateBlock(localName, localAttrs, selfClosing);
        setIsEditing(false);
      };

      // 编辑模式
      if (isEditing) {
        const selectedComponent = availableComponents.find(
          (c) => c.name === localName
        );
        const definedPropNames =
          selectedComponent?.props.map((p) => p.name) || [];
        const extraAttrs = Object.keys(localAttrs).filter(
          (k) => !definedPropNames.includes(k)
        );

        return (
          <div
            className="rounded-lg border border-base-300 bg-base-200/50 p-4 w-full max-w-none"
            contentEditable={false}
            onFocus={() => {
              blockRegistry.focusBlock(props.editor, props.block.id);
            }}
            onKeyDown={(e) => {
              if (e.key === "Escape") {
                setIsEditing(false);
              } else if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) {
                e.preventDefault();
                finishEditing();
              }
            }}
          >
            <div className="mb-3 flex items-center gap-2">
              <Icon icon="mdi:vuejs" className="h-5 w-5 text-green-500" />
              <span className="font-semibold">插入 Vue 组件</span>
            </div>

            {/* 组件描述 */}
            {selectedComponent && selectedComponent.description && (
              <div className="mb-3 text-xs text-base-content/70 bg-base-100 rounded-md p-2 border border-base-200 prose prose-xs max-w-none">
                <Markdown>{selectedComponent.description}</Markdown>
              </div>
            )}

            {/* 组件名称与自闭合选项 - 同一行 */}
            <div className="flex gap-4 mb-4 items-end">
              {/* 组件选择 */}
              <div className="form-control flex-1">
                <label className="label py-1">
                  <span className="label-text text-xs font-medium">
                    组件名称
                  </span>
                </label>
                <div
                  ref={componentDropdownRef}
                  className={`dropdown w-full ${isComponentDropdownOpen ? "dropdown-open" : ""}`}
                >
                  <div
                    role="button"
                    className="btn btn-sm w-full justify-between border-base-300 bg-base-100"
                    onClick={(e) => {
                      e.stopPropagation();
                      setIsComponentDropdownOpen(!isComponentDropdownOpen);
                    }}
                  >
                    <span className={localName ? "" : "text-base-content/50"}>
                      {localName || "点击选择组件..."}
                    </span>
                    <Icon icon="ph:caret-down" className="h-4 w-4" />
                  </div>
                  {isComponentDropdownOpen && (
                    <ul
                      tabIndex={0}
                      className="dropdown-content z-[10] menu menu-xs bg-base-100 rounded-xl w-64 shadow-xl border border-base-content/10 mt-1 p-2"
                    >
                      {componentTree.length === 0 ? (
                        <li className="disabled">
                          <span className="text-base-content/50">
                            暂无组件或加载中...
                          </span>
                        </li>
                      ) : (
                        componentTree.map((node) => (
                          <ComponentTreeNode
                            key={node.path}
                            node={node}
                            onSelect={(c) => {
                              setLocalName(c.name);
                              setIsComponentDropdownOpen(false);
                            }}
                            currentName={localName}
                          />
                        ))
                      )}
                    </ul>
                  )}
                </div>
              </div>

              {/* 自闭合选项 */}
              <div className="form-control">
                <label className="label cursor-pointer justify-start gap-2 py-1">
                  <input
                    type="checkbox"
                    className="checkbox checkbox-sm"
                    checked={selfClosing}
                    onChange={(e) =>
                      props.editor.updateBlock(props.block, {
                        props: { selfClosing: e.target.checked },
                      })
                    }
                  />
                  <span className="label-text text-xs">自闭合 (/&gt;)</span>
                </label>
              </div>
            </div>

            {/* 自定义属性 / 已定义属性 */}
            <div className="grid grid-cols-1 gap-4 mb-3">
              {/* 如果选中了组件，显示已定义属性表单 */}
              {selectedComponent && selectedComponent.props.length > 0 && (
                <div className="col-span-full">
                  <label className="label py-1">
                    <span className="label-text text-xs font-medium">
                      组件属性
                    </span>
                  </label>
                  <div className="grid grid-cols-1 sm:grid-cols-2 gap-3 bg-base-100 p-3 rounded-md border border-base-200">
                    {selectedComponent.props.map((prop) => (
                      <div key={prop.name} className="form-control min-w-0">
                        <label className="label py-1 h-auto min-h-0 block">
                          <div className="flex items-center gap-2 min-w-0 overflow-hidden">
                            <span className="label-text text-xs font-medium whitespace-nowrap flex-shrink-0">
                              {prop.name}
                              {!prop.optional && (
                                <span
                                  className="text-error ml-0.5"
                                  title="必填"
                                >
                                  *
                                </span>
                              )}
                            </span>
                            {prop.type_name && (
                              <span className="label-text-alt opacity-50 text-[10px] whitespace-nowrap flex-shrink-0">
                                {prop.type_name}
                              </span>
                            )}
                            {prop.description && (
                              <span
                                className="text-[10px] opacity-60 text-right flex-1 min-w-0 truncate"
                                title={prop.description}
                              >
                                {prop.description}
                              </span>
                            )}
                          </div>
                        </label>
                        <input
                          className="input input-bordered input-xs w-full"
                          placeholder={
                            prop.default ? `默认: ${prop.default}` : "Value"
                          }
                          value={localAttrs[prop.name] || ""}
                          onChange={(e) => {
                            if (!e.target.value) {
                              const updated = { ...localAttrs };
                              delete updated[prop.name];
                              setLocalAttrs(updated);
                            } else {
                              setLocalAttrs({
                                ...localAttrs,
                                [prop.name]: e.target.value,
                              });
                            }
                          }}
                        />
                      </div>
                    ))}
                  </div>
                </div>
              )}
            </div>

            {/* 额外属性列表 */}
            <div className="form-control mb-3">
              <label className="label py-1">
                <span className="label-text text-xs font-medium">
                  {selectedComponent?.props.length ? "其他属性" : "属性列表"}
                </span>
              </label>
              <div className="space-y-2">
                {extraAttrs.map((key) => (
                  <div key={key} className="flex items-center gap-2">
                    <span className="badge badge-outline">{key}</span>
                    <input
                      type="text"
                      className="input input-bordered input-xs flex-1"
                      value={localAttrs[key]}
                      onChange={(e) =>
                        setLocalAttrs({
                          ...localAttrs,
                          [key]: e.target.value,
                        })
                      }
                    />
                    <button
                      className="btn btn-ghost btn-xs text-error"
                      onClick={() => removeAttribute(key)}
                    >
                      <Icon icon="mdi:close" />
                    </button>
                  </div>
                ))}

                {/* 添加新属性 */}
                <div className="flex items-center gap-2">
                  <input
                    type="text"
                    className="input input-bordered input-xs w-24"
                    placeholder="属性名"
                    value={newAttrKey}
                    onChange={(e) => setNewAttrKey(e.target.value)}
                    onKeyDown={(e) => e.key === "Enter" && addAttribute()}
                  />
                  <span>=</span>
                  <input
                    type="text"
                    className="input input-bordered input-xs flex-1"
                    placeholder="值"
                    value={newAttrValue}
                    onChange={(e) => setNewAttrValue(e.target.value)}
                    onKeyDown={(e) => e.key === "Enter" && addAttribute()}
                  />
                  <button
                    className="btn btn-ghost btn-xs text-success"
                    onClick={addAttribute}
                  >
                    <Icon icon="mdi:plus" />
                  </button>
                </div>
              </div>
            </div>

            {/* 操作按钮 */}
            <div className="flex justify-end gap-2">
              <button
                className="btn btn-ghost btn-xs"
                onClick={() => setIsEditing(false)}
              >
                取消
              </button>
              <button
                className="btn btn-primary btn-xs"
                onClick={finishEditing}
                disabled={!localName.trim()}
              >
                完成
                <kbd className="kbd kbd-xs ml-1 bg-primary-content/20 text-primary-content border-none">
                  Ctrl
                </kbd>
                <kbd className="kbd kbd-xs ml-1 bg-primary-content/20 text-primary-content border-none">
                  ↵
                </kbd>
              </button>
            </div>
          </div>
        );
      }

      // 预览模式
      const attrsString = Object.entries(attributes)
        .map(([key, value]) => {
          // 检测是否是动态绑定 (以 : 开头的值或包含变量引用)
          if (value.includes("{{") || value.match(/^[a-zA-Z_][a-zA-Z0-9_]*$/)) {
            return `:${key}="${value}"`;
          }
          return `${key}="${value}"`;
        })
        .join(" ");

      const tagContent = selfClosing
        ? `<${componentName}${attrsString ? " " + attrsString : ""} />`
        : `<${componentName}${attrsString ? " " + attrsString : ""}>...</${componentName}>`;

      return (
        <div
          className="group w-full cursor-pointer rounded border border-green-500/30 bg-green-500/10 transition-colors overflow-hidden hover:bg-green-500/20"
          onClick={() => setIsEditing(true)}
          contentEditable={false}
        >
          {/* 头部 */}
          <div className="flex items-center gap-2 border-b border-green-500/20 px-3 py-2 font-mono text-sm">
            <Icon
              icon="mdi:vuejs"
              className="h-4 w-4 flex-shrink-0 text-green-500"
            />
            <code className="flex-1 text-green-700 dark:text-green-300">
              {tagContent}
            </code>
            <Icon
              icon="mdi:pencil"
              className="h-4 w-4 opacity-0 transition-opacity group-hover:opacity-100"
            />
          </div>
          {/* iframe 预览 */}
          {previewUrl ? (
            <iframe
              ref={iframeRef}
              src={previewUrl}
              className="w-full border-0 bg-base-100/50"
              style={{ height: `${iframeHeight}px`, minHeight: "100px" }}
              title="Vue Component Preview"
              sandbox="allow-scripts allow-same-origin"
            />
          ) : (
            <div className="flex items-center justify-center gap-2 p-4 text-sm text-base-content/50 bg-base-100/30">
              <Icon icon="ph:warning" className="h-4 w-4" />
              <span>请先启动预览服务器以查看组件预览</span>
            </div>
          )}
        </div>
      );
    },
  }
);

blockRegistry.register("vueComponent", {
  icon: React.createElement(Icon, {
    icon: "mdi:vuejs",
    className: "w-4 h-4",
  }),
  label: "Vue 组件",
  supportedStyles: [],
  actions: [],
});
