# E0 阶段实施交接文档（核心基础设施）

> 本文档记录 E0 阶段的实施成果，为后续 E1-E4 阶段提供技术基础和交接说明。

---

## 1. 实施概述

**阶段目标**：建立编辑器核心基础设施，包括 Block 类型系统和 Markdown 双向转换器。

**完成日期**：2026-01-01

**状态**：✅ 已完成

---

## 2. 交付物清单

### 2.1 类型定义

| 文件                    | 说明                                |
| ----------------------- | ----------------------------------- |
| `src/types/block.ts`    | Block 类型系统（标准块 + 自定义块） |
| `src/types/document.ts` | Document、Frontmatter、DocTree 类型 |

### 2.2 转换器

| 文件                              | 说明                      |
| --------------------------------- | ------------------------- |
| `src/utils/markdown-to-blocks.ts` | Markdown → Block 转换器   |
| `src/utils/blocks-to-markdown.ts` | Block → Markdown 序列化器 |
| `src/utils/markdown-converter.ts` | 统一导出入口              |

### 2.3 测试

| 文件                                             | 说明              |
| ------------------------------------------------ | ----------------- |
| `src/utils/__tests__/markdown-converter.test.ts` | 44 个单元测试用例 |

---

## 3. 依赖变更

新增以下生产依赖：

```json
{
  "unified": "^11.0.5",
  "remark-parse": "^11.0.0",
  "remark-stringify": "^11.0.0",
  "remark-gfm": "^4.0.1",
  "remark-directive": "^4.0.0",
  "remark-math": "^6.0.0",
  "remark-frontmatter": "^5.0.0",
  "yaml": "^2.8.2"
}
```

新增开发依赖：

```json
{
  "@types/mdast": "^4.0.4"
}
```

---

## 4. Block 类型系统

### 4.1 标准块类型

| 类型                    | 说明          |
| ----------------------- | ------------- |
| `HeadingBlock`          | 标题（h1-h6） |
| `ParagraphBlock`        | 段落          |
| `BulletListItemBlock`   | 无序列表项    |
| `NumberedListItemBlock` | 有序列表项    |
| `CheckListItemBlock`    | 复选框列表项  |
| `CodeBlockBlock`        | 代码块        |
| `TableBlock`            | 表格          |
| `ImageBlock`            | 图片          |
| `QuoteBlock`            | 引用          |
| `ThematicBreakBlock`    | 分割线        |

### 4.2 自定义块类型（VitePress 扩展）

| 类型                | 说明                        |
| ------------------- | --------------------------- |
| `ContainerBlock`    | VitePress 容器（:::tip 等） |
| `MathBlock`         | LaTeX 数学公式              |
| `MermaidBlock`      | Mermaid 图表                |
| `VueComponentBlock` | Vue 组件标签                |
| `IncludeBlock`      | @include 指令               |

### 4.3 辅助函数

```typescript
// 类型守卫
isStandardBlock(block): block is StandardBlock
isCustomBlock(block): block is CustomBlock
isListItemBlock(block): block is ListItemBlock

// 创建函数
createTextContent(text): TextContent
createParagraphBlock(content): ParagraphBlock
createHeadingBlock(level, content): HeadingBlock
createCodeBlock(code, language): CodeBlockBlock
createContainerBlock(type, children, title): ContainerBlock
createMathBlock(formula, display): MathBlock
createMermaidBlock(code): MermaidBlock
```

---

## 5. 转换器 API

### 5.1 Markdown → Block

```typescript
import {
  markdownToBlocks,
  parseMarkdownDocument,
} from "@/utils/markdown-converter";

// 解析 Markdown 为 Block 数组
const blocks = markdownToBlocks(markdownString);

// 解析完整文档（含 Frontmatter）
const doc = parseMarkdownDocument(markdown, "/path/to/file.md");
console.log(doc.frontmatter.title);
console.log(doc.blocks);
```

### 5.2 Block → Markdown

```typescript
import {
  blocksToMarkdown,
  documentToMarkdown,
} from "@/utils/markdown-converter";

// 序列化 Block 数组
const markdown = blocksToMarkdown(blocks);

// 序列化完整文档（含 Frontmatter）
const fullMarkdown = documentToMarkdown(document);
```

---

## 6. 支持的 Markdown 语法

### 6.1 标准 Markdown

- ✅ 标题（h1-h6）
- ✅ 段落
- ✅ 无序/有序/复选框列表
- ✅ 代码块（含语法高亮）
- ✅ 表格（GFM）
- ✅ 图片
- ✅ 引用块
- ✅ 分割线
- ✅ 行内格式（粗体、斜体、行内代码、链接）

### 6.2 VitePress 扩展

- ✅ 容器语法（:::tip, :::warning, :::danger, :::details）
- ✅ LaTeX 数学公式（$inline$ 和 $$block$$）
- ✅ Mermaid 图表（```mermaid）
- ✅ Vue 组件标签（`<OList path="..." />`）
- ✅ @include 指令（`<!--@include: @/path-->`)

### 6.3 已知限制

- 容器语法解析依赖 remark-directive，可能与 VitePress 原生 :::syntax 有细微差异
- 嵌套容器的深度解析有限
- Vue 组件仅支持静态属性，动态绑定（:prop）需后续扩展

---

## 7. 测试覆盖

### 7.1 测试统计

- 测试用例：44 个
- 通过率：100%
- 覆盖场景：标准 Markdown、行内内容、VitePress 扩展、往返一致性、边界情况

### 7.2 运行测试

```bash
# 运行转换器测试
pnpm test -- src/utils/__tests__/markdown-converter.test.ts

# 运行全部测试
pnpm test
```

---

## 8. 后续阶段衔接

| 阶段                  | 依赖 E0 的内容                                           |
| --------------------- | -------------------------------------------------------- |
| E1 VitePress 项目集成 | `Document`、`DocTreeNode` 类型用于目录树展示             |
| E2 块编辑器           | `markdownToBlocks`/`blocksToMarkdown` 用于编辑器数据转换 |
| E4 PDF 导入           | `BlockSource` 字段用于 PDF 来源追踪                      |

---

## 附：文件变更总结

```diff
+ src/types/block.ts                     # 新增
+ src/types/document.ts                  # 新增
+ src/utils/markdown-to-blocks.ts        # 新增
+ src/utils/blocks-to-markdown.ts        # 新增
+ src/utils/markdown-converter.ts        # 新增
+ src/utils/__tests__/markdown-converter.test.ts  # 新增
M package.json                           # 新增依赖
M pnpm-lock.yaml                         # 更新
```
