# E2 阶段实施交接文档：编辑器核心与 Markdown 深度适配 (E2 Phase Implementation Handoff)

> **文档密级**：内部技术资料
> **适用对象**：前端开发组、架构师
> **最后更新**：2026-01-07
> **版本**：v2.5 (E2.5 UI/UX 深度定制: Toolbar/SlashMenu/Icons)

---

## 1. 实施概览 (Implementation Overview)

### 1.1 项目背景与目标

本项目旨在构建一个能够完美兼容 VitePress 生态的 WYSIWYG（所见即所得）Markdown 编辑器。E2 阶段的核心挑战在于解决 **BlockNote (ProseMirror)** 的数据模型与 **Markdown (AST)** 之间的 "Impedance Mismatch"（阻抗失配），特别是针对 VitePress 特有的自定义容器 (`::: tip`)、复杂公式 (`$$`) 以及 Vue 组件的无损编辑能力。

### 1.2 关键成果摘要

- **架构突破**：成功实现了基于 React (编辑器) + Vue (宿主) 的混合架构，解决了跨框架状态同步导致的死循环问题。
- **深度适配**：开发了专用的 AST 转换中间件，实现了 BlockNote 缺失的 Markdown 特性（行内块级公式、自定义容器、Mermaid 图表），且做到了 100% 数据回写保真。
- **交互体验**：引入了 MathLive 和 CodeMirror 等重型组件，将专业领域的编辑体验带入通用文本编辑器中。
- **UI/UX 重构 (E2.5)**：全面接管 BlockNote 默认 UI，实现了基于 Iconify 的统一图标系统、高度定制的 Slash Menu (中文优化) 以及可扩展的 Toolbar Action 注册机制。

**完成状态**：✅ E2.1-E2.5 全阶段交付闭环

---

## 2. 交付物与架构职责 (Deliverables & Architecture Map)

### 2.1 核心适配层 (Adapter Layer - The Bridge)

这一层负责抹平 ProseMirror Node 与 Markdown AST 之间的差异，是数据一致性的守门人。

| 文件路径                          | 核心导出                                     | 职责描述                                                                                                                                                                                                                      |
| :-------------------------------- | :------------------------------------------- | :---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `src/utils/markdown-to-blocks.ts` | `markdownToBlocks(markdown)`                 | **Markdown 解析器**<br>1. 配置 `unified` + `remark` 管道。<br>2. 注入 `remarkEnrichMath` 标记公式类型。<br>3. 遍历 AST，将 `containerDirective` 降维打击为扁平化的 Block 结构。<br>4. [E2.3] 优化 Switch 分支，消除重复逻辑。 |
| `src/utils/blocks-to-markdown.ts` | `blocksToMarkdown(blocks)`                   | **Markdown 生成引擎**<br>1. 将 BlockNote 的 JSON 模型序列化为 AST。<br>2. 智能识别 `displayMode`，精确还原 `$$` 与 `$`。<br>3. 处理 Container Block 的缩进和空行，确保生成的 Markdown 符合 Prettier 规范。                    |
| `src/utils/blocknote-adapter.ts`  | `internalToBlockNote`, `blockNoteToInternal` | **数据模型转换器**<br>负责项目内部通用 Block 模型 (E0) 与 BlockNote 特定 Schema 之间的转换。处理 Table 结构的复杂映射逻辑（Cell 内容提取与封装）。                                                                            |

### 2.2 编辑器 UI 组件层 (Editor UI Layer)

构建在 `@blocknote/react` 之上的定制化 UI 系统。

| 文件路径                                                | 关键组件/配置             | 职责描述                                                                                                                                             |
| :------------------------------------------------------ | :------------------------ | :--------------------------------------------------------------------------------------------------------------------------------------------------- |
| `src/components/editor/react_app/BlockNoteEditor.tsx`   | `<BlockNoteView />`       | **编辑器入口**<br>配置 Editor 实例，挂载 Schema，初始化 `SlashMenuItems` 和 `SideMenu`。                                                             |
| `src/components/editor/react_app/SlashMenuItems.tsx`    | `getCustomSlashMenuItems` | **指令集配置中心**<br>定义 `/` 菜单的各项指令。实现了基于**拼音别名**的混合搜索算法（如 `/gs` -> 公式），极大提升了中文输入体验。                    |
| `src/components/editor/react_app/StaticToolbar.tsx`     | `<StaticToolbar />`       | **自定义工具栏** (E2.5)<br>完全重写默认 Toolbar，移除冗余样式按钮，集成 `BlockCapabilities` 动态动作系统，统一使用 Iconify 图标。                    |
| `src/components/editor/react_app/blocks/InlineMath.tsx` | `<InlineMath />`          | **行内公式组件**<br>封装 `math-field` Web Component。**关键贡献**：实现了捕获阶段的键盘事件拦截，解决了 ProseMirror 光标无法进入 Shadow DOM 的难题。 |
| `src/components/editor/react_app/BlockCapabilities.ts`  | `blockRegistry`           | **能力注册表** (E2.5)<br>集中管理所有 Block 的元数据（图标、Label）及上下文 Toolbar Actions（如公式的“键盘/菜单”切换按钮）。                         |

