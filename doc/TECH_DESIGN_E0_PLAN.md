# E0 阶段细化路线图与开发计划（核心基础设施）

> 本文档将 E0 阶段「核心基础设施」拆解为可执行的任务清单，为后续 E1-E4 阶段的 VitePress 编辑器功能奠定类型基础和转换核心。

---

## 0. 目标、范围与成功标准

### 目标

- 定义统一的 Block 类型系统，作为编辑器内容模型的核心
- 实现 Markdown ↔ Block 双向转换器，支持 VitePress 扩展语法
- 为后续 E1（VitePress 项目集成）、E2（块编辑器）、E4（PDF 导入）提供共享类型和转换工具

### 范围

**包含（E0）**

| 模块             | 说明                                                    |
| ---------------- | ------------------------------------------------------- |
| Block 类型定义   | 标准块（heading/paragraph/list/code/table/image/quote） |
|                  | 自定义块（container/math/mermaid/vueComponent/include） |
| Document 类型    | 文档结构、Frontmatter 元数据                            |
| Markdown → Block | 使用 unified + remark 解析 Markdown 并转换为 Block      |
| Block → Markdown | 将 Block 序列化回 Markdown 文本                         |
| 单元测试         | 覆盖转换器的正向/逆向/往返一致性                        |

**不包含（推迟到后续阶段）**

| 模块                     | 推迟至 |
| ------------------------ | ------ |
| MinerU JSON → Block 转换 | E4     |
| BlockNote ↔ Block 适配  | E2     |
| VitePress 配置解析       | E1     |
| AI/LLM 相关功能          | E4     |

### 成功标准

1. **类型完备性**：Block 类型覆盖 EDITOR_DEVELOPMENT_PLAN.md 中定义的所有块类型
2. **转换正确性**：标准 Markdown 语法的双向转换无信息丢失
3. **扩展语法支持**：VitePress 容器（:::tip）、LaTeX 公式（$$）、Mermaid 代码块正确解析
4. **测试覆盖**：单元测试覆盖率 ≥ 80%
5. **构建通过**：`pnpm build` 与 `pnpm test` 全部通过

---

## 1. 项目现状分析

### 1.1 现有结构

```
src/
├── types/
│   └── project.ts          ← 仅包含项目/仓库相关类型
└── utils/
    ├── __tests__/           ← 8 个测试文件
    ├── check-preheat.ts
    ├── format.ts
    ├── github-api.ts
    ├── github-auth.ts
    └── ...                  ← 无 markdown 相关工具
```

### 1.2 依赖分析

当前 `package.json` 中**无** Markdown 解析相关依赖，需新增：

| 依赖                 | 版本    | 用途                              |
| -------------------- | ------- | --------------------------------- |
| `unified`            | ^11.0.0 | Markdown AST 处理管道             |
| `remark-parse`       | ^11.0.0 | Markdown → MDAST 解析             |
| `remark-stringify`   | ^11.0.0 | MDAST → Markdown 序列化           |
| `remark-directive`   | ^3.0.0  | 解析 VitePress :::容器 语法       |
| `remark-math`        | ^6.0.0  | 解析 LaTeX $$ 公式                |
| `remark-frontmatter` | ^5.0.0  | 解析 YAML Frontmatter             |
| `yaml`               | ^2.0.0  | YAML 解析/序列化                  |
| `@types/mdast`       | ^4.0.0  | MDAST 类型定义（devDependencies） |

---

## 2. E0 分阶段与任务清单

### E0.1 依赖安装与类型定义（约 0.5 周）

**范围**：

- 安装 unified 生态依赖
- 创建 `src/types/block.ts`：定义所有 Block 类型
- 创建 `src/types/document.ts`：定义 Document 结构

**交付物**：

- [ ] 安装并验证 unified 相关依赖
- [ ] Block 基础类型（BaseBlock、InlineContent）
- [ ] 标准块类型（HeadingBlock、ParagraphBlock、ListBlock 等）
- [ ] 自定义块类型（ContainerBlock、MathBlock、MermaidBlock）
- [ ] VitePress 特有块类型（VueComponentBlock、IncludeBlock）
- [ ] Document 类型与 Frontmatter 接口

**验收**：

- TypeScript 编译通过
- 类型导出完整

---

### E0.2 Markdown → Block 转换器（约 1 周）

**范围**：

- 创建 `src/utils/markdown-to-blocks.ts`
- 使用 remark 解析 Markdown 为 MDAST
- 将 MDAST 节点映射为 Block 类型

