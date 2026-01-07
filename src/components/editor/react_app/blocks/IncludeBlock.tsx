/**
 * IncludeBlock - 文件包含指令可视化编辑块
 *
 * 用于在编辑器中可视化编辑 VitePress 的 @include 指令
 * 例如: <!--@include: ./snippet.md--> 或 <!--@include: ./code.ts{1-10}-->
 */

import { createReactBlockSpec } from "@blocknote/react";
import React, { useState, useEffect, useRef, useMemo } from "react";
import { Icon } from "@iconify/react";
import { invoke } from "@tauri-apps/api/core";
import { useEditorContext, useGlobalEditorContext } from "../EditorContext";
import { contentRegistry, iconify } from "../ContentRegistry";
import { BasePopover, BaseMenu } from "../menu";

// 递归文件树节点组件
const IGNORED_NAMES = [
  "public",
  "scripts",
  "parts",
  "components",
  "assets",
  ".github",
  ".vscode",
  "README.md",
  "CONTRIBUTING.md",
  "_fireworks_preview", // 忽略预览目录
];

// 辅助函数：递归检查节点是否可见
const filterNode = (
  node: any,
  prefix: string,
  currentRelativePath: string | null
): boolean => {
  // 1. 基础过滤：忽略列表、隐藏文件、预览目录
  if (
    IGNORED_NAMES.includes(node.name) ||
    node.name.startsWith(".") ||
    node.name === "_fireworks_preview"
  ) {
    return false;
  }

  // 计算节点相对路径
  const displayPrefix =
    node._explicitPrefix !== undefined ? node._explicitPrefix : prefix;
  const nodeRelPath = displayPrefix
    ? `${displayPrefix}/${node.name}`
    : node.name;

  // 2. 文件过滤
  if (!node.children) {
    // 只显示 .md 文件
    if (!node.name.endsWith(".md")) return false;
    // 过滤掉当前正在编辑的文件
    if (currentRelativePath && nodeRelPath === currentRelativePath) {
      return false;
    }
    return true;
  }

  // 3. 文件夹过滤：如果至少有一个子节点可见，则该文件夹可见
  // 虚拟文件夹透传前缀，普通文件夹追加路径
  const childrenPrefix = node._isVirtualFolder ? displayPrefix : nodeRelPath;
  return node.children.some((child: any) =>
    filterNode(child, childrenPrefix, currentRelativePath)
  );
};