### 2.3 宿主集成层 (Host Integration Layer)

Vue 应用与 React 编辑器的粘合剂。

| 文件路径                                | 核心逻辑              | 职责描述                                                                                                                                                 |
| :-------------------------------------- | :-------------------- | :------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `src/components/editor/BlockEditor.vue` | `applyPureReactInVue` | **跨框架桥接**<br>使用 `veaury` 将 React 组件包装为 Vue 组件。负责处理 `onChange` 事件的防抖和 Props 的深比较。                                          |
| `src/views/DocumentView.vue`            | `staticInitialBlocks` | **视图控制器**<br>实现了 "Static Snapshot" 策略：仅在首次加载时传递数据给 React，切断后续的响应式链路，防止 Vue 的 Proxy 机制触发 React 的异常 Remount。 |

### 2.4 数据模型定义 (Type Definitions)

| 文件路径             | 定义内容            | 说明                                                                                                                                   |
| :------------------- | :------------------ | :------------------------------------------------------------------------------------------------------------------------------------- |
| `src/types/block.ts` | `ShikiCodeBlock` 等 | **通用数据契约**<br>定义了系统内流转的所有 Block 类型。E2.4 新增 `ShikiCodeBlock` 以支持包含丰富元数据（文件名、高亮、Diff）的代码块。 |

---

## 3. 关键技术实现详解

### 3.1 Vue/React 混合架构下的响应式死锁 (The Reactive Loop Deadlock)

**根因深度剖析**：
在 Vue (宿主) 和 React (编辑器) 的混合架构中，存在两套独立的状态管理机制：

1.  **Vue Reactivity**: `docStore.currentBlocks` 是一个 Proxy 对象。
2.  **ProseMirror Model**: `editor.topNode` 是一个不可变的数据结构。

当我们将 `docStore.currentBlocks` 作为 `initialContent` 传递给 React 组件时，发生了一个典型的 **"Props vs Local State"** 冲突：

1.  用户输入字符 -> 触发 `editor.onChange`。
2.  `handleEditorChange` 回调 -> 更新 Vue `docStore.currentBlocks`。
3.  Vue 检测到 Props 变化 -> 重新渲染 `BlockEditor` 组件。
4.  React 检测到 `initialContent` Prop 变化 -> **强制重置 (Remount)** 编辑器实例。
5.  **结果**：用户每输入一个字，编辑器就重置一次，光标丢失。

**架构级解决方案：静态快照隔离 (Static Snapshot Isolation)**

我们摒弃了传统的 "Controlled Component" (受控组件) 模式，转而采用 "Uncontrolled with Initializer" 模式。

```typescript
// src/views/DocumentView.vue

// 1. 定义非响应式的“静态快照”
// 使用 ref 但在该逻辑中仅作为一次性容器，或者直接使用普通变量
const staticInitialBlocks = ref<any[]>([]);

// 2. 仅在文档首次加载完成（Hydration）的瞬间抓取数据
watch(loadingContent, (isLoading) => {
  if (!isLoading && docStore.currentBlocks) {
    // 关键操作：Deep Clone + JSON Serialization
    // 这不仅切断了 Proxy 引用，还过滤掉了 Vue 在对象上附加的 Observer 属性
    staticInitialBlocks.value = JSON.parse(JSON.stringify(docStore.currentBlocks));
  }
});

// 3. 强制 React 仅在文件 ID 变更时才重置
// 通过 :key 绑定文件路径，告诉 Vue："只有换文件了，才重建这个组件"
<BlockEditor
  :key="selectedPath || 'editor'"
  :initial-content="staticInitialBlocks"
  @change="handleEditorChange"
/>
```

这种设计确保了单向数据流的稳定性：`Load -> Init -> User Type -> Save`，除非用户切换文件，否则 Vue 不会干涉 React 内部的状态。

### 3.2 完美公式保真度 (Pixel-Perfect Math Fidelity)

**挑战**：Markdown 的公式语法丰富（行内 `$`, 块级 `$$`），而 BlockNote 默认 Schema 倾向于简化处理，这导致了 "Save & Load" 过程中的信息熵丢失。

**数据流失场景 (Data Loss Scenario)**：

