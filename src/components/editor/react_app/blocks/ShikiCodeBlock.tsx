/**
 * ShikiCodeBlock - 支持 VitePress 高级语法的代码块
 *
 * 替代默认 CodeBlock，支持：
 * 1. 语言选择 (Shiki languages)
 * 2. 文件名标题 [filename]
 * 3. 行高亮 {1,3-5}
 * 4. 行号显示 :line-numbers
 * 5. 起始行号 :line-numbers=2
 * 6. Diff 模式 // [!code ++]
 *
 * 底层使用 @uiw/react-codemirror 提供编辑体验
 */

import { createReactBlockSpec } from "@blocknote/react";
import { useState, useEffect, useCallback, useMemo, useRef } from "react";
import CodeMirror, { ReactCodeMirrorRef } from "@uiw/react-codemirror";
import { Icon } from "@iconify/react";
import { DropdownMenu, MenuItem, BaseMenu } from "../menu";
import { javascript } from "@codemirror/lang-javascript";
import { html } from "@codemirror/lang-html";
import { css } from "@codemirror/lang-css";
import { json } from "@codemirror/lang-json";
import { markdown } from "@codemirror/lang-markdown";
import { python } from "@codemirror/lang-python";
import { rust } from "@codemirror/lang-rust";
import { githubLight } from "@uiw/codemirror-theme-github";
import {
  EditorView,
  Decoration,
  ViewPlugin,
  DecorationSet,
  ViewUpdate,
} from "@codemirror/view";
import { RangeSetBuilder, EditorState, Extension } from "@codemirror/state";
import { contentRegistry, iconify } from "../ContentRegistry";

// 常用语言列表
const LANGUAGES = [
  { value: "text", label: "Text", comment: { start: "// ", end: "" } },
  { value: "js", label: "JavaScript", comment: { start: "// ", end: "" } },
  { value: "ts", label: "TypeScript", comment: { start: "// ", end: "" } },
  { value: "html", label: "HTML", comment: { start: "<!-- ", end: " -->" } },
  { value: "css", label: "CSS", comment: { start: "/* ", end: " */" } },
  { value: "vue", label: "Vue", comment: { start: "<!-- ", end: " -->" } }, // Default to HTML style, script override handled dynamically
  { value: "json", label: "JSON", comment: { start: "// ", end: "" } },
  { value: "md", label: "Markdown", comment: { start: "<!-- ", end: " -->" } },
  { value: "py", label: "Python", comment: { start: "# ", end: "" } },
  { value: "rs", label: "Rust", comment: { start: "// ", end: "" } },
  { value: "sh", label: "Shell", comment: { start: "# ", end: "" } },
  { value: "yaml", label: "YAML", comment: { start: "# ", end: "" } },
] as const;

type LanguageValue = (typeof LANGUAGES)[number]["value"];

// 获取语言的默认注释风格
function getDefaultCommentStyle(lang: string) {
  const config = LANGUAGES.find((l) => l.value === lang);
  return config?.comment || { start: "// ", end: "" };
}

/**
 * 获取 CodeMirror 语言扩展
 */
function getLanguageExtension(lang: string): Extension[] {
  switch (lang) {
    case "js":
    case "ts":
      return [javascript()];
    case "vue":
    case "html":
      return [html()];
    case "css":
      return [css()];
    case "json":
      return [json()];
    case "md":
      return [markdown()];
    case "py":
      return [python()];
    case "rs":
      return [rust()];
    default:
      return [];
  }
}

/**
 * 从 EditorState 获取特定位置的注释风格 (使用标准 CodeMirror API)
 */
function getCommentStyleFromState(
  state: EditorState,
  pos: number,
  lang: string
) {
  // 1. 尝试使用标准 API 获取语言数据
  const data = state.languageDataAt<{
    line?: string;
    block?: { open: string; close: string };
  }>("commentTokens", pos);

  if (data && data.length > 0) {
    const token = data[0];
    if (token.line) {
      return { start: token.line + " ", end: "" };
    }
    if (token.block) {
      return { start: token.block.open + " ", end: " " + token.block.close };
    }
  }

  // 2. 如果获取失败，回退到默认配置
  return getDefaultCommentStyle(lang);
}

/**
 * 解析 {1,3-5} 格式的行号
 */