**交付物**：

- [ ] unified + remark 插件链配置
- [ ] MDAST → Block 节点映射函数
- [ ] VitePress 容器（:::tip/warning/danger/details）解析
- [ ] LaTeX 公式（$inline$ 和 $$block$$）解析
- [ ] Mermaid 代码块识别
- [ ] Vue 组件标签（`<OList .../>`）解析
- [ ] @include 指令（`<!--@include: ...-->`）解析
- [ ] Frontmatter YAML 解析

**验收**：

- 单元测试覆盖基本解析场景
- 能正确解析 fireworks-notes-society 仓库中的示例文档

---

### E0.3 Block → Markdown 转换器（约 0.5 周）

**范围**：

- 创建 `src/utils/blocks-to-markdown.ts`
- 将 Block 序列化为 Markdown 文本

**交付物**：

- [ ] Block → MDAST 反向映射
- [ ] MDAST → Markdown 序列化
- [ ] VitePress 容器输出 `:::type\n内容\n:::`
- [ ] 公式输出 `$$..$$` 或 `$...$`
- [ ] Mermaid 输出 ` ```mermaid `
- [ ] Vue 组件原样输出 `<Component .../>`
- [ ] @include 指令原样输出

**验收**：

- Markdown → Block → Markdown 往返一致性测试
- 格式化输出符合 VitePress 规范

---

### E0.4 测试与文档（约 0.5 周）

**范围**：

- 编写完整的单元测试
- 更新项目文档

**交付物**：

- [ ] 转换器单元测试（`src/utils/__tests__/markdown-converter.test.ts`）
- [ ] 类型测试示例
- [ ] E0 实施交接文档（`doc/FUNDAMENTAL_IMPLEMENTATION/E0_IMPLEMENTATION_HANDOFF.md`）
- [ ] 更新 CHANGELOG.md

**验收**：

- `pnpm test` 全部通过
- 测试覆盖率 ≥ 80%
- 文档完整

---

## 3. 技术方案拆解

### 3.1 Block 类型设计

```typescript
// src/types/block.ts

/** Block 基础接口 */
interface BaseBlock {
  id: string;
  type: string;
  children?: Block[];
  /** PDF 来源信息（E4 阶段使用） */
  source?: {
    pageIndex: number;
    bbox: [number, number, number, number];
  };
}

/** 标准块类型枚举 */
type StandardBlockType =
  | "heading"
  | "paragraph"
  | "bulletListItem"
  | "numberedListItem"
  | "checkListItem"
  | "codeBlock"
  | "table"
  | "image"
  | "quote";

/** 自定义块类型枚举 */
type CustomBlockType =
  | "container"
  | "math"
  | "mermaid"
  | "vueComponent"
  | "include";

/** Heading 块 */
interface HeadingBlock extends BaseBlock {
  type: "heading";
  props: {
    level: 1 | 2 | 3 | 4 | 5 | 6;
  };
  content: InlineContent[];
}

/** 容器块（VitePress :::tip 等） */
interface ContainerBlock extends BaseBlock {
  type: "container";
  props: {
    containerType: "tip" | "warning" | "danger" | "details" | "info";
    title?: string;
  };
  children: Block[];
}

/** 数学公式块 */
interface MathBlock extends BaseBlock {
  type: "math";
  props: {
    formula: string;
    display: "inline" | "block";
  };
}

/** Mermaid 图表块 */
interface MermaidBlock extends BaseBlock {
  type: "mermaid";
  props: {
    code: string;
  };
}

/** Vue 组件块 */
interface VueComponentBlock extends BaseBlock {
  type: "vueComponent";
  props: {
    componentName: string;
    attributes: Record<string, string | number | boolean>;
  };
}

/** @include 指令块 */
interface IncludeBlock extends BaseBlock {
  type: "include";
  props: {
    path: string;
    lineRange?: { start?: number; end?: number };
    region?: string;
  };
}
```

### 3.2 Document 类型设计

```typescript
// src/types/document.ts

/** Frontmatter 元数据 */
interface Frontmatter {
  title?: string;
  description?: string;
  tags?: string[];
  date?: string;
  author?: string;
  layout?: string;
  [key: string]: unknown;
}