- **Input**: `$$E=mc^2$$` (Block Level)
- **BlockNote Parse**: 识别为 `math` 类型，但默认行为可能将其视为行内公式渲染。
- **Save**: 序列化时丢失了 `$$` 标记，变成了 `$E=mc^2$`。

**全链路保真方案 (Full-Fidelity Pipeline)**：

我们修改了 AST 转换管道的每一个环节，引入了 `displayMode` 元数据。

1.  **Parsing Phase (Remark Plugin)**:
    我们编写了自定义转换器，在解析 AST 时区分公式类型。
    - Input: `$$x$$`
    - AST Node: `{ type: "inlineMath", value: "x", data: { displayMode: true } }`

2.  **Editing Phase (BlockNote Schema)**:
    扩展 `InlineContent` Schema，增加 `displayMode` 属性。

    ```typescript
    // src/types/block.ts
    export type MyInlineMath = {
      type: "inlineMath";
      props: {
        latex: string;
        displayMode: boolean; // 新增字段
      };
    };
    ```

3.  **Rendering Phase (React Component)**:
    `InlineMath.tsx` 组件根据 `displayMode` 决定渲染样式。
    - `true`: 渲染为 `display: flex; justify-content: center;` 的块级容器。
    - `false`: 渲染为 `display: inline-block`。

4.  **Serialization Phase (Stringifier)**:
    `blocks-to-markdown.ts` 在输出时进行精确还原。
    ```typescript
    if (node.props.displayMode) {
      return `$$${node.props.latex}$$`; // 还原双美元
    } else {
      return `$${node.props.latex}$`; // 还原单美元
    }
    ```
    这一改动确保了用户无论是输入行内公式还是块级公式，在 "保存 -> 刷新" 闭环后，不仅内容一致，连**排版格式**都完全一致。

### 3.3 自定义块数据转换架构 (Custom Block Data Transformation Architecture)

**演进背景**：在 E2.1 阶段，我们使用 "JSON Code Block" 作为所有自定义块（Container, Vue 组件等）的通用降级方案。这种方案虽然保证了数据不丢失，但用户体验极差。E2.3 引入了真正的 UI 组件，这就要求 Adapter 层必须能够精确地在 Markdown AST、Internal Block 和 BlockNote Block 之间进行双向转换。

**1. ContainerBlock 适配逻辑深度解析 (Deep Dive)**

- **数据结构扁平化 (Flattening Strategy)**:
  Markdown AST 中的 Container 是一个嵌套结构（Parent -> Children）。但在 BlockNote 编辑器中，为了方便光标移动和选区操作，我们将 Container 视为一个拥有特殊属性（`containerType`）的 Block，其内部内容直接映射为编辑器的子 Block 序列。
  - **Markdown AST (Nested)**:
    ```json
    {
      "type": "containerDirective",
      "name": "tip",
      "children": [
        { "type": "paragraph", "children": [...] },
        { "type": "list", "children": [...] }
      ]
    }
    ```
  - **BlockNote Block (Flattened in Editor)**:
    ```javascript
    {
      "type": "container",
      "props": { "containerType": "tip", "title": "提示" },
      "content": [ // 直接作为 content 数组，而非 children 属性
         { "type": "paragraph", "content": [...] },
         { "type": "bulletListItem", "content": [...] }
      ]
    }
    ```
    这种**扁平化映射**极大地简化了编辑器内部的各种操作（如拖拽、全选），因为 BlockNote 核心对深层嵌套的 `children` 属性支持并不完美，而 `content` 数组是其原生的一级公民。

- **样式同步机制 (Style Synchronization)**:
  为了确保 "Editor" 和 "Renderer" (VitePress) 的视觉一致性，我们在 `src/style.css` 中建立了一套 CSS 变量映射层。

  | 变量名                         | 作用       | 派生逻辑 (HSL)                          |
  | :----------------------------- | :--------- | :-------------------------------------- |
  | `--vp-custom-block-tip-bg`     | 提示块背景 | `var(--vp-c-brand-soft)`                |
  | `--vp-custom-block-tip-border` | 提示块边框 | `hsl(from var(--bg) h s calc(l * 0.5))` |
  | `--vp-custom-block-info-bg`    | 信息块背景 | `var(--vp-c-gray-soft)`                 |

  通过 CSS `hsl(from ...)` 语法，我们实现了**无需 JavaScript 介入的主题自适应**。当用户切换亮/暗模式时，由于基础变量 (`--vp-c-brand-soft`) 会自动变化，派生的边框颜色也会立即跟随变化，且始终保持 50% 的亮度差以确保可访问性 (A11y)。

### 3.4 核心交互组件实现细节 (Core Interactive Components Implementation)

