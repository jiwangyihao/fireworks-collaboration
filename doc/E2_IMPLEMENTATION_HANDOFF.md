# E2 阶段实施交接文档（编辑器基础与 Markdown 适配）

> 本文档记录 E2.1 和 E2.2 阶段的实施成果，包括 BlockNote 编辑器集成、Markdown 双向适配器、以及关键的输入与渲染修复。

---

## 1. 实施概述

**阶段目标**：集成 BlockNote 编辑器，实现 Markdown 内容的无损加载与保存。解决 Vue/React 混合架构下的数据流问题，并确保自定义块（公式、Mermaid、VitePress 容器）的数据在编辑过程中不丢失。

**完成日期**：2026-01-03

**状态**：✅ 已完成（E2.1 & E2.2）

---

## 2. 交付物清单

### 2.1 核心适配器 (Adapter Layer)

| 文件                              | 说明                                                                                                                                                                   |
| :-------------------------------- | :--------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `src/utils/markdown-to-blocks.ts` | **Markdown 解析器**<br>- 基于 `unified` + `remark` 生态<br>- 双美元符号公式 (`$$`) 智能识别插件<br>- 支持自定义块（Vue组件、Include指令）解析                          |
| `src/utils/blocks-to-markdown.ts` | **Markdown 生成器**<br>- 将内部 Block 模型序列化为 Markdown<br>- 智能处理行内/块级公式格式<br>- 支持 Prettier 兼容的尾部换行                                           |
| `src/utils/blocknote-adapter.ts`  | **编辑器适配器**<br>- Internal Block (E0) <-> BlockNote Block 转换<br>- 自定义块通过 JSON Code Block 实现无损透传<br>- 内联公式 (`$`) 与块级内联公式 (`$$`) 的显示控制 |

### 2.2 编辑器组件

| 文件                                                  | 说明                                                                                                            |
| :---------------------------------------------------- | :-------------------------------------------------------------------------------------------------------------- |
| `src/components/editor/react_app/BlockNoteEditor.tsx` | **React 编辑器内核**<br>- 封装 `@blocknote/react`<br>- 处理中文本地化<br>- 仅负责渲染与内部状态更新             |
| `src/components/editor/BlockEditor.vue`               | **Vue 包装器**<br>- 使用 `veaury` 实现 React 组件桥接<br>- 负责 Props 传递与事件监听                            |
| `src/views/DocumentView.vue`                          | **文档视图控制器**<br>- 管理 `docStore` 与编辑器的交互<br>- 实现 `staticInitialBlocks` 策略解决输入循环重置问题 |

### 2.3 类型定义

| 文件                 | 说明                                                                    |
| :------------------- | :---------------------------------------------------------------------- |
| `src/types/block.ts` | **Block 模型定义**<br>- 新增 `displayMode` 字段用于区分行内公式显示模式 |

---

## 3. 关键技术实现详解

### 3.1 解决 Vue/React 响应式循环 (Infinite Loop Fix)

**问题**：DocumentView 将 `docStore.currentBlocks` 作为 props 传给编辑器，编辑器 `onChange` 更新 `docStore`，导致 `currentBlocks` 变更，进而触发编辑器重新渲染（Re-mount），使用户输入中断。

**解决方案**：采用 **静态快照初始化 (Static Snapshot Initialization)** 策略。

```typescript
// src/views/DocumentView.vue

// 仅在文档加载完成时获取一次快照
const staticInitialBlocks = ref<any[]>([]);
watch(loadingContent, (isLoading) => {
  if (!isLoading && docStore.currentBlocks) {
    // 深拷贝以切断响应式引用
    staticInitialBlocks.value = JSON.parse(JSON.stringify(docStore.currentBlocks));
  }
});

// 编辑器仅使用 key 强制在切换文件时重置
<BlockEditor
  :key="selectedPath || 'editor'"
  :initial-content="staticInitialBlocks"
  @change="handleEditorChange"
/>
```

### 3.2 公式格式保真 (Math Fidelity)

**问题**：BlockNote 默认不支持 `$$...$$` 的行内块级公式，保存时会将其降级为 `$...$`，导致渲染不一致。

**解决方案**：

1.  **解析阶段**：自定义 `remarkEnrichMath` 插件，分析 Markdown 源码，标记 `displayMode: true`。
2.  **编辑阶段**：在编辑器中将 `displayMode: true` 的公式显式渲染为文本 `$$x^2$$`，而普通公式渲染为 `$x^2$`。
3.  **保存阶段**：`blocks-to-markdown` 根据标记还原为双美元符号。

```typescript
// markdown-to-blocks.ts (remarkEnrichMath)
if (raw.startsWith("$$")) {
  node.data.displayMode = true;
}

// blocknote-adapter.ts
if (item.displayMode) {
  return { type: "text", text: `$$${formula}$$`, ... };
}
```

### 3.3 自定义块无损透传 (Custom Block Preservation)

**策略**：在 E2.3 实现专门的 UI 渲染之前，防止 Parser 解析出的自定义块（如 `VueComponent`, `Include`）被编辑器丢弃。

**实现**：

- **Internal -> BlockNote**：将自定义块转换为 `json` 语言的 `codeBlock`，并将原始数据序列化为注释块。
- **BlockNote -> Internal**：识别包含特定签名的 JSON 代码块，反序列化还原为原始 Block 类型。

---

## 4. 后续阶段衔接 (E2.3 & E2.4)

当前编辑器已具备稳健的数据层，接下来的工作将聚焦于 UI 层的丰富：

1.  **MathBlock 实现**：移除文本形式的 `$$`，使用 `KaTeX` 实现真·所见即所得公式编辑。
2.  **MermaidBlock 实现**：实现 Mermaid 图表的实时预览与代码编辑双模式。
3.  **ContainerBlock 实现**：支持 VitePress 提示块的可视化编辑。
4.  **VitePress 特性支持**：为 `VueComponent` 和 `@include` 提供专门的属性编辑表单，而非 JSON 代码块。
