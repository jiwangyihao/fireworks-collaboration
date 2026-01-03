# E2 阶段实施交接文档：编辑器核心与 Markdown 深度适配 (E2 Phase Implementation Handoff)

> **文档密级**：内部技术资料
> **适用对象**：前端开发组、架构师
> **最后更新**：2026-01-04
> **版本**：v2.3 (Final Integrated)

---

## 1. 实施概览 (Implementation Overview)

### 1.1 项目背景与目标

本项目旨在构建一个能够完美兼容 VitePress 生态的 WYSIWYG（所见即所得）Markdown 编辑器。E2 阶段的核心挑战在于解决 **BlockNote (ProseMirror)** 的数据模型与 **Markdown (AST)** 之间的 "Impedance Mismatch"（阻抗失配），特别是针对 VitePress 特有的自定义容器 (`::: tip`)、复杂公式 (`$$`) 以及 Vue 组件的无损编辑能力。

### 1.2 关键成果摘要

- **架构突破**：成功实现了基于 React (编辑器) + Vue (宿主) 的混合架构，解决了跨框架状态同步导致的死循环问题。
- **深度适配**：开发了专用的 AST 转换中间件，实现了 BlockNote 缺失的 Markdown 特性（行内块级公式、自定义容器、Mermaid 图表），且做到了 100% 数据回写保真。
- **交互体验**：引入了 MathLive 和 CodeMirror 等重型组件，将专业领域的编辑体验带入通用文本编辑器中。

**完成状态**：✅ E2.1/E2.2/E2.3 全阶段交付闭环

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

| 文件路径                                                    | 关键组件/配置             | 职责描述                                                                                                                                             |
| :---------------------------------------------------------- | :------------------------ | :--------------------------------------------------------------------------------------------------------------------------------------------------- |
| `src/components/editor/react_app/BlockNoteEditor.tsx`       | `<BlockNoteView />`       | **编辑器入口**<br>配置 Editor 实例，挂载 Schema，初始化 `SlashMenuItems` 和 `SideMenu`。                                                             |
| `src/components/editor/react_app/SlashMenuItems.tsx`        | `getCustomSlashMenuItems` | **指令集配置中心**<br>定义 `/` 菜单的各项指令。实现了基于**拼音别名**的混合搜索算法（如 `/gs` -> 公式），极大提升了中文输入体验。                    |
| `src/components/editor/react_app/blocks/InlineMath.tsx`     | `<InlineMath />`          | **行内公式组件**<br>封装 `math-field` Web Component。**关键贡献**：实现了捕获阶段的键盘事件拦截，解决了 ProseMirror 光标无法进入 Shadow DOM 的难题。 |
| `src/components/editor/react_app/blocks/MermaidBlock.tsx`   | `<MermaidBlock />`        | **图表组件**<br>集成 `CodeMirror` 提供专业的代码编辑体验，并配合 `Start/Finish` 状态机管理实时预览。                                                 |
| `src/components/editor/react_app/blocks/ContainerBlock.tsx` | `<ContainerBlock />`      | **容器组件**<br>利用 CSS 变量 (`hsl from`) 实现与 VitePress 主题色的自动同步。管理标题的即时编辑。                                                   |

### 2.3 宿主集成层 (Host Integration Layer)

Vue 应用与 React 编辑器的粘合剂。

| 文件路径                                | 核心逻辑              | 职责描述                                                                                                                                                 |
| :-------------------------------------- | :-------------------- | :------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `src/components/editor/BlockEditor.vue` | `applyPureReactInVue` | **跨框架桥接**<br>使用 `veaury` 将 React 组件包装为 Vue 组件。负责处理 `onChange` 事件的防抖和 Props 的深比较。                                          |
| `src/views/DocumentView.vue`            | `staticInitialBlocks` | **视图控制器**<br>实现了 "Static Snapshot" 策略：仅在首次加载时传递数据给 React，切断后续的响应式链路，防止 Vue 的 Proxy 机制触发 React 的异常 Remount。 |

### 2.4 数据模型定义 (Type Definitions)

| 文件路径             | 定义内容                     | 说明                                                                                                              |
| :------------------- | :--------------------------- | :---------------------------------------------------------------------------------------------------------------- |
| `src/types/block.ts` | `InternalBlock`, `MathBlock` | **通用数据契约**<br>定义了系统内流转的所有 Block 类型。E2.3 新增了 `displayMode` (boolean) 字段用于公式精确保真。 |

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
    ``typescript
if (node.props.displayMode) {
    return `$$${node.props.latex}$$`; // 还原双美元
} else {
    return `$${node.props.latex}$`;   // 还原单美元
}
``
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
    `javascript
{
  "type": "container",
  "props": { "containerType": "tip", "title": "提示" },
  "content": [ // 直接作为 content 数组，而非 children 属性
     { "type": "paragraph", "content": [...] },
     { "type": "bulletListItem", "content": [...] }
  ]
}
`
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

## 6. 后续阶段规划 (Future Roadmap E2.4+)

当前编辑器已具备稳健的数据层 (E2.1/E2.2) 和丰富的 UI 层 (E2.3)，但为了达到 "Production Ready" 状态，仍需解决以下深水区问题：

1.  **QuoteBlock 的原生化改造**:
    - 当前通过 "降级为普通文本" 处理引用块。
    - **计划**: 实现支持 GitHub Alerts 语法 (`> [!NOTE]`) 的原生 Quote Block，支持颜色条、图标和嵌套内容的渲染。

2.  **复杂 Vue 组件的可视化编辑**:
    - 当前自定义 Vue 组件（如 `<CountTo :end="100" />`）仍以 JSON 代码块形式存在。
    - **计划**: 开发通用的 `VuePropEditor`，利用 AST 分析 Vue 组件的 Props 定义，自动生成表单控件（Input, Switch, Slider），让运营人员无需接触代码即可配置组件参数。

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