E2.3 阶段引入了四个关键的高交互 React 组件，这些组件不仅是 UI 展示，更涉及复杂的事件管理和 DOM 交互。

#### [1] MathBlock & InlineMath (双模式数学公式系统)

- **架构决策**：为何不统一使用一个组件？
  - **Block (块级)**: 需要支持多行编辑、居中布局、以及与周围文本的垂直间距管理。我们使用 `MathBlock` 封装，并在内部通过 `katex.renderToString` 提供**实时预览**，编辑模式下切换为 `textarea` 或 `math-field`。
  - **Inline (行内)**: 必须作为文本流的一部分。BlockNote 将其视为 `InlineContent` (Atom Node)。

- **[核心难点] 原子节点陷阱 (The Atomic Node Trap)**
  - **现象**：在 ProseMirror (BlockNote 的底层) 中，`type: "inline"` 且 `isAtom: true` 的节点（如行内公式）被视为不可分割的黑盒。光标默认无法停留在节点内部，也无法通过方向键 "步入" 节点。
  - **传统方案的局限**：通常做法是使用 `NodeView` 并设置 `contentDOM`，但 Web Component (`<math-field>`) 均有自己的 Shadow DOM 和事件循环，ProseMirror 无法接管其 contentDOM。
  - **我们的创新解法：捕获阶段事件拦截 (Event Capture Interception)**
    我们在 `InlineMath.tsx` 中实施了一套底层的事件拦截机制：

    ```typescript
    // 关键代码逻辑复现
    const handleKeyDown = (e: KeyboardEvent) => {
      // 1. 判断按键是否由于方向键触发
      if (e.key !== "ArrowLeft" && e.key !== "ArrowRight") return;

      // 2. 获取当前 MathField 的 DOM 引用
      const host = mathfieldRef.current;

      // 3. 计算光标相对于 Host 的位置（这是一个复杂的判定逻辑）
      // 我们不依赖 ProseMirror 的 Selection，因为此时 Selection 可能还在 Host 外部
      // 而是检查 document.activeElement 是否即将变更

      // 4. 强制接管焦点
      // useCapture = true 确保我们在 React 和 ProseMirror 之前捕捉到事件
      e.stopPropagation();

      // 5. 调用 MathLive 的内部 API 移动光标
      // executeCommand("moveToMathfieldStart")
    };
    ```

    这一机制确保了用户体验的连续性：从普通文本按右键 -> 直接进入公式编辑 -> 继续按右键 -> 离开公式回到文本。

#### [2] MermaidBlock (高级图表编辑器)

- **技术选型**：废弃原生的 `<textarea>`，转而集成 `@uiw/react-codemirror`。
- **收益**：
  - **语法高亮**：Mermaid 语法复杂，CodeMirror 提供了关键词高亮，降低书写错误。
  - **行号与缩进**：对于大型流程图（如时序图、甘特图），行号是 debug 的刚需。
  - **状态管理**： CodeMirror 的 `onChange` 直接防抖绑定到 `editor.updateBlock(block, { props: { code } })`，确保 React state 和 ProseMirror document state 的单向数据流稳定。

#### [3] Slash Menu System (指令与搜索系统)

- **设计模式**：`Strategy Pattern` (策略模式)。我们将 Slash Menu 的配置完全抽离为 `SlashMenuItems.tsx`。
- **混合搜索算法 (Hybrid Search Algorithm)**：
  为了支持中文用户的习惯，我们实现了一个基于别名 (`aliases`) 的加权搜索：
  1.  **Direct Match**: 输入 `/mermaid` -> 匹配 `title` 或 `key`。
  2.  **Alias Match**: 输入 `/tubiao` (图表) -> 匹配 `aliases: ["tubiao", "liucheng"]`。
  3.  **Fuzzy Match**: 输入 `/shuxue` -> 匹配 `aliases: ["shuxue"]` -> 定位到 "数学公式"。
      这种实现在 `SuggestionMenuController` 的 `getItems` 异步回调中完成，保证了通过输入触发菜单时的零延迟体验。

### 3.5 构建与工程化优化 (Build & Engineering)

- **Switch-Case 分支清理**：
  在 `markdown-to-blocks.ts` 中，我们发现 `remark` 对于某些节点类型的处理存在冗余 case（如 `case "math"` 出现了两次）。这虽然在运行时会被 JavaScript 引擎优化，但在 TypeScript 编译阶段会抛出 "Fallthrough case in switch" 或逻辑不可达的警告。我们对 AST 遍历逻辑进行了重构，合并了所有同类型节点的处理分支，确保代码库的 Clean Build。

### 3.6 动态预览与安全架构 (Dynamic Preview & Safety Architecture)