function parseHighlightLines(str: string): Set<number> {
  const nums = new Set<number>();
  if (!str) return nums;
  const rangeStr = str.replace(/^\{|\}$/g, "");
  if (!rangeStr.trim()) return nums;

  rangeStr.split(",").forEach((part) => {
    const p = part.trim();
    if (p.includes("-")) {
      const [start, end] = p.split("-").map(Number);
      if (!isNaN(start) && !isNaN(end)) {
        for (let i = start; i <= end; i++) nums.add(i);
      }
    } else {
      const n = Number(p);
      if (!isNaN(n)) nums.add(n);
    }
  });
  return nums;
}

/**
 * 序列化行号集合到 {1,3-5} 格式
 */
function serializeHighlightLines(nums: Set<number>): string {
  if (nums.size === 0) return "";
  const sorted = Array.from(nums).sort((a, b) => a - b);
  const parts: string[] = [];
  let start = sorted[0];
  let prev = start;

  for (let i = 1; i < sorted.length; i++) {
    const curr = sorted[i];
    if (curr === prev + 1) {
      prev = curr;
    } else {
      parts.push(start === prev ? `${start}` : `${start}-${prev}`);
      start = curr;
      prev = curr;
    }
  }
  parts.push(start === prev ? `${start}` : `${start}-${prev}`);
  return `{${parts.join(",")}}`;
}

// 样式定义扩展 (注入 CSS 变量以支持浅色/深色模式适配，这里针对 githubLight 优化)
const shikiThemeExt = EditorView.theme({
  ".cm-line": { paddingLeft: "1rem" },
  // 高亮行
  ".shiki-line-highlight": {
    backgroundColor: "#f6f8fa99",
    borderLeft: "3px solid #0969da",
  },
  // Diff Add
  ".shiki-line-diff-add": {
    backgroundColor: "#e6ffec99",
    borderLeft: "3px solid #2da44e",
  },
  // Diff Remove
  ".shiki-line-diff-remove": {
    backgroundColor: "#ffebe999",
    borderLeft: "3px solid #cf222e",
    opacity: "0.7",
  },
  // Focus
  ".shiki-line-focus": {
    backgroundColor: "#ddf4ff99",
    borderLeft: "3px solid #0969da",
  },
  // Error
  ".shiki-line-error": {
    backgroundColor: "#ffebe9",
    borderLeft: "3px solid #cf222e",
  },
  // Warning
  ".shiki-line-warning": {
    backgroundColor: "#fff8c599",
    borderLeft: "3px solid #d29922",
  },
});

// 代码组 Tab 接口
interface ShikiTab {
  code: string;
  language: string;
  filename: string;
  highlightLines: string;
  showLineNumbers: boolean;
  startLineNumber: number;
}

import React from "react";

// 模块加载时注册 shikiCode 块的能力配置
contentRegistry.register("shikiCode", {
  icon: iconify("lucide:code-2"),
  label: "代码块",
  supportedStyles: [],
  actions: [
    // 语言选择下拉菜单
    {
      type: "dropdown",
      id: "language",
      label: "语言",
      icon: iconify("lucide:code"),
    },
    // 文件/Tab 选择下拉菜单
    {
      type: "dropdown",
      id: "activeTab",
      label: "文件",
      icon: iconify("lucide:file-code"),
      iconOnly: true,
    },
    // 文件名输入
    {
      type: "input",
      id: "filename",
      label: "文件名",
      icon: iconify("lucide:file"),
      placeholder: "filename",
      width: "6rem",
      hideIcon: true,
    },
    // 行号显示切换
    {
      type: "toggle",
      id: "showLineNumbers",
      label: "行号",
      icon: iconify("lucide:list-ordered"),
    },
    // 高亮按钮
    {
      type: "button",
      id: "highlight",
      label: "高亮",
      icon: iconify("lucide:highlighter"),
    },
    {
      type: "button",
      id: "focus",
      label: "聚焦",
      icon: iconify("lucide:scan-eye"),
    },
    {
      type: "button",
      id: "++",
      label: "新增行",
      icon: iconify("lucide:plus"),
    },
    {
      type: "button",
      id: "--",
      label: "删除行",
      icon: iconify("lucide:minus"),
    },
    {
      type: "button",
      id: "error",
      label: "错误",
      icon: iconify("lucide:x-circle"),
    },
    {
      type: "button",
      id: "warning",
      label: "警告",
      icon: iconify("lucide:alert-triangle"),
    },
  ],
  slashMenuItems: [
    {
      id: "shikiCode",
      title: "代码块",
      subtext: "插入代码块",
      icon: iconify("lucide:code-2"),
      group: "基础",
      aliases: ["code", "pre", "shiki", "daima"],
      blockType: "shikiCode",
      props: { code: "", language: "text" },
    },
  ],
});