/** 文档结构 */
interface Document {
  /** 文件路径（相对于项目根目录） */
  path: string;
  /** Frontmatter 元数据 */
  frontmatter: Frontmatter;
  /** 文档内容块 */
  blocks: Block[];
  /** 最后修改时间 */
  lastModified?: string;
}
```

### 3.3 转换器架构

```
┌─────────────────┐     ┌──────────────────┐     ┌─────────────────┐
│   Markdown      │ ──► │   unified +      │ ──► │   Block[]       │
│   String        │     │   remark 插件     │     │                 │
└─────────────────┘     └──────────────────┘     └─────────────────┘
        ▲                                                  │
        │           ┌──────────────────┐                   │
        └────────── │   serializer     │ ◄─────────────────┘
                    └──────────────────┘
```

**解析流程**：

1. `remark-parse` → MDAST
2. `remark-frontmatter` → 提取 YAML Frontmatter
3. `remark-directive` → 识别 :::容器
4. `remark-math` → 识别 $$ 公式
5. 自定义 transformer → MDAST 节点 → Block 类型

**序列化流程**：

1. Block[] → MDAST 节点
2. `remark-stringify` → Markdown 字符串
3. 后处理：修复 VitePress 特有语法格式

---

## 4. 依赖安装命令

```bash
# 生产依赖
pnpm add unified remark-parse remark-stringify remark-directive remark-math remark-frontmatter yaml

# 开发依赖
pnpm add -D @types/mdast
```

---

## 5. 文件结构规划

```
src/
├── types/
│   ├── project.ts           现有文件（不修改）
│   ├── block.ts             [NEW] Block 类型定义
│   └── document.ts          [NEW] Document 类型定义
└── utils/
    ├── __tests__/
    │   ├── ...              现有测试文件
    │   └── markdown-converter.test.ts  [NEW] 转换器测试
    ├── markdown-to-blocks.ts    [NEW] Markdown → Block
    ├── blocks-to-markdown.ts    [NEW] Block → Markdown
    └── markdown-converter.ts    [NEW] 统一导出入口
```

---

## 6. 验证计划

### 6.1 自动化测试

**测试文件**：`src/utils/__tests__/markdown-converter.test.ts`

**测试用例**：

| 测试场景           | 验证内容                                     |
| ------------------ | -------------------------------------------- |
| 标准 Markdown 解析 | heading、paragraph、list、code、table、image |
| VitePress 容器解析 | :::tip、:::warning、:::danger、:::details    |
| LaTeX 公式解析     | 行内 $..$ 和块级 $$..$$                      |
| Mermaid 代码块识别 | ```mermaid 代码块                            |
| Vue 组件解析       | `<OList path="..."/>` 等自闭合标签           |
| @include 指令解析  | 完整文件、行范围、区域引用                   |
| Frontmatter 解析   | YAML 元数据提取                              |
| 往返一致性         | md → blocks → md 格式保持                    |
| 空文档处理         | 边界条件                                     |
| 嵌套结构           | 列表嵌套、容器内嵌套                         |

**运行命令**：

```bash
pnpm test -- src/utils/__tests__/markdown-converter.test.ts
```

### 6.2 集成验证

使用 fireworks-notes-society 仓库的真实 Markdown 文件进行测试：

```bash
# 在项目中临时添加验证脚本
# 解析真实文档并验证往返一致性
pnpm test -- --testPathPattern="markdown-converter"
```

### 6.3 手动验证

1. 检查 TypeScript 编译：`pnpm build`
2. 检查测试覆盖率：`pnpm test:cov`
3. 查看 Block 类型的 IDE 智能提示是否完整

---

## 7. 风险清单与缓解

| 风险                   | 表现                   | 缓解措施                              |
| ---------------------- | ---------------------- | ------------------------------------- |
| remark 插件版本不兼容  | unified 生态 ESM 问题  | 锁定兼容版本，统一使用 ESM            |
| VitePress 语法演进     | :::容器语法变化        | 参考 VitePress 官方文档，保持灵活扩展 |
| 往返转换信息丢失       | 格式差异（空格、换行） | 定义"语义等价"而非"字符等价"标准      |
| Vue 组件属性解析复杂度 | 多种属性绑定语法       | 初期仅支持静态属性，动态绑定推迟处理  |
| 嵌套结构深度           | 递归解析性能           | 设置最大嵌套深度限制                  |

---

## 8. 与后续阶段的衔接

| 后续阶段 | 依赖 E0 的内容                               |
| -------- | -------------------------------------------- |
| E1       | Block/Document 类型用于文档树展示            |
| E2       | markdownToBlocks/blocksToMarkdown 用于编辑器 |
| E4       | Block.source 字段用于 PDF 来源追踪           |

---

## 附：变更记录

- v1.0: 初版（E0 阶段细化规划）