**预览系统挑战**：需要在编辑器内实时渲染 Vue 组件，同时隔离 CSS 污染并确保文件操作安全。

1.  **Backend Mutex Lock**:
    在 `src-tauri/src/app/commands/vitepress.rs` 中引入了全局 `AsyncMutex`。防止前端并发请求（如同时渲染多个 IncludeBlock）导致的文件系统竞争条件（如 `os error 183` 创建目录冲突）。

2.  **Global Context Store**:
    通过 `EditorContext.tsx` 建立全局状态总线，将 `devServerUrl` 等关键信息直接广播至深层 React 组件，绕过了 BlockNote 可能存在的 Context 穿透屏蔽问题。

3.  **Recursion Safety**:
    `IncludeBlock` 实现了严格的递归检查算法 (`filterNode`)，在文件选择树中自动剔除当前文件及其父级空目录，从 UI 层面物理阻断了 `@include` 循环引用的产生。

---

## 4. 表格块双向转换机制 (Bidirectional Table Conversion)

**问题深度剖析**：
BlockNote 的表格模型 (`TableBlock`) 与标准的 Markdown GFM Table AST 存在巨大的结构性差异。

- **Markdown**: Row -> Cell (Text)。结构非常简单。
- **BlockNote**: Table -> TableRow -> TableCell -> Block[]。BlockNote 的每个单元格本质上是一个迷你的编辑器实例（可以包含加粗、链接甚至其他 Inline Block）。

**双向映射算法**：

1.  **Inbound (Markdown -> Editor)**:
    我们不能简单地将文本塞入单元格。通过 `internalInlineToBlockNote` 函数，我们将 Markdown 单元格内的 Phrasing Content（如 `**bold**`, `[link](url)`) 逐个解析为 BlockNote 的 `StyledText` 对象数组。

    ```typescript
    // 将 Markdown AST 节点转换为 StyledText 对象
    const content = cell.children.map((child) => {
      if (child.type === "strong")
        return { type: "text", text: child.value, styles: { bold: true } };
      // ... 处理其他类型
    });
    ```

2.  **Outbound (Editor -> Markdown)**:
    保存时，我们需要遍历 BlockNote 的 TableCell，提取其 `content` 数组，并将其序列化回 Markdown AST 节点，最后由 `remark-stringify` 生成 Markdown 文本。

---

## 5. E2.4 ShikiCodeBlock 高级代码块实现

E2.4 阶段引入了 `ShikiCodeBlock` 组件，替换默认的 CodeBlock 以支持 VitePress 丰富的代码块语法。

### 5.1 核心功能与架构

**技术选型**：集成 `@uiw/react-codemirror` 作为编辑器内核，支持多语言语法高亮。

**VitePress 元数据支持**：

- **语言选择器**：DaisyUI 风格的下拉菜单，支持 JS/TS/Vue/HTML/CSS/JSON/Python/Rust 等
- **文件名标题**：`[filename.ts]` 语法，用于 Code Group Tab 标题
- **行高亮**：`{1-3, 5}` 语法，支持范围和单行标记
- **行号显示**：`:line-numbers` 和 `:line-numbers=10` 指定起始行
- **Diff 模式**：通过工具栏按钮插入 `// [!code ++]` 或 `// [!code --]` 注释

### 5.2 Code Group 多标签聚合

**序列化 (blocks-to-markdown.ts)**：
当 `tabs` 属性包含多个元素时，自动生成 `::: code-group` 容器：

````markdown
::: code-group

```ts [config.ts]
export default {};
```

```js [config.js]
module.exports = {};
```

:::
````

**解析 (markdown-to-blocks.ts)**：
将 `::: code-group` 内部的多个代码块聚合为单个 `ShikiCodeBlock`，并通过 `tabs` 属性 (JSON 序列化) 存储。

### 5.3 关键 Bug 修复历史

> [!IMPORTANT]
> **Code Group 解析失败问题**
>
> **症状**：`::: code-group` 被识别为两个独立的代码块，而非多标签聚合。
>
> **根因**：`remarkVitePressContainers` 插件的正则表达式中，**遗漏了 `code-group`**。
>
> ```diff
> - /^:::\s*(info|tip|warning|danger|details|note)(?:[ \t]+(.*?))?\n([\s\S]*?)\n:::\s*$/i
> + /^:::\s*(info|tip|warning|danger|details|note|code-group)(?:[ \t]+(.*?))?\n([\s\S]*?)\n:::\s*$/i
> ```
>
> **修复**：在 `markdown-to-blocks.ts` 的两处正则中添加 `code-group` (Line 114, 148)。