export const ShikiCodeBlock = createReactBlockSpec(
  {
    type: "shikiCode",
    propSchema: {
      code: { default: "" },
      language: { default: "text" },
      filename: { default: "" },
      highlightLines: { default: "" }, // e.g. "{1,3-5}"
      showLineNumbers: { default: false }, // boolean
      startLineNumber: { default: 1 }, // number
      // 代码组支持
      tabs: { default: "[]" }, // JSON string of ShikiTab[]
      activeTabIndex: { default: 0 },
    },
    content: "none",
  },
  {
    render: (props) => {
      const {
        code,
        language,
        filename,
        highlightLines,
        showLineNumbers,
        startLineNumber,
        tabs,
        activeTabIndex,
      } = props.block.props;

      // 内部状态 - Tabs (Source of Truth for 'Code Group')
      const [localTabs, setLocalTabs] = useState<ShikiTab[]>(() => {
        try {
          const t = JSON.parse(tabs);
          if (Array.isArray(t) && t.length) return t;
        } catch {}
        // 默认初始化: 将当前 Props 作为第一个 Tab
        return [
          {
            code,
            language,
            filename,
            highlightLines,
            showLineNumbers,
            startLineNumber,
          },
        ];
      });

      // 同步 Tabs Prop -> Local State
      useEffect(() => {
        try {
          const t = JSON.parse(tabs);
          if (Array.isArray(t) && t.length) {
            // 简单深比较避免循环? 这里只比较长度或者信任 BlockNote 的更新机制
            // 由于 BlockNote 每次更新都会重新渲染组件，我们假设 props 是最新的
            setLocalTabs(t);
            return;
          }
        } catch {}
        // Fallback
        setLocalTabs([
          {
            code,
            language,
            filename,
            highlightLines,
            showLineNumbers,
            startLineNumber,
          },
        ]);
      }, [tabs]);

      // 注册执行器到 Registry (用于 StaticToolbar 调用)
      useEffect(() => {
        const blockId = props.block.id;

        // 语言选择执行器
        contentRegistry.registerExecutor(blockId, "language", {
          execute: (lang) => {
            setLocalLang(lang);
            updateBlockRef.current?.({ language: lang });
          },
          isActive: () => false,
          getValue: () => localLangRef.current,
          getOptions: () =>
            LANGUAGES.map((l) => ({ value: l.value, label: l.label })),
        });

        // 文件/Tab 选择执行器
        contentRegistry.registerExecutor(blockId, "fileTab", {
          execute: (tabIndex) => {
            handleTabSwitchRef.current?.(parseInt(tabIndex));
          },
          isActive: () => false,
          getValue: () => String(activeTabIndexRef.current),
          getOptions: () =>
            localTabsRef.current.map((tab, i) => ({
              value: String(i),
              label: tab.filename || `File ${i + 1}`,
            })),
        });

        // 文件名输入执行器
        contentRegistry.registerExecutor(blockId, "filename", {
          execute: (name) => {
            setLocalFilename(name);
            updateBlockRef.current?.({ filename: name });
          },
          isActive: () => false,
          getValue: () => localFilenameRef.current,
        });

        // 行号开关执行器
        contentRegistry.registerExecutor(blockId, "lineNumbers", {
          execute: (val) => {
            const newVal =
              typeof val === "boolean" ? val : !localShowLineNumbersRef.current;
            setLocalShowLineNumbers(newVal);
            updateBlockRef.current?.({ showLineNumbers: newVal });
          },
          isActive: () => localShowLineNumbersRef.current,
          getValue: () => localShowLineNumbersRef.current,
        });

        // 注册标注按钮的执行器
        const annotationActions = [
          "highlight",
          "focus",
          "++",
          "--",
          "error",
          "warning",
        ] as const;
        annotationActions.forEach((actionId) => {
          contentRegistry.registerExecutor(blockId, actionId, {
            execute: () => toggleAnnotationRef.current?.(actionId),
            isActive: () => activeAnnotationRef.current === actionId,
          });
        });

        return () => {
          contentRegistry.unregisterExecutors(blockId);
        };
      }, [props.block.id]);

      // 内部 UI 状态 (用于编辑器绑定和快速反馈)
      const [localCode, setLocalCode] = useState(code);
      const [localLang, setLocalLang] = useState(language);
      const [localFilename, setLocalFilename] = useState(filename);
      const [localHighlight, setLocalHighlight] = useState(highlightLines);
      const [localShowLineNumbers, setLocalShowLineNumbers] =
        useState(showLineNumbers);

      // 当前选中行的状态 (用于 Toolbar 高亮显示)
      const [activeAnnotation, setActiveAnnotation] = useState<string | null>(
        null
      );

      // Refs for registry callbacks (avoid stale closure)
      const activeAnnotationRef = useRef(activeAnnotation);
      activeAnnotationRef.current = activeAnnotation;

      const localLangRef = useRef(localLang);
      localLangRef.current = localLang;

      const localFilenameRef = useRef(localFilename);
      localFilenameRef.current = localFilename;

      const localShowLineNumbersRef = useRef(localShowLineNumbers);
      localShowLineNumbersRef.current = localShowLineNumbers;

      const activeTabIndexRef = useRef(activeTabIndex);
      activeTabIndexRef.current = activeTabIndex;

      // UI State: File Switcher Menu
      const [showFileMenu, setShowFileMenu] = useState(false);
      const [showLangMenu, setShowLangMenu] = useState(false);

      const editorRef = useRef<ReactCodeMirrorRef>(null);
      const langButtonRef = useRef<HTMLDivElement>(null);
      const fileButtonRef = useRef<HTMLDivElement>(null);

      // 同步 Props -> UI State (当 activeTabIndex 切换或 Props 外部更新时)
      useEffect(() => {
        setLocalCode(code);
        setLocalLang(language);
        setLocalFilename(filename);
        setLocalHighlight(highlightLines);
        setLocalShowLineNumbers(showLineNumbers);
      }, [code, language, filename, highlightLines, showLineNumbers]);

      // 保存更改 (同时更新 Props 和 Tabs JSON)
      const updateBlock = useCallback(
        (changes: Partial<typeof props.block.props>) => {
          setLocalTabs((prev) => {
            const idx = activeTabIndex;
            const newTabs = [...prev];

            if (newTabs[idx]) {
              // 过滤掉 tabs 和 activeTabIndex 避免递归或污染
              const {
                tabs: _t,
                activeTabIndex: _a,
                ...validChanges
              } = changes as any;
              newTabs[idx] = { ...newTabs[idx], ...validChanges };
            }

            props.editor.updateBlock(props.block, {
              props: {
                ...changes,
                tabs: JSON.stringify(newTabs),
              },
            });
            return newTabs;
          });
        },
        [props.editor, props.block, activeTabIndex]
      );

      // Ref for updateBlock (used by executors)
      const updateBlockRef = useRef(updateBlock);
      updateBlockRef.current = updateBlock;

      // Tab 操作处理器
      const handleTabSwitch = (index: number) => {
        if (index < 0 || index >= localTabs.length) return;
        const target = localTabs[index];

        props.editor.updateBlock(props.block, {
          props: {
            activeTabIndex: index,
            ...target, // 将目标 Tab 的属性提升到 Block Props，实现“切换视图”
          },
        });

        // 通知工具栏重新渲染（更新文件下拉菜单选中项）
        contentRegistry.notify();
      };

      // Refs for handleTabSwitch and localTabs (used by executors)
      const handleTabSwitchRef = useRef(handleTabSwitch);
      handleTabSwitchRef.current = handleTabSwitch;

      const localTabsRef = useRef(localTabs);
      localTabsRef.current = localTabs;

      const handleAddTab = () => {
        const newTab: ShikiTab = {
          code: "",
          language: "text",
          filename: `File ${localTabs.length + 1}`,
          highlightLines: "",
          showLineNumbers: false,
          startLineNumber: 1,
        };
        const newTabs = [...localTabs, newTab];
        const newIndex = newTabs.length - 1;

        props.editor.updateBlock(props.block, {
          props: {
            tabs: JSON.stringify(newTabs),
            activeTabIndex: newIndex,
            ...newTab, // 立即切换到新 Tab
          },
        });
      };

      const handleRemoveTab = (index: number, e: React.MouseEvent) => {
        e.stopPropagation();
        if (localTabs.length <= 1) return;

        const newTabs = localTabs.filter((_, i) => i !== index);
        let newIndex = activeTabIndex;

        // 如果删除的是当前 Tab 或之前的 Tab，需要调整 Active Index
        if (index === activeTabIndex) {
          newIndex = Math.max(0, activeTabIndex - 1);
        } else if (index < activeTabIndex) {
          newIndex = activeTabIndex - 1;
        }

        // 确保 index 合法
        newIndex = Math.min(newIndex, newTabs.length - 1);
        const target = newTabs[newIndex];

        props.editor.updateBlock(props.block, {
          props: {
            tabs: JSON.stringify(newTabs),
            activeTabIndex: newIndex,
            ...target,
          },
        });
      };

      // 防抖保存内容
      useEffect(() => {
        const timer = setTimeout(() => {
          if (localCode !== code) {
            updateBlock({ code: localCode });
          }
        }, 300);
        return () => clearTimeout(timer);
      }, [localCode, code, updateBlock]);

      // 行高亮装饰器插件 (ViewPlugin)
      const highlightPlugin = useMemo(() => {
        const highlightSet = parseHighlightLines(localHighlight);

        return ViewPlugin.fromClass(
          class {
            decorations: DecorationSet;
            constructor(view: EditorView) {
              this.decorations = this.compute(view);
            }
            update(update: ViewUpdate) {
              if (update.docChanged || update.viewportChanged)
                this.decorations = this.compute(update.view);
            }
            compute(view: EditorView) {
              const builder = new RangeSetBuilder<Decoration>();
              for (const { from, to } of view.visibleRanges) {
                // 遍历可见行
                for (let pos = from; pos <= to; ) {
                  const line = view.state.doc.lineAt(pos);
                  const lineText = line.text;
                  const lineNo = line.number;

                  let className = "";

                  // 1. 优先检查行内注释标记
                  if (lineText.includes("[!code ++]"))
                    className = "shiki-line-diff-add";
                  else if (lineText.includes("[!code --]"))
                    className = "shiki-line-diff-remove";
                  else if (lineText.includes("[!code focus]"))
                    className = "shiki-line-focus";
                  else if (lineText.includes("[!code highlight]"))
                    className = "shiki-line-highlight";
                  else if (lineText.includes("[!code error]"))
                    className = "shiki-line-error";
                  else if (lineText.includes("[!code warning]"))
                    className = "shiki-line-warning";

                  // 2. 检查 highlightLines Prop (如果未被 Comment 覆盖)
                  if (!className && highlightSet.has(lineNo)) {
                    className = "shiki-line-highlight";
                  }

                  if (className) {
                    builder.add(
                      line.from,
                      line.from,
                      Decoration.line({ class: className })
                    );
                  }
                  pos = line.to + 1;
                }
              }
              return builder.finish();
            }
          },
          {
            decorations: (v) => v.decorations,
          }
        );
      }, [localHighlight]); // 依赖 localHighlight，当它变化时重建插件

      // 监听光标和内容变化，更新 Toolbar 状态
      const selectionUpdatePlugin = useMemo(() => {
        return EditorView.updateListener.of((update) => {
          if (update.selectionSet || update.docChanged) {
            const { state } = update.view;
            const line = state.doc.lineAt(state.selection.main.from);
            const text = line.text;
            const lineNo = line.number;

            // 检测注释类型
            // 匹配任何形式的 [!code type]
            const match = text.match(/\[\!code\s+(.*?)\]/);
            let newAnnotation: string | null = null;

            if (match) {
              newAnnotation = match[1].trim();
            } else {
              // 检测 Prop Highlight
              const set = parseHighlightLines(localHighlight);
              if (set.has(lineNo)) {
                newAnnotation = "highlight";
              }
            }

            // 更新状态并通知 registry（触发 StaticToolbar 重渲染）
            // 注意：先同步更新 ref，再通知（因为 React state 是异步的）
            activeAnnotationRef.current = newAnnotation;
            setActiveAnnotation(newAnnotation);
            contentRegistry.notify();
          }
        });
      }, [localHighlight]); // 当 localHighlight 变化时，也需要重新评估当前状态

      // 语言扩展加载
      const extensions = useMemo(() => {
        const langs = [];
        switch (localLang) {
          case "js":
          case "ts":
            langs.push(javascript());
            break;
          case "vue":
          case "html":
            langs.push(html());
            break;
          case "css":
            langs.push(css());
            break;
          case "json":
            langs.push(json());
            break;
          case "md":
            langs.push(markdown());
            break;
          case "py":
            langs.push(python());
            break;
          case "rs":
            langs.push(rust());
            break;
        }
        return [
          ...langs,
          shikiThemeExt,
          highlightPlugin,
          selectionUpdatePlugin,
          EditorView.lineWrapping, // 自动换行
        ];
      }, [localLang, highlightPlugin, selectionUpdatePlugin]);

      /**
       * 切换语言时的处理：更新代码中的注释格式 (使用标准化 CodeMirror API)
       */
      const handleLanguageChange = (newLang: string) => {
        setLocalLang(newLang);
        updateBlock({ language: newLang });

        // 创建临时 EditorState 以模拟新语言环境下的解析
        // 这样可以利用 CodeMirror 准确的 languageDataAt 功能
        const tempState = EditorState.create({
          doc: localCode,
          extensions: getLanguageExtension(newLang),
        });

        const lines = localCode.split("\n");
        let currentPos = 0; // 累计字符位置

        const newLines = lines.map((line) => {
          const lineEndPos = currentPos + line.length;

          const match = line.match(/\[\!code\s+(.*?)\]/);
          if (match) {
            const type = match[1].trim();

            // 使用标准 API 查询该位置的注释风格
            const targetStyle = getCommentStyleFromState(
              tempState,
              lineEndPos,
              newLang
            );
            const markerFull = `${targetStyle.start}[!code ${type}]${targetStyle.end}`;

            // 替换旧标记
            const regex =
              /(\/\/|#|<!--|\/\*)\s*\[!code\s+.*?\](\s*-->|\s*\*\/)?/;
            currentPos += line.length + 1; // +1 for newline
            if (line.match(regex)) {
              return line.replace(regex, markerFull);
            } else {
              return line.replace(/\[\!code\s+.*?\]/, `[!code ${type}]`);
            }
          }
          currentPos += line.length + 1;
          return line;
        });

        const newCode = newLines.join("\n");
        if (newCode !== localCode) {
          setLocalCode(newCode);
          updateBlock({ code: newCode });
        }
      };

      /**
       * 统一管理行注释标记
       */
      const toggleAnnotation = (
        type: "highlight" | "focus" | "++" | "--" | "error" | "warning"
      ) => {
        const view = editorRef.current?.view;
        if (!view) return;
        const { state, dispatch } = view;
        const line = state.doc.lineAt(state.selection.main.from);
        const lineText = line.text;

        // 获取当前上下文的正确样式 (使用标准化 API)
        const style = getCommentStyleFromState(state, line.to, localLang);
        const markerCore = `[!code ${type}]`;
        const markerFull = `${style.start}${markerCore}${style.end}`;

        // 检测行内是否已有标记
        const coreRegex = /\[\!code\s+(.*?)\]/;
        const match = lineText.match(coreRegex);

        let newText = lineText;
        let shouldDispatch = true;

        const set = parseHighlightLines(localHighlight);
        let propsChanged = false;

        if (match) {
          const currentType = match[1].trim();

          // 构造能够匹配当前行实际存在的完整注释的正则
          // 这里的策略是：不管当前的注释符号是什么（可能是旧的，或者不规范的），只要包含了 [!code ...] 就尝试整体移除
          const fullMarkerRegex =
            /(\/\/|#|<!--|\/\*)\s*\[!code\s+.*?\](\s*-->|\s*\*\/)?/;
          const fullMatch = lineText.match(fullMarkerRegex);
          const targetToReplace = fullMatch ? fullMatch[0] : match[0];

          if (currentType === type) {
            // 相同类型 -> 移除
            // 如果 targetToReplace 前面有空格，replace 默认只删字符串本身
            // 我们可以再多删一点前导空格以保持整洁，但 .trimEnd() 通常够用
            newText = lineText.replace(targetToReplace, "").trimEnd();
          } else {
            // 不同类型 -> 替换
            // 使用当前上下文正确的 markerFull 替换旧的 targetToReplace
            newText = lineText.replace(targetToReplace, markerFull);

            // 互斥清理 Prop
            if (set.has(line.number)) {
              set.delete(line.number);
              propsChanged = true;
            }
          }
        } else {
          // 无标记
          if (set.has(line.number)) {
            set.delete(line.number);
            propsChanged = true;

            if (type === "highlight") {
              shouldDispatch = false;
            } else {
              newText = `${lineText} ${markerFull}`;
            }
          } else {
            newText = `${lineText} ${markerFull}`;
          }
        }

        if (propsChanged) {
          const newHighlight = serializeHighlightLines(set);
          updateBlock({ highlightLines: newHighlight });
        }

        if (shouldDispatch) {
          // 计算光标在行内的相对偏移，以便在文本变化后保持合理位置
          const cursorOffsetInLine = Math.min(
            state.selection.main.from - line.from,
            newText.length
          );
          const newAnchor = line.from + cursorOffsetInLine;

          dispatch({
            changes: { from: line.from, to: line.to, insert: newText },
            selection: { anchor: newAnchor },
          });
          // 强制同步
          setLocalCode(view.state.doc.toString());
        }
      };

      // Ref for toggleAnnotation (used by registry executors)
      const toggleAnnotationRef = useRef(toggleAnnotation);
      toggleAnnotationRef.current = toggleAnnotation;

      // 辅助函数：判断按钮是否激活
      const isActive = (type: string) => activeAnnotation === type;

      return (
        <div className="w-full rounded-lg border border-base-200 bg-white shadow-sm group">
          {/* Header / Toolbar */}
          <div className="flex flex-wrap items-center gap-1 p-1.5 bg-gray-50/50 border-b border-gray-100 text-xs rounded-t-lg">
            {/* Language Selector */}
            <div className="relative z-20" ref={langButtonRef}>
              <div
                role="button"
                className="btn btn-xs btn-ghost gap-1 h-6 min-h-0 px-2 font-normal text-gray-600 hover:bg-gray-200 border-transparent hover:border-gray-300"
                onClick={() => setShowLangMenu(!showLangMenu)}
                title="Change Language"
              >
                <span className="font-mono">
                  {LANGUAGES.find((l) => l.value === localLang)?.label ||
                    "Text"}
                </span>
                <Icon
                  icon="lucide:chevron-down"
                  className="w-3 h-3 opacity-50"
                />
              </div>
              <DropdownMenu
                isOpen={showLangMenu}
                triggerElement={langButtonRef.current}
                position="bottom-left"
                onClose={() => setShowLangMenu(false)}
              >
                {LANGUAGES.map((lang) => (
                  <MenuItem
                    key={lang.value}
                    label={lang.label}
                    active={localLang === lang.value}
                    onClick={() => {
                      handleLanguageChange(lang.value);
                      setShowLangMenu(false);
                    }}
                  />
                ))}
              </DropdownMenu>
            </div>

            {/* Filename Input & Switcher */}
            <div className="flex items-center text-gray-400 relative z-10 gap-1">
              {/* Dropdown Toggle */}
              <div className="relative" ref={fileButtonRef}>
                <div
                  role="button"
                  className={`btn btn-xs btn-ghost gap-1 h-6 min-h-0 px-2 text-gray-600 hover:bg-gray-200 border-transparent hover:border-gray-300 ${localTabs.length > 1 ? "text-blue-600" : ""}`}
                  onClick={() => setShowFileMenu(!showFileMenu)}
                  title="Switch or Add File"
                >
                  <Icon
                    icon={localTabs.length > 1 ? "lucide:files" : "lucide:file"}
                    className="w-3.5 h-3.5"
                  />
                  <Icon
                    icon="lucide:chevron-down"
                    className="w-3 h-3 opacity-50"
                  />
                </div>

                {/* Dropdown Menu using DropdownMenu */}
                <DropdownMenu
                  isOpen={showFileMenu}
                  triggerElement={fileButtonRef.current}
                  position="bottom-left"
                  width={144}
                  onClose={() => setShowFileMenu(false)}
                >
                  {localTabs.map((tab, idx) => (
                    <MenuItem
                      key={idx}
                      icon="lucide:file"
                      label={tab.filename || "Untitled"}
                      active={idx === activeTabIndex}
                      onClick={() => {
                        handleTabSwitch(idx);
                        setShowFileMenu(false);
                      }}
                      rightContent={
                        localTabs.length > 1 ? (
                          <button
                            className="btn btn-ghost btn-xs btn-square h-5 w-5 min-h-0 hover:bg-error/20 hover:text-error"
                            onClick={(e) => {
                              e.stopPropagation();
                              handleRemoveTab(idx, e);
                            }}
                            title="Delete File"
                          >
                            <Icon icon="lucide:trash-2" className="w-3 h-3" />
                          </button>
                        ) : undefined
                      }
                    />
                  ))}
                  {/* Add New File Button */}
                  <li className="w-full border-t border-base-content/10 mt-1 pt-1">
                    <a
                      role="button"
                      className="flex items-center gap-2 py-1.5 px-2 rounded-md text-primary hover:bg-primary/10 cursor-pointer transition-all border border-transparent hover:border-base-content/20"
                      onClick={() => {
                        handleAddTab();
                        setShowFileMenu(false);
                      }}
                    >
                      <Icon icon="lucide:plus" className="w-4 h-4" />
                      <span>Add New File</span>
                    </a>
                  </li>
                </DropdownMenu>
              </div>

              <input
                type="text"
                className="input input-ghost input-xs h-6 min-h-0 w-32 px-1 focus:bg-white placeholder:text-gray-300 font-mono text-gray-600 focus:w-48 transition-all focus:outline-none focus:ring-1 focus:ring-blue-200 rounded-sm"
                placeholder="filename.ext"
                value={localFilename}
                onChange={(e) => {
                  setLocalFilename(e.target.value);
                  updateBlock({ filename: e.target.value });
                }}
              />
            </div>

            {/* Annotation Tools */}
            <div className="flex items-center gap-0.5">
              {/* Highlighter */}
              <button
                className={`btn btn-xs btn-square h-6 w-6 transition-colors ${isActive("highlight") ? "bg-amber-100 text-amber-600" : "btn-ghost text-gray-500 hover:bg-amber-50 hover:text-amber-500"}`}
                onClick={() => toggleAnnotation("highlight")}
                title="Highlight Line"
              >
                <Icon icon="lucide:highlighter" className="w-3.5 h-3.5" />
              </button>

              {/* Focus */}
              <button
                className={`btn btn-xs btn-square h-6 w-6 transition-colors ${isActive("focus") ? "bg-blue-100 text-blue-600" : "btn-ghost text-blue-500 hover:bg-blue-50"}`}
                onClick={() => toggleAnnotation("focus")}
                title="Focus Line"
              >
                <Icon icon="lucide:scan-eye" className="w-3.5 h-3.5" />
              </button>

              {/* Diff +/- */}
              <button
                className={`btn btn-xs btn-square h-6 w-6 transition-colors ${isActive("++") ? "bg-green-100 text-green-600" : "btn-ghost text-green-600 hover:bg-green-50"}`}
                onClick={() => toggleAnnotation("++")}
                title="Add Line (Diff +)"
              >
                <Icon icon="lucide:plus" className="w-3.5 h-3.5" />
              </button>
              <button
                className={`btn btn-xs btn-square h-6 w-6 transition-colors ${isActive("--") ? "bg-red-100 text-red-600" : "btn-ghost text-red-500 hover:bg-red-50"}`}
                onClick={() => toggleAnnotation("--")}
                title="Remove Line (Diff -)"
              >
                <Icon icon="lucide:minus" className="w-3.5 h-3.5" />
              </button>

              {/* Error / Warning */}
              <button
                className={`btn btn-xs btn-square h-6 w-6 transition-colors ${isActive("error") ? "bg-red-100 text-red-600" : "btn-ghost text-red-600 hover:bg-red-50"}`}
                onClick={() => toggleAnnotation("error")}
                title="Error"
              >
                <Icon icon="lucide:x-circle" className="w-3.5 h-3.5" />
              </button>
              <button
                className={`btn btn-xs btn-square h-6 w-6 transition-colors ${isActive("warning") ? "bg-amber-100 text-amber-600" : "btn-ghost text-amber-600 hover:bg-amber-50"}`}
                onClick={() => toggleAnnotation("warning")}
                title="Warning"
              >
                <Icon icon="lucide:alert-triangle" className="w-3.5 h-3.5" />
              </button>
            </div>

            <div className="flex-1"></div>

            {/* Line Numbers Toggle */}
            <button
              className={`btn btn-xs btn-ghost btn-square h-6 w-6 ${localShowLineNumbers ? "text-primary bg-primary/5" : "text-gray-400"}`}
              onClick={() => {
                const newVal = !localShowLineNumbers;
                setLocalShowLineNumbers(newVal);
                updateBlock({ showLineNumbers: newVal });
              }}
              title="Toggle Line Numbers"
            >
              <Icon icon="ph:list-numbers" className="w-4 h-4" />
            </button>
          </div>

          {/* Editor */}
          <div
            className="relative font-mono text-sm leading-6 rounded-b-lg overflow-hidden"
            style={{ backgroundColor: "#ffffff" }}
          >
            <CodeMirror
              ref={editorRef}
              value={localCode}
              height="auto"
              theme={githubLight}
              extensions={extensions}
              onChange={(val) => setLocalCode(val)}
              onFocus={() => {
                // 使用 blockRegistry 辅助方法同步选区
                contentRegistry.focusBlock(
                  props.editor,
                  props.block.id,
                  "start"
                );
              }}
              basicSetup={{
                lineNumbers: localShowLineNumbers,
                foldGutter: true,
                highlightActiveLine: false,
                // 禁用默认的高亮，避免与 custom highlighter 冲突
              }}
              className="cm-shiki-editor"
            />
          </div>
        </div>
      );
    },
  }
);