const FileTreeNode = ({
  node,
  prefix,
  currentPath,
  currentRelativePath,
  onSelect,
}: {
  node: any;
  prefix: string;
  currentPath: string;
  currentRelativePath: string | null;
  onSelect: (path: string) => void;
}) => {
  // 检查自身是否可见
  if (!filterNode(node, prefix, currentRelativePath)) {
    return null;
  }

  // 处理显式前缀（用于提升显示层级但保留路径）
  const displayPrefix =
    node._explicitPrefix !== undefined ? node._explicitPrefix : prefix;

  // 构建当前节点的相对路径
  const nodeRelPath = displayPrefix
    ? `${displayPrefix}/${node.name}`
    : node.name;
  const fullPath = `@/${nodeRelPath}`;

  // 计算子节点的前缀（如果是虚拟文件夹，则透传当前前缀）
  const childrenPrefix = node._isVirtualFolder ? displayPrefix : nodeRelPath;

  // 判断当前节点是否高亮（包括子文件被选中）
  const isActive = useMemo(() => {
    if (!currentPath) return false;
    const target = currentPath.replace(/^@\//, "");

    // 如果是普通文件夹，直接通过路径前缀判断
    if (!node._isVirtualFolder && node.children) {
      return target.startsWith(nodeRelPath + "/");
    }

    // 如果是虚拟文件夹，需要递归检查子节点
    if (node._isVirtualFolder && node.children) {
      const check = (n: any, p: string): boolean => {
        const dp = n._explicitPrefix !== undefined ? n._explicitPrefix : p;
        const np = dp ? `${dp}/${n.name}` : n.name;

        if (n._isVirtualFolder) {
          const cp = dp;
          return n.children?.some((c: any) => check(c, cp)) || false;
        }

        if (n.children) {
          return target.startsWith(np + "/");
        }

        return target === np;
      };
      // 虚拟文件夹的子节点使用 childrenPrefix (即 displayPrefix)
      return node.children.some((c: any) => check(c, childrenPrefix));
    }

    return false;
  }, [node, currentPath, nodeRelPath, childrenPrefix]);

  // 如果是目录
  if (node.children) {
    // 预过滤子节点
    const visibleChildren = node.children.filter((child: any) =>
      filterNode(child, childrenPrefix, currentRelativePath)
    );

    // 如果没有可见子节点，隐藏当前目录
    if (visibleChildren.length === 0) {
      return null;
    }

    // Hover state for submenu
    const [isHovered, setIsHovered] = useState(false);
    const hoverTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
    const triggerRef = useRef<HTMLDivElement>(null);

    const OPEN_DELAY = 50;
    const CLOSE_DELAY = 200;

    const handleMouseEnter = () => {
      if (hoverTimerRef.current) {
        clearTimeout(hoverTimerRef.current);
      }
      hoverTimerRef.current = setTimeout(() => {
        setIsHovered(true);
      }, OPEN_DELAY);
    };

    const handleMouseLeave = () => {
      if (hoverTimerRef.current) {
        clearTimeout(hoverTimerRef.current);
      }
      hoverTimerRef.current = setTimeout(() => {
        setIsHovered(false);
      }, CLOSE_DELAY);
    };

    return (
      <li onMouseEnter={handleMouseEnter} onMouseLeave={handleMouseLeave}>
        <div
          ref={triggerRef}
          tabIndex={0}
          role="button"
          className={`grid grid-cols-[auto_1fr_auto] w-full items-center gap-2 py-1.5 px-2 rounded-md border transition-all cursor-pointer ${
            isActive
              ? "border-primary/30 bg-primary/5 text-primary font-medium"
              : "border-transparent hover:border-base-content/20 hover:bg-base-200"
          }`}
        >
          <Icon
            icon="ph:folder"
            className={`h-4 w-4 flex-shrink-0 ${isActive ? "text-primary" : "text-warning"}`}
          />
          <div
            className={`truncate text-left ${isActive ? "" : "text-base-content/70 font-semibold"}`}
          >
            {node.name}
          </div>
          <Icon
            icon="ph:caret-right"
            className={`h-3 w-3 flex-shrink-0 ${isActive ? "text-primary opacity-80" : "opacity-50"}`}
          />
        </div>
        <BasePopover
          isOpen={isHovered}
          triggerElement={triggerRef.current}
          placement="right-center"
          offset={4}
        >
          <BaseMenu className="w-56">
            {visibleChildren.map((child: any) => (
              <FileTreeNode
                key={child.path || child.name}
                node={child}
                prefix={childrenPrefix}
                currentPath={currentPath}
                currentRelativePath={currentRelativePath}
                onSelect={onSelect}
              />
            ))}
          </BaseMenu>
        </BasePopover>
      </li>
    );
  }

  // 如果是文件（只显示 .md）
  if (!node.name.endsWith(".md")) return null;

  const isSelected = currentPath === fullPath;

  return (
    <li>
      <a
        className={`grid grid-cols-[auto_1fr] items-center gap-2 py-1.5 px-2 m-0.5 rounded-md border transition-all no-underline ${
          isSelected
            ? "border-primary bg-primary/10 text-primary font-medium"
            : "border-transparent hover:border-base-content/20 hover:bg-base-200"
        }`}
        onClick={(e) => {
          e.stopPropagation();
          onSelect(fullPath);
        }}
      >
        <Icon
          icon="ph:file-text"
          className={`h-4 w-4 flex-shrink-0 ${isSelected ? "text-primary" : "text-base-content/50"}`}
        />
        <div
          className="truncate text-left !whitespace-nowrap"
          style={{
            whiteSpace: "nowrap",
            overflow: "hidden",
            textOverflow: "ellipsis",
          }}
        >
          {node.name}
        </div>
      </a>
    </li>
  );
};

// 创建 IncludeBlock
// BlockNote propSchema 只支持 string | number | boolean，行范围使用 string 存储
export const IncludeBlock = createReactBlockSpec(
  {
    type: "include",
    propSchema: {
      path: {
        default: "" as const,
      },
      // 行范围使用 "start-end" 格式的字符串存储
      lineRange: {
        default: "" as const,
      },
      region: {
        default: "" as const,
      },
    },
    content: "none",
  },
  {
    render: (props) => {
      const { path, lineRange, region } = props.block.props;
      const {
        filePath: currentFilePath,
        projectRoot,
        devServerUrl,
      } = useGlobalEditorContext();

      // 计算当前文件的相对路径
      const currentRelativePath = useMemo(() => {
        if (!currentFilePath || !projectRoot) return null;

        let rel = currentFilePath;
        // Windows 兼容：大小写不敏感的前缀移除
        if (
          projectRoot &&
          rel.toLowerCase().startsWith(projectRoot.toLowerCase())
        ) {
          rel = rel.slice(projectRoot.length);
        } else {
          rel = rel.replace(projectRoot, "");
        }

        // 统一为正斜杠
        rel = rel.replace(/\\/g, "/");

        // 递归移除开头的 ./ 或 /
        return rel.replace(/^(\.?\/)+/, "");
      }, [currentFilePath, projectRoot]);

      // 解析行范围
      const parseLineRange = (
        range: string
      ): { start?: number; end?: number } => {
        if (!range) return {};
        const match = range.match(/^(\d+)?-(\d+)?$/);
        if (!match) return {};
        return {
          start: match[1] ? parseInt(match[1]) : undefined,
          end: match[2] ? parseInt(match[2]) : undefined,
        };
      };

      const { start: lineStart, end: lineEnd } = parseLineRange(lineRange);

      const [isEditing, setIsEditing] = useState(!path);
      const [localPath, setLocalPath] = useState(path);

      // 同步外部属性变化
      useEffect(() => {
        setLocalPath(path);
      }, [path]);

      // 注册执行器
      useEffect(() => {
        const blockId = props.block.id;
        // 路径输入
        contentRegistry.registerExecutor(blockId, "path", {
          execute: (val) => {
            props.editor.updateBlock(props.block, {
              props: { path: val },
            });
            setLocalPath(val);
          },
          getValue: () => path,
          isActive: () => false,
        });

        // 编辑按钮
        contentRegistry.registerExecutor(blockId, "edit", {
          execute: () => setIsEditing(true),
          isActive: () => isEditing,
        });
      }, [props.block.id, props.editor, props.block, path, isEditing]);
      const [localLineStart, setLocalLineStart] = useState<string>(
        lineStart?.toString() || ""
      );
      const [localLineEnd, setLocalLineEnd] = useState<string>(
        lineEnd?.toString() || ""
      );

      const [previewUrl, setPreviewUrl] = useState<string>("");
      const [previewError, setPreviewError] = useState<string>("");
      const [iframeHeight, setIframeHeight] = useState<number>(200);
      const iframeRef = useRef<HTMLIFrameElement>(null);

      // 生成唯一的预览 ID（需要在 message 监听器之前定义）
      const previewId = `include_${props.block.id}`;

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

      // 加载文件内容并创建预览
      useEffect(() => {
        if (!path) {
          setPreviewUrl("");
          return;
        }

        // 如果没有 devServerUrl 或 projectRoot，不创建预览
        if (!devServerUrl || !projectRoot) {
          setPreviewUrl("");
          setPreviewError("");
          return;
        }

        // 读取被包含文件的内容
        invoke<{ content: string }>("vitepress_read_document", {
          path,
          basePath: currentFilePath,
          rootPath: projectRoot,
        })
          .then((result) => {
            let content = result.content;

            // 应用行范围
            if (lineStart !== undefined || lineEnd !== undefined) {
              const lines = content.split("\n");
              const startIdx = (lineStart || 1) - 1;
              const endIdx = lineEnd || lines.length;
              content = lines.slice(startIdx, endIdx).join("\n");
            }

            // 创建预览文件 (这里不需要再次检查 projectRoot，因为上面已经检查过了)
            return invoke<string>("vitepress_create_preview", {
              projectRoot,
              previewId,
              content,
              contentType: "markdown",
            });
          })
          .then((urlPath) => {
            // 移除 baseUrl 末尾的斜杠避免双斜杠
            const baseUrl = devServerUrl?.replace(/\/$/, "") || "";
            setPreviewUrl(`${baseUrl}${urlPath}`);
            setPreviewError("");
          })
          .catch((err) => {
            console.error("Failed to create preview:", err);
            setPreviewUrl("");
            setPreviewError(`无法加载文件: ${err}`);
          });

        // 清理：组件卸载时删除预览文件
        return () => {
          if (projectRoot) {
            invoke("vitepress_delete_preview", {
              projectRoot,
              previewId,
            }).catch(() => {});
          }
        };
      }, [path, lineStart, lineEnd, projectRoot, devServerUrl]);

      // 更新 Block 属性
      const updateBlock = () => {
        const rangeStr =
          localLineStart || localLineEnd
            ? `${localLineStart || ""}-${localLineEnd || ""}`
            : "";
        props.editor.updateBlock(props.block, {
          props: {
            path: localPath,
            lineRange: rangeStr,
            region: "",
          },
        });
      };

      // 完成编辑
      const finishEditing = () => {
        if (!localPath.trim()) return;
        updateBlock();
        setIsEditing(false);
      };

      // 可选文件列表
      // 文件列表树
      const [fileTree, setFileTree] = useState<any[]>([]);
      // Dropdown 状态
      const [isFileDropdownOpen, setIsFileDropdownOpen] = useState(false);
      const fileDropdownRef = useRef<HTMLDivElement>(null);

      // 点击外部关闭 dropdown
      useEffect(() => {
        const handleClickOutside = (event: MouseEvent) => {
          if (
            fileDropdownRef.current &&
            !fileDropdownRef.current.contains(event.target as Node)
          ) {
            setIsFileDropdownOpen(false);
          }
        };

        if (isFileDropdownOpen) {
          document.addEventListener("click", handleClickOutside);
        }
        return () => {
          document.removeEventListener("click", handleClickOutside);
        };
      }, [isFileDropdownOpen]);

      // 加载项目中的 markdown 文件列表
      useEffect(() => {
        if (!projectRoot) return;

        // 使用 vitepress_get_doc_tree 获取文档树
        invoke<{ path: string; name: string; children?: any[] }>(
          "vitepress_get_doc_tree",
          {
            projectPath: projectRoot,
          }
        )
          .then((tree) => {
            if (tree.children) {
              // 1. 提取 parts 目录内容
              const partsNode = tree.children.find(
                (c: any) => c.name === "parts"
              );
              let promotedItems: any[] = [];
              if (partsNode && partsNode.children) {
                // 为 parts 下的文件添加显式前缀，以便在根目录显示时保留正确路径
                promotedItems = partsNode.children.map((child: any) => ({
                  ...child,
                  _explicitPrefix: "parts",
                }));
              }

              // 2. 将其他文件归档到 "其他文件" 虚拟文件夹
              // 过滤掉 _fireworks_preview 和 parts (parts 已被提升)
              // 注意：IGNORED_NAMES 会在 FileTreeNode 渲染时过滤，这里只需处理结构
              const otherChildren = tree.children.filter(
                (c: any) =>
                  c.name !== "_fireworks_preview" && c.name !== "parts"
              );

              const otherFilesNode = {
                name: "其他文件", // 或 "项目文件"
                nodeType: "folder",
                children: otherChildren,
                _isVirtualFolder: true, // 标记为虚拟文件夹，路径计算时透明
              };

              // 3. 组合新树结构
              setFileTree([...promotedItems, otherFilesNode]);
            } else {
              setFileTree([]);
            }
          })
          .catch((err) => {
            console.error("Failed to load file tree:", err);
            setFileTree([]);
          });
      }, [projectRoot]);

      // 编辑模式
      if (isEditing) {
        return (
          <div
            className="w-full rounded-lg border border-blue-500/30 bg-blue-500/10 p-4"
            contentEditable={false}
            onFocus={() => {
              contentRegistry.focusBlock(props.editor, props.block.id);
            }}
            onKeyDown={(e) => {
              if (e.key === "Escape") {
                setIsEditing(false);
              } else if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) {
                e.preventDefault();
                finishEditing(); // 注意：需要确保 finishEditing 内部检查了有效性
              }
            }}
          >
            <div className="mb-4 flex items-center gap-2">
              <Icon icon="mdi:file-import" className="h-5 w-5 text-blue-500" />
              <span className="font-semibold">包含文件</span>
            </div>

            {/* 文件选择与行范围 - Flex 布局 */}
            <div className="flex gap-4 mb-4 items-end">
              {/* 文件选择 - DaisyUI dropdown */}
              <div className="form-control flex-1">
                <label className="label py-1">
                  <span className="label-text text-xs font-medium">
                    选择文件
                  </span>
                </label>
                <div ref={fileDropdownRef} className="w-full">
                  <div
                    role="button"
                    className="btn btn-sm w-full justify-between border-base-300 bg-base-100"
                    onClick={(e) => {
                      e.stopPropagation();
                      setIsFileDropdownOpen(!isFileDropdownOpen);
                    }}
                  >
                    <span className={localPath ? "" : "text-base-content/50"}>
                      {localPath
                        ? localPath.replace("@/", "")
                        : "点击选择文件..."}
                    </span>
                    <Icon icon="ph:caret-down" className="h-4 w-4" />
                  </div>
                  <BasePopover
                    isOpen={isFileDropdownOpen}
                    triggerElement={fileDropdownRef.current}
                    placement="bottom-start"
                    offset={4}
                    onClickOutside={() => setIsFileDropdownOpen(false)}
                  >
                    <BaseMenu className="w-64">
                      {fileTree.length === 0 ? (
                        <li className="disabled">
                          <span className="text-base-content/50 py-2 px-2">
                            加载中...
                          </span>
                        </li>
                      ) : (
                        fileTree.map((node: any) => (
                          <FileTreeNode
                            key={node.path || node.name}
                            node={node}
                            prefix=""
                            currentPath={localPath}
                            currentRelativePath={currentRelativePath}
                            onSelect={(path) => {
                              setLocalPath(path);
                              setIsFileDropdownOpen(false);
                            }}
                          />
                        ))
                      )}
                    </BaseMenu>
                  </BasePopover>
                </div>
              </div>

              {/* 行范围 */}
              <div className="form-control">
                <label className="label py-1">
                  <span className="label-text text-xs font-medium">
                    行范围（可选）
                  </span>
                </label>
                <div className="flex items-center gap-2">
                  <input
                    type="number"
                    className="input input-bordered input-sm w-20 px-2 text-center"
                    placeholder="起始"
                    min={1}
                    value={localLineStart}
                    onChange={(e) => setLocalLineStart(e.target.value)}
                  />
                  <span className="text-base-content/60">-</span>
                  <input
                    type="number"
                    className="input input-bordered input-sm w-20 px-2 text-center"
                    placeholder="结束"
                    min={1}
                    value={localLineEnd}
                    onChange={(e) => setLocalLineEnd(e.target.value)}
                  />
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
                disabled={!localPath.trim()}
              >
                完成
                <kbd className="kbd kbd-xs ml-1 bg-primary-content/20 text-primary-content border-none">
                  Ctrl
                </kbd>
                <kbd className="kbd kbd-xs bg-primary-content/20 text-primary-content border-none">
                  ↵
                </kbd>
              </button>
            </div>
          </div>
        );
      }

      // 构建显示文本
      let displayText = `@include: ${path}`;
      if (lineStart !== undefined || lineEnd !== undefined) {
        displayText += `{${lineStart || 1}-${lineEnd || ""}}`;
      }
      if (region) {
        displayText += `#${region}`;
      }

      // 预览模式
      return (
        <div
          className="group w-full cursor-pointer rounded border border-blue-500/30 bg-blue-500/10 transition-colors overflow-hidden hover:bg-blue-500/20"
          onClick={() => setIsEditing(true)}
          contentEditable={false}
        >
          {/* 头部 */}
          <div className="flex items-center gap-2 border-b border-blue-500/20 px-3 py-2">
            <Icon
              icon="mdi:file-import"
              className="h-4 w-4 flex-shrink-0 text-blue-500"
            />
            <code className="flex-1 font-mono text-sm text-blue-700 dark:text-blue-300">
              {displayText}
            </code>
            <Icon
              icon="mdi:pencil"
              className="h-4 w-4 opacity-0 transition-opacity group-hover:opacity-100"
            />
          </div>

          {/* 预览内容 - iframe */}
          {previewUrl ? (
            <iframe
              ref={iframeRef}
              src={previewUrl}
              className="w-full border-0 bg-base-100/50"
              style={{ height: `${iframeHeight}px`, minHeight: "100px" }}
              title="Include Preview"
              sandbox="allow-scripts allow-same-origin"
            />
          ) : !previewError ? (
            <div className="flex items-center justify-center gap-2 p-4 text-sm text-base-content/50 bg-base-100/30">
              <Icon icon="ph:warning" className="h-4 w-4" />
              <span>请先启动预览服务器以查看文件预览</span>
            </div>
          ) : null}
          {previewError && (
            <div className="px-3 py-2 text-xs text-error">{previewError}</div>
          )}
        </div>
      );
    },
  }
);

contentRegistry.register("include", {
  icon: iconify("mdi:file-import"),
  label: "包含文件",
  supportedStyles: [],
  actions: [],
  slashMenuItems: [
    {
      id: "include",
      title: "文件包含",
      subtext: "插入文件包含指令",
      icon: iconify("mdi:file-import"),
      group: "VitePress",
      aliases: ["include", "import", "bh", "baohan", "yinyong"],
      blockType: "include",
      props: { path: "", lineRange: "", region: "" },
    },
  ],
});