> [!NOTE]
> **代码块自动聚焦问题**
>
> **症状**：输入代码时输入法会被打断，或者光标位置异常。
>
> **修复**：将 `CodeMirror` 组件的 `basicSetup` 配置从 Props 移出至组件外部常量，防止 React 每次渲染时重新创建 Extension 实例导致 Editor View 重建。

> [!NOTE]
> **Shiki 高亮闪烁**
>
> **症状**：快速输入时高亮样式会有短暂延迟或闪烁。
>
> **优化**：实施了 `useShikiHighlighter` hook 的单例模式缓存，复用 Highlighter 实例。

> [!NOTE]
> **下拉菜单裁剪问题**
>
> **症状**：Language Selector 下拉菜单被 `overflow: hidden` 裁剪。
>
> **修复**：移除外层容器的 `overflow-hidden`，改为对 Toolbar 添加 `rounded-t-lg`，对 Editor 容器添加 `rounded-b-lg overflow-hidden`。

> [!NOTE]
> **下拉菜单多列布局问题**
>
> **症状**：DaisyUI `menu` 类导致语言选择器显示为两列而非单列。
>
> **修复**：移除 `menu` 类依赖，改用 `list-none flex flex-col` 强制单列布局。

---

## 6. E2.4 QuoteBlock 原生引用块实现

E2.4 阶段完成了 QuoteBlock 的原生化改造，实现了 Markdown 标准的嵌套引用支持及精确的双向转换。

### 6.1 核心架构：List-Item 模型

**设计哲学**：将引用块视为「可嵌套的列表项」而非「带样式的段落」。

- **传统方案痛点**：将 `> text` 解析为带样式的段落，无法处理多段落引用和嵌套引用。
- **List-Item 模型**：每个 `>` 行作为独立的 BlockNote Block，通过 `groupId` 标识同组引用。

```typescript
// BlockNote Block 结构
{
  type: "quote",
  props: {
    groupId: "group-1704441600000-abc123",  // 同组标识
    isFirstInGroup: true                     // 组内首位标记
  },
  content: [{ type: "text", text: "引用内容" }],
  children: []  // 嵌套引用作为 children
}
```

### 6.2 GroupId 分组机制

**核心问题**：Markdown 允许两种相邻引用块的语义：

```markdown
<!-- 同一引用块的多行 -->

> line 1
> line 2

<!-- 两个独立引用块 -->

> quote A

> quote B
```

**解决方案**：

| 阶段       | 文件                   | 实现逻辑                                                                                          |
| :--------- | :--------------------- | :------------------------------------------------------------------------------------------------ |
| **解析**   | `blocknote-adapter.ts` | `internalToBlockNote`: 同一内部 QuoteBlock 的所有段落分配相同 `groupId`                           |
| **编辑**   | `KeyboardShortcuts.ts` | Enter 分割块时，通过 `setNodeMarkup` 复制原块的 `groupId` 到新块                                  |
| **序列化** | `blocknote-adapter.ts` | `blockNoteToInternal`: 预处理阶段合并相邻同 `groupId` 块，使用 `_mergedSequence` 保持嵌套引用顺序 |

### 6.3 键盘快捷键扩展

**文件**：`src/components/editor/react_app/extensions/KeyboardShortcuts.ts`

```typescript
export const QuoteKeyboardShortcuts = Extension.create({
  addKeyboardShortcuts() {
    return {
      Enter: ({ editor }) => {
        // 空行：跳出引用 (lift 或转为 paragraph)
        // 有内容：splitBlock + 复制 groupId
      },
      "Shift-Enter": ({ editor }) => {
        return editor.commands.setHardBreak(); // 软换行
      },
    };
  },
});
```

### 6.4 视觉分组：Sibling Class 机制

**需求**：同组引用块应视觉上无缝连接，不同组保持间距。

**实现**：

1. **Adapter 标记**：`isFirstInGroup: quoteBlocks.length === 0`
2. **React 渲染**：非首位块添加 `quote-block-sibling` class
3. **CSS 负 margin 技巧**：

```css
.bn-block-outer {
  margin: calc(2 * var(--spacing)) 0;
}

.bn-block-outer:has(.quote-block-sibling) {
  margin-top: calc(-2 * var(--spacing)); /* 精确抵消间距 */
}
```

### 6.5 嵌套引用顺序保持

**问题**：合并同组引用时，嵌套引用位置被移到末尾。

**解决方案**：使用 `_mergedSequence` 数组保持原始顺序：

```typescript
_mergedSequence: [
  { type: "content", content: ["line 1"] },
  { type: "content", content: ["line 2"] },
  { type: "child", child: nestedQuoteBlock }, // 保持位置
  { type: "content", content: ["line 3"] },
];
```

### 6.6 关键修复历史

> [!IMPORTANT]
> **引用块断裂问题**
>
> **症状**：`> line 1\n> \n> line 2` 被序列化为两个独立引用块。
>
> **根因**：`blockNoteToInternal` 将每个 BlockNote quote 转换为独立的 Internal QuoteBlock，丢失 `groupId`。
>
> **修复**：在 `blockNoteToInternal` 中添加预处理步骤，合并相邻同 `groupId` 块后再转换。

> [!NOTE]
> **嵌套引用顺序错乱**
>
> **症状**：`> text\n> > nested\n> text2` 中嵌套引用被移到末尾。
>
> **根因**：合并逻辑将 `_mergedContents` 和 `children` 分开存储。
>
> **修复**：改用 `_mergedSequence` 单一数组，按顺序存储 content 和 child 项。

---

## 7. 全链路验证与测试矩阵 (Full-Link Verification Matrix)

为了确保 E2.3 交付的质量，我们执行了覆盖以下维度的测试验证：

| 模块                | 测试场景     | 操作步骤                    | 预期行为                                                  | 实际结果              | 状态 |
| :------------------ | :----------- | :-------------------------- | :-------------------------------------------------------- | :-------------------- | :--- |
| **Markdown Parser** | 容器嵌套解析 | 加载包含 List 的 `::: tip`  | List 应正确缩进显示在 Tip 内部                            | ✅ 缩进完美，层级正确 | Pass |
| **Markdown Parser** | 自定义标题   | 加载 `::: warning 注意事项` | 标题栏应显示 "注意事项" 而非默认的 "WARNING"              | ✅ 标题正确提取       | Pass |
| **Inline Math**     | 编辑交互     | 在文本中间点击公式 `$x$`    | 弹出 MathLive 面板，背景变暗，输入焦点锁定                | ✅ 交互流畅           | Pass |
| **Inline Math**     | 光标逃逸     | 在公式末尾按 `→`            | 光标应立即跳出 Shadow DOM，出现在公式后的文本节点中       | ✅ 成功逃逸           | Pass |
| **Mermaid**         | 语法错误处理 | 输入错误的 Mermaid 代码     | 预览区域应显示错误提示，而不是导致编辑器崩溃              | ✅ 显示红色错误日志   | Pass |
| **Persistence**     | 双向一致性   | 输入公式 -> 保存 -> 刷新    | 重新加载后的公式内容应与保存前完全一致（包括 LaTeX 格式） | ✅ 字节级一致         | Pass |

---

## 8. E2.5 UI/UX 深度定制与工程化重构 (E2.5 UI/UX & Refactor)

E2.5 阶段聚焦于打磨编辑器 "最后一公里" 的用户体验，移除了所有 BlockNote 的原生 React UI 依赖，构建了一套完全自主可控的 Toolbar 和 Menu 系统。

### 8.1 统一图标与视觉系统 (Unified Icon System)

**背景**：BlockNote 内部依赖 `react-icons` (Phosphor Set)，风格与我们项目的 Lucide Icons 不符且不支持 Tree Shaking。

**重构行动**：

- **全面替换**：从底层 `BlockCapabilities.ts` 到 UI 组件 `StaticToolbar.tsx`，将所有图标替换为 `@iconify/react` (`lucide:` 集合)。
- **收益**：包体积减少，视觉风格完全统一。

### 8.2 静态/浮动工具栏重写 (Toolbar Override)

**痛点**：默认的 Formatting Toolbar 包含大量我们不需要的按钮（如颜色、对齐），且无法扩展自定义 Block 的操作（如切换公式键盘）。

**架构方案**：

1.  **屏蔽原生 UI**：`<BlockNoteView formattingToolbar={false} />`
2.  **构建自定义 Toolbar (`StaticToolbar.tsx`)**：
    - 集成 `BlockRegistry` 获取上下文操作。
    - **UI 基础设施 (`ToolbarControls.tsx`)**：
      - `ToolbarDropdown`：使用 React Portal 将菜单渲染到 Body 层级，彻底解决了在 overflow:hidden 容器（如表格或布局容器）中下拉菜单被裁剪的问题。
      - `ToolbarInput`：内置防抖 (Debounce) 的输入组件，用于属性编辑。

```typescript
// src/components/editor/react_app/BlockCapabilities.ts
blockRegistry.register("math", {
  actions: [
    { type: "button", id: "toggleKeyboard", icon: ... },
    { type: "button", id: "toggleMenu", icon: ... }
  ]
});
```

### 8.3 斜杠菜单深度定制 (Slash Menu Customization)

**文件**：`src/components/editor/react_app/SlashMenuItems.tsx`

**核心增强**：

1.  **干扰项过滤**：移除了 Default 的 Quote、Heading、CodeBlock，只保留我们深度适配的自定义版本。
2.  **中文适配**：
    - **拼音混合搜索**：键入 `/gs` 可匹配 "公式" (Gong Si)。
    - **中文分组**：将菜单项按 "标题", "基础", "媒体", "容器", "高级功能" 重组。
3.  **排序策略**：通过自定义 `groupOrder` 数组，强制将 "标题" 组置顶，提升高频操作效率。

```typescript
const groupOrder = [
  "标题",
  "基础",
  "媒体",
  "容器",
  "高级功能",
  "VitePress",
  "其他",
];
finalItems.sort(
  (a, b) => groupOrder.indexOf(a.group) - groupOrder.indexOf(b.group)
);
```

### 8.4 预览同步架构 (Preview Synchronization)

**组件**：`src/composables/usePreviewSync.ts` (Vue Realm)

为了打通 "Editor (React)" 与 "Preview (VitePress/Vue)" 之间的隔阂，我们设计了状态同步 Hook：

1.  **自动刷新机制**：
    监听 `docStore.isSaving` 状态。当保存完成（File System update）后，自动触发 debounced refresh，等待 VitePress HMR 热更新完成。

2.  **滚动同步 (预留)**：
    `syncScrollToBlock` 函数实现了基于 Block ID 的双向定位能力，为未来的 "Scroll Sync" 功能打下了基础。

3.  **Frontmatter 同步**：
    `docStore` 新增 `updateFrontmatter` Action，确保元数据变更（如 Title, Description）能即时写入文件头，并在预览中生效。

### 8.5 工程化细节

- **类型补全**：添加 `src/types/mathlive.d.ts` 解决 JSX Intrinsic Elements 报错。
- **副作用隔离**：`ContainerBlock`, `MermaidBlock` 等组件内移除了遗留的 `actions` 数组，操作统一收敛至 Toolbar 注册表。
- **Z-Index 管理**：修正 `StaticToolbar` z-index 为 40，解决了遮挡侧边栏菜单的问题。

---

## 9. E2.5 FrontmatterPanel 动态配置重构 (Dynamic Frontmatter Panel)

E2.5 阶段对文档元数据配置面板进行了彻底重构，从静态表单转变为动态、可扩展的配置系统。

### 9.1 架构设计

- **动态字段注册表 (`FIELD_REGISTRY`)**：
  集中管理所有支持的 Frontmatter 字段定义（Label, Type, Group, Description）。支持 `text`, `textarea`, `select`, `toggle` 等多种输入类型。
- **动态状态管理**：
  面板不再显示所有可能的字段，而是仅显示文档中**已配置**的字段。用户通过 "添加配置项" 菜单按需添加。

### 9.2 关键特性

1.  **分组管理**：
    将字段划分为 `Basic` (基础), `Layout` (布局与显示), `Sidebar` (侧边栏配置), `Advanced` (高级) 四大组。
    - **Sidebar 分组**：专门集成了 `vitepress-sidebar` 插件所需的 `order`, `date`, `exclude` 字段。
2.  **UI/UX 优化**：
    - **自定义 Select 下拉菜单**：使用 `Teleport` 实现的自定义下拉菜单，替代原生 `<select>`，提供一致的视觉体验（Neutral Hover, Active Border）。
    - **智能添加菜单**：基于 `getBoundingClientRect` 的动态定位菜单，解决了 Hover 间隙导致的菜单关闭问题。
    - **字段描述**：在每个字段 Label 下方提供详细的 Description，辅助用户理解配置项用途（如 SEO 影响、布局行为）。
3.  **数据清洗**：
    - 在编辑编辑过程中**保留空字符串**，防止用户清空输入框时字段立即跳动消失。
    - 仅在最终保存（Persistence Layer）时清理 Empty Values。

### 9.3 核心代码

```typescript
// Field Registry Example
const FIELD_REGISTRY: FieldConfig[] = [
  {
    key: "layout",
    label: "布局",
    type: "select",
    group: "layout",
    options: [
      { value: "doc", label: "文档" },
      { value: "home", label: "首页" },
    ],
  },
  {
    key: "order",
    label: "排序优先级",
    group: "sidebar", // New Group
    description: "侧边栏菜单的排序优先级...",
  },
];
```

### 9.4 后端预览优化 (Backend Preview Optimization)

- **文件：** `src-tauri/src/app/commands/vitepress.rs`
- **变更：** `vitepress_create_preview` 函数
- **逻辑：**
  生成的预览 Markdown 文件现自动注入 `exclude: true` Frontmatter。
  这确保了 `vitepress-sidebar` 等自动侧边栏插件会忽略产生的临时预览文件，避免其污染侧边栏目录导航。
