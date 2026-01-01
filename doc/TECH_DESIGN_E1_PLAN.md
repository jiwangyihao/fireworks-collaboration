# E1 阶段细化路线图与开发计划（VitePress 项目集成）

> 本文档将 E1 阶段「VitePress 项目集成」拆解为可执行的任务清单，实现从工作区加载 VitePress 项目、解析配置、管理文档目录的完整功能。

---

## 0. 目标、范围与成功标准

### 目标

- 在 ProjectView 的工作区（worktree）中集成 VitePress 项目检测和加载
- 解析 VitePress 配置文件，提取站点信息
- 实现文档目录树展示和 CRUD 操作
- 集成 pnpm 依赖管理和 VitePress Dev Server 启动
- 为 E2（块编辑器）提供文档加载和保存接口

### 使用流程

```
用户在 ProjectView 中点击工作区（worktree）
    ↓
检测该 worktree 是否为 VitePress 项目
    ↓
若是 → 进入 DocumentView 视图
    ↓
检查并安装依赖（pnpm install）
    ↓
展示文档目录树 + 启动 VitePress Dev Server（可选）
    ↓
用户选择文档进行编辑
```

### 范围

**包含（E1）**

| 模块               | 说明                                           |
| ------------------ | ---------------------------------------------- |
| VitePress 项目检测 | 检测 worktree 中是否存在 `.vitepress/config.*` |
| 配置解析           | 解析站点标题、描述、导航、侧边栏规则           |
| 依赖管理           | 使用 pnpm 检测/安装项目依赖                    |
| Dev Server 管理    | 启动/停止 VitePress Dev Server                 |
| 文档目录树         | 展示 lessons/ 目录结构（学院 → 课程 → 章节）   |
| 文档 CRUD          | 创建/读取/重命名/删除 Markdown 文件和文件夹    |
| Git 状态集成       | 在目录树中标记文件的 Git 状态                  |
| 智能标题提取       | 从 index.md 或 # 标题提取文件夹/文件显示名称   |

**不包含（推迟）**

| 模块               | 推迟至 |
| ------------------ | ------ |
| 块编辑器           | E2     |
| VitePress 预览同步 | E2/E5  |
| PDF 导入           | E4     |

### 成功标准

1. 点击 worktree 能正确识别 VitePress 项目并进入 DocumentView
2. 依赖缺失时自动提示安装（pnpm install）
3. 文档目录树正确展示层级结构和 Git 状态
4. 文件 CRUD 操作正常工作
5. Dev Server 可正常启动/停止
6. 构建和测试通过

---

## 1. 项目现状分析

### 1.1 已有基础设施

| 模块                  | 文件                         | 说明                  |
| --------------------- | ---------------------------- | --------------------- |
| 工作区管理            | `src/stores/workspace.ts`    | worktree 列表管理     |
| 项目视图              | `src/views/ProjectView.vue`  | worktree 展示和操作   |
| Git 命令              | `src-tauri/.../git.rs`       | Git 操作后端          |
| 工作区后端            | `src-tauri/.../workspace.rs` | 工作区相关 Tauri 命令 |
| Block 类型（E0 产出） | `src/types/block.ts`         | 内容模型类型          |
| Document 类型         | `src/types/document.ts`      | 文档结构类型          |
| Markdown 转换器       | `src/utils/markdown-*.ts`    | Markdown 解析/序列化  |

### 1.2 需要新增的模块

```
src/
├── views/
│   └── DocumentView.vue              [NEW] 文档视图主入口
├── components/
│   └── document/                     [NEW] 文档相关组件
│       ├── DocumentTree.vue          目录树
│       ├── DocumentTreeItem.vue      树节点
│       ├── DocumentBreadcrumb.vue    面包屑导航
│       ├── DocumentContextMenu.vue   右键菜单
│       ├── DocumentToolbar.vue       工具栏
│       └── DevServerStatus.vue       Dev Server 状态
├── stores/
│   └── document.ts                   [NEW] 文档状态管理
├── api/
│   └── vitepress.ts                  [NEW] VitePress API 封装
└── utils/
    └── vitepress-config.ts           [NEW] 配置解析工具

src-tauri/src/app/commands/
    └── vitepress.rs                  [NEW] VitePress 后端命令
```

---

## 2. E1 分阶段与任务清单

### E1.1 VitePress 项目检测与路由（约 2 天）

**范围**：

- 实现 VitePress 项目检测逻辑
- 添加 DocumentView 路由
- 在 ProjectView 中集成入口

**交付物**：

- [ ] Tauri 命令：`vitepress_detect_project`
- [ ] 前端 API：`detectVitePressProject()`
- [ ] DocumentView.vue 基础框架
- [ ] Vue Router 配置（/document/:worktreePath）
- [ ] ProjectView 中 worktree 点击跳转逻辑

**检测逻辑**：

```rust
// src-tauri/src/app/commands/vitepress.rs

/// 检测指定路径是否为 VitePress 项目
#[tauri::command]
pub async fn vitepress_detect_project(path: String) -> Result<VitePressDetection, Error> {
    // 检查以下文件是否存在：
    // 1. .vitepress/config.mts
    // 2. .vitepress/config.mjs
    // 3. .vitepress/config.ts
    // 4. .vitepress/config.js
    // 返回：是否为 VitePress、配置文件路径、内容根目录
}
```

---

### E1.2 依赖管理与 Dev Server（约 3 天）

**范围**：

- 实现 pnpm 依赖检测和安装
- 实现 VitePress Dev Server 启动/停止

**交付物**：

- [ ] Tauri 命令：`vitepress_check_dependencies`
- [ ] Tauri 命令：`vitepress_install_dependencies`
- [ ] Tauri 命令：`vitepress_start_dev_server`
- [ ] Tauri 命令：`vitepress_stop_dev_server`
- [ ] 前端 DevServerStatus.vue 组件
- [ ] 依赖安装进度显示

**后端实现**：

```rust
/// 检查项目依赖状态
#[tauri::command]
pub async fn vitepress_check_dependencies(project_path: String) -> Result<DependencyStatus, Error> {
    // 检查 node_modules 是否存在
    // 检查 node_modules/.pnpm 是否存在（pnpm 特有）
    // 检查 package.json 与 pnpm-lock.yaml 是否匹配
}

/// 安装依赖（运行 pnpm install）
#[tauri::command]
pub async fn vitepress_install_dependencies(
    project_path: String,
    window: Window
) -> Result<(), Error> {
    // 使用 tauri-plugin-shell 执行 pnpm install
    // 通过事件发送进度到前端
}

/// 启动 VitePress Dev Server
#[tauri::command]
pub async fn vitepress_start_dev_server(
    project_path: String,
    port: Option<u16>,
    window: Window
) -> Result<DevServerInfo, Error> {
    // 执行 pnpm run docs:dev 或 pnpm vitepress dev
    // 返回 URL 和进程 ID
}

/// 停止 Dev Server
#[tauri::command]
pub async fn vitepress_stop_dev_server(process_id: u32) -> Result<(), Error>
```

**依赖状态类型**：

```typescript
interface DependencyStatus {
  installed: boolean;
  pnpmLockExists: boolean;
  nodeModulesExists: boolean;
  outdated: boolean;
  packageManager: "pnpm";
}

interface DevServerInfo {
  url: string;
  port: number;
  processId: number;
  status: "starting" | "running" | "stopped" | "error";
}
```

---

### E1.3 配置解析（约 2 天）

**范围**：

- 解析 VitePress 配置文件
- 提取站点信息和侧边栏规则

**交付物**：

- [ ] Tauri 命令：`vitepress_parse_config`
- [ ] 前端工具：`parseVitePressConfig()`
- [ ] 配置类型定义扩展

**配置解析策略**：

由于 VitePress 配置是 TypeScript/JavaScript 文件，直接在 Rust 中解析较复杂。采用以下策略：

1. **方案 A（推荐）**：在 Node.js 环境中执行配置文件提取脚本
2. **方案 B**：使用正则表达式提取关键字段（有限支持）
3. **方案 C**：要求用户提供配置 JSON 副本（不推荐）

**采用方案 A**：

```rust
/// 解析 VitePress 配置
#[tauri::command]
pub async fn vitepress_parse_config(project_path: String) -> Result<VitePressConfig, Error> {
    // 创建临时脚本，使用 Node.js 加载配置并输出 JSON
    // 执行脚本并解析输出
}
```

临时脚本模板：

```javascript
// 由 Tauri 生成并执行
import { fileURLToPath } from "url";
import { dirname, join } from "path";

const configPath = process.argv[2];
const config = await import(configPath);

console.log(
  JSON.stringify({
    title: config.default?.title,
    description: config.default?.description,
    srcDir: config.default?.srcDir || ".",
    srcExclude: config.default?.srcExclude || [],
    themeConfig: {
      nav: config.default?.themeConfig?.nav,
      // 注意：vitepress-sidebar 的配置需要特殊处理
    },
  })
);
```

---

### E1.4 文档目录树（约 3 天）

**范围**：

- 实现文档目录树组件
- 智能标题提取
- Git 状态集成

**交付物**：

- [ ] Tauri 命令：`vitepress_get_doc_tree`
- [ ] DocumentTree.vue 组件
- [ ] DocumentTreeItem.vue 组件
- [ ] 智能标题提取逻辑
- [ ] Git 状态颜色标记

**目录树数据结构**：

```rust
#[derive(Serialize)]
pub struct DocTreeNode {
    pub name: String,
    pub path: String,
    pub node_type: DocTreeNodeType, // "file" | "folder"
    pub title: Option<String>,
    pub children: Option<Vec<DocTreeNode>>,
    pub git_status: Option<GitStatus>,
    pub order: Option<i32>,
}

#[derive(Serialize)]
pub enum GitStatus {
    Clean,
    Modified,
    Staged,
    Untracked,
    Conflict,
}
```

**智能标题提取规则**：

1. **文件夹**：读取 `index.md` 的第一个 # 标题或 frontmatter.title
2. **Markdown 文件**：读取第一个 # 标题或 frontmatter.title
3. **降级**：使用文件名（去除 .md 后缀）

**Git 状态获取**：

```rust
/// 获取文档树（含 Git 状态）
#[tauri::command]
pub async fn vitepress_get_doc_tree(
    project_path: String,
    content_root: Option<String>
) -> Result<DocTreeNode, Error> {
    // 1. 遍历 lessons/ 或 srcDir 目录
    // 2. 过滤 srcExclude 配置的文件
    // 3. 获取每个文件的 Git 状态
    // 4. 提取标题
    // 5. 构建树结构
}
```

---

### E1.5 文档 CRUD 操作（约 2 天）

**范围**：

- 实现文档的创建、读取、重命名、删除

**交付物**：

- [ ] Tauri 命令：`vitepress_read_document`
- [ ] Tauri 命令：`vitepress_save_document`
- [ ] Tauri 命令：`vitepress_create_document`
- [ ] Tauri 命令：`vitepress_create_folder`
- [ ] Tauri 命令：`vitepress_rename`
- [ ] Tauri 命令：`vitepress_delete`
- [ ] DocumentContextMenu.vue 组件
- [ ] 文件操作确认对话框

**后端实现**：

```rust
/// 读取文档内容
#[tauri::command]
pub async fn vitepress_read_document(path: String) -> Result<DocumentContent, Error> {
    // 读取文件内容
    // 解析 frontmatter
    // 返回 { path, content, frontmatter }
}

/// 保存文档
#[tauri::command]
pub async fn vitepress_save_document(
    path: String,
    content: String,
    frontmatter: Option<Frontmatter>
) -> Result<SaveResult, Error> {
    // 如果有 frontmatter，序列化并添加到内容前面
    // 写入文件
    // 返回保存结果
}

/// 创建新文档
#[tauri::command]
pub async fn vitepress_create_document(
    dir: String,
    name: String,
    template: Option<String>
) -> Result<String, Error> {
    // 创建 .md 文件
    // 使用模板或默认内容
    // 返回文件路径
}

/// 创建文件夹
#[tauri::command]
pub async fn vitepress_create_folder(
    parent: String,
    name: String
) -> Result<String, Error> {
    // 创建文件夹
    // 可选：创建 index.md
    // 返回文件夹路径
}

/// 重命名文件或文件夹
#[tauri::command]
pub async fn vitepress_rename(
    old_path: String,
    new_name: String
) -> Result<String, Error>

/// 删除文件或文件夹
#[tauri::command]
pub async fn vitepress_delete(path: String) -> Result<bool, Error>
```

---

### E1.6 前端状态管理与视图整合（约 2 天）

**范围**：

- 实现 document store
- 整合所有组件到 DocumentView

**交付物**：

- [ ] `src/stores/document.ts`
- [ ] DocumentView.vue 完整实现
- [ ] DocumentBreadcrumb.vue
- [ ] DocumentToolbar.vue
- [ ] 前端 API 封装 `src/api/vitepress.ts`

**Store 设计**：

```typescript
// src/stores/document.ts
interface DocumentState {
  // 项目信息
  projectPath: string | null;
  projectConfig: VitePressConfig | null;

  // 依赖状态
  dependencyStatus: DependencyStatus | null;
  installingDependencies: boolean;

  // Dev Server
  devServer: DevServerInfo | null;

  // 文档树
  docTree: DocTreeNode | null;
  loadingTree: boolean;

  // 当前文档
  currentDocument: Document | null;
  currentPath: string | null;

  // UI 状态
  expandedFolders: Set<string>;
  selectedPath: string | null;

  // 错误
  lastError: string | null;
}
```

---

### E1.7 测试与文档（约 1 天）

**范围**：

- 编写单元测试
- 更新文档

**交付物**：

- [ ] 前端组件测试
- [ ] API 测试
- [ ] E1 交接文档
- [ ] 更新 CHANGELOG.md

---

## 3. 技术方案拆解

### 3.1 路由设计

```typescript
// src/router.ts
{
  path: '/document/:worktreePath(.*)',
  name: 'document',
  component: () => import('./views/DocumentView.vue'),
  meta: { requiresProject: true }
}
```

### 3.2 DocumentView 布局

```
┌─────────────────────────────────────────────────────────────┐
│  工具栏 [返回项目] [刷新] [新建] [Dev Server: 运行中 ●]     │
├─────────────────────────────────────────────────────────────┤
│  面包屑: 项目名 > 数学学院 > 数学分析 > 第一章.md          │
├────────────────────┬────────────────────────────────────────┤
│  文档目录树         │  （E2 阶段添加编辑器）                  │
│  ├── 数学学院       │                                        │
│  │   ├── index.md  │  当前未选择文档                         │
│  │   ├── 数学分析   │                                        │
│  │   │   ├── ...   │                                        │
│  │   └── 高等代数   │                                        │
│  └── 计算机学院     │                                        │
├────────────────────┴────────────────────────────────────────┤
│  状态栏: 共 42 个文档 | 3 个待提交 | Dev Server: localhost:5173 │
└─────────────────────────────────────────────────────────────┘
```

### 3.3 依赖安装流程

```typescript
async function ensureDependencies(projectPath: string) {
  const status = await checkDependencies(projectPath);

  if (!status.installed || status.outdated) {
    // 显示安装对话框
    const confirmed = await showInstallDialog();
    if (!confirmed) return false;

    // 执行安装
    await installDependencies(projectPath, (progress) => {
      updateProgressUI(progress);
    });
  }

  return true;
}
```

### 3.4 Dev Server 管理

```typescript
// 事件类型
type DevServerEvent =
  | { type: "starting" }
  | { type: "ready"; url: string; port: number }
  | { type: "output"; line: string }
  | { type: "error"; message: string }
  | { type: "stopped" };

// 监听事件
listen<DevServerEvent>("vitepress://dev-server", (event) => {
  switch (event.payload.type) {
    case "ready":
      devServerUrl.value = event.payload.url;
      break;
    case "error":
      showError(event.payload.message);
      break;
  }
});
```

---

## 4. 时间线

| 阶段     | 内容                  | 预计时间    |
| -------- | --------------------- | ----------- |
| E1.1     | 项目检测与路由        | 2 天        |
| E1.2     | 依赖管理与 Dev Server | 3 天        |
| E1.3     | 配置解析              | 2 天        |
| E1.4     | 文档目录树            | 3 天        |
| E1.5     | 文档 CRUD             | 2 天        |
| E1.6     | 前端状态与视图整合    | 2 天        |
| E1.7     | 测试与文档            | 1 天        |
| **总计** |                       | **约 2 周** |

---

## 5. 风险清单与缓解

| 风险                       | 表现             | 缓解措施                         |
| -------------------------- | ---------------- | -------------------------------- |
| Node.js 配置解析复杂       | ESM/CJS 兼容问题 | 使用临时脚本在 Node 环境执行     |
| pnpm 未安装                | 依赖安装失败     | 检测 pnpm 可用性，提示用户安装   |
| Dev Server 端口冲突        | 启动失败         | 支持自定义端口，自动选择可用端口 |
| 大型项目目录树性能         | 加载缓慢         | 懒加载子目录，虚拟滚动           |
| Git 状态获取慢             | UI 阻塞          | 异步获取，增量更新               |
| vitepress-sidebar 配置复杂 | 解析困难         | 仅解析基本配置，复杂场景降级     |

---

## 6. 与后续阶段的衔接

| 后续阶段 | 依赖 E1 的内容                            |
| -------- | ----------------------------------------- |
| E2       | DocumentView 作为编辑器容器，读写文档 API |
| E3       | 目录树中的 Git 状态展示，提交面板入口     |
| E4       | 新建文档 API 用于 PDF 导入后创建 Markdown |
| E5       | Dev Server 管理用于预览同步               |

---

## 附：变更记录

- v1.0: 初版（E1 阶段细化规划）
- v1.1: 添加技术细节附录（组件用法、权限配置、配置解析参考）

---

## 附录 A：DaisyUI 组件用法参考

### A.1 目录树组件（menu + details）

使用 DaisyUI 的 `menu` + `details` 组件实现可折叠的文件树结构：

```html
<ul class="menu menu-xs bg-base-200 rounded-box max-w-xs w-full">
  <!-- 文件项 -->
  <li>
    <a>
      <svg class="h-4 w-4" ...><!-- 文件图标 --></svg>
      resume.pdf
    </a>
  </li>

  <!-- 文件夹项（可折叠） -->
  <li>
    <details open>
      <summary>
        <svg class="h-4 w-4" ...><!-- 文件夹图标 --></svg>
        My Files
      </summary>
      <ul>
        <li><a>file1.md</a></li>
        <li>
          <details>
            <summary>Subfolder</summary>
            <ul>
              <li><a>nested.md</a></li>
            </ul>
          </details>
        </li>
      </ul>
    </details>
  </li>
</ul>
```

**Vue 组件设计**：

```vue
<!-- DocumentTreeItem.vue -->
<template>
  <li>
    <!-- 文件夹 -->
    <details v-if="node.nodeType === 'folder'" :open="isExpanded">
      <summary @click.prevent="toggleExpand" @contextmenu="showContextMenu">
        <BaseIcon :icon="isExpanded ? 'ph--folder-open' : 'ph--folder'" />
        <span :class="gitStatusClass">{{ node.title || node.name }}</span>
      </summary>
      <ul v-if="node.children">
        <DocumentTreeItem
          v-for="child in node.children"
          :key="child.path"
          :node="child"
        />
      </ul>
    </details>

    <!-- 文件 -->
    <a v-else @click="selectFile" @contextmenu="showContextMenu">
      <BaseIcon icon="ph--file-md" />
      <span :class="gitStatusClass">{{ node.title || node.name }}</span>
    </a>
  </li>
</template>
```

### A.2 面包屑导航（breadcrumbs）

```html
<div class="breadcrumbs text-sm">
  <ul>
    <li>
      <a @click="navigateTo('/')">
        <BaseIcon icon="ph--folder" size="sm" />
        项目根目录
      </a>
    </li>
    <li>
      <a @click="navigateTo('/数学学院')">数学学院</a>
    </li>
    <li>
      <span>数学分析</span>
    </li>
  </ul>
</div>
```

### A.3 右键菜单（dropdown）

```html
<div class="dropdown dropdown-end" v-if="showMenu">
  <ul
    class="dropdown-content menu bg-base-100 rounded-box z-10 w-52 p-2 shadow"
  >
    <li>
      <a @click="createFile"><BaseIcon icon="ph--file-plus" /> 新建文档</a>
    </li>
    <li>
      <a @click="createFolder"
        ><BaseIcon icon="ph--folder-plus" /> 新建文件夹</a
      >
    </li>
    <li class="divider"></li>
    <li>
      <a @click="rename"><BaseIcon icon="ph--pencil" /> 重命名</a>
    </li>
    <li>
      <a @click="deleteItem" class="text-error"
        ><BaseIcon icon="ph--trash" /> 删除</a
      >
    </li>
  </ul>
</div>
```

### A.4 Git 状态样式

```css
/* Git 状态颜色 */
.git-modified {
  color: oklch(var(--wa));
} /* warning 黄色 */
.git-staged {
  color: oklch(var(--su));
} /* success 绿色 */
.git-untracked {
  color: oklch(var(--n));
} /* neutral 灰色 */
.git-conflict {
  color: oklch(var(--er));
} /* error 红色 */
```

---

## 附录 B：Tauri Shell 权限配置

### B.1 Capabilities 配置

在 `src-tauri/capabilities/default.json` 中添加 shell 权限：

```json
{
  "$schema": "../gen/schemas/desktop-schema.json",
  "identifier": "default",
  "description": "Default capabilities",
  "windows": ["main"],
  "permissions": [
    "shell:default",
    {
      "identifier": "shell:allow-spawn",
      "allow": [
        {
          "name": "pnpm",
          "cmd": "pnpm",
          "args": true,
          "sidecar": false
        },
        {
          "name": "node",
          "cmd": "node",
          "args": true,
          "sidecar": false
        }
      ]
    },
    {
      "identifier": "shell:allow-execute",
      "allow": [
        {
          "name": "pnpm-install",
          "cmd": "pnpm",
          "args": ["install"],
          "sidecar": false
        },
        {
          "name": "pnpm-dev",
          "cmd": "pnpm",
          "args": ["run", { "validator": "^docs?:dev$" }],
          "sidecar": false
        }
      ]
    },
    "shell:allow-kill"
  ]
}
```

### B.2 前端 Shell API 使用

```typescript
// src/api/vitepress.ts
import { Command } from "@tauri-apps/plugin-shell";
import { listen } from "@tauri-apps/api/event";

/**
 * 安装依赖（pnpm install）
 */
export async function installDependencies(
  projectPath: string,
  onProgress: (line: string) => void
): Promise<void> {
  const command = Command.create("pnpm", ["install"], {
    cwd: projectPath,
  });

  command.stdout.on("data", (line) => onProgress(line));
  command.stderr.on("data", (line) => onProgress(line));

  return new Promise((resolve, reject) => {
    command.on("close", (data) => {
      if (data.code === 0) {
        resolve();
      } else {
        reject(new Error(`pnpm install failed with code ${data.code}`));
      }
    });
    command.on("error", reject);
    command.spawn();
  });
}

/**
 * 启动 Dev Server
 */
export async function startDevServer(
  projectPath: string,
  port: number = 5173
): Promise<{ child: Child; url: string }> {
  const command = Command.create(
    "pnpm",
    ["run", "docs:dev", "--port", String(port)],
    {
      cwd: projectPath,
    }
  );

  const child = await command.spawn();

  // 监听输出，检测 ready 信号
  return new Promise((resolve) => {
    command.stdout.on("data", (line) => {
      if (line.includes("Local:")) {
        const match = line.match(/http:\/\/localhost:(\d+)/);
        if (match) {
          resolve({ child, url: `http://localhost:${match[1]}` });
        }
      }
    });
  });
}
```

---

## 附录 C：VitePress 配置解析参考

### C.1 VitePress 配置结构

```typescript
// VitePress 配置类型（来自官方文档）
import { defineConfig } from "vitepress";

export default defineConfig({
  // 站点元数据
  title: "My Docs",
  description: "Documentation site",
  lang: "zh-CN",

  // 目录设置
  srcDir: "./lessons", // 内容根目录（相对于项目根）
  outDir: "./dist", // 构建输出目录
  srcExclude: ["**/README.md", "**/CHANGELOG.md"],

  // 主题配置
  themeConfig: {
    nav: [
      { text: "指南", link: "/guide/" },
      { text: "API", link: "/api/" },
    ],
    sidebar: {
      "/guide/": [
        {
          text: "介绍",
          items: [
            { text: "什么是 VitePress?", link: "/guide/what-is-vitepress" },
            { text: "快速开始", link: "/guide/getting-started" },
          ],
        },
      ],
    },
  },
});
```

### C.2 配置提取脚本

用于在 Tauri 后端执行的 Node.js 脚本：

```javascript
// extract-config.mjs（由 Tauri 动态生成）
import { pathToFileURL } from "url";

const configPath = process.argv[2];
const configUrl = pathToFileURL(configPath).href;

try {
  const config = await import(configUrl);
  const cfg = config.default || config;

  const result = {
    title: cfg.title || "",
    description: cfg.description || "",
    lang: cfg.lang || "en-US",
    srcDir: cfg.srcDir || ".",
    srcExclude: cfg.srcExclude || [],
    themeConfig: {
      nav: cfg.themeConfig?.nav || [],
      sidebar: cfg.themeConfig?.sidebar || {},
      logo: cfg.themeConfig?.logo || null,
    },
  };

  console.log(JSON.stringify(result));
} catch (error) {
  console.error(JSON.stringify({ error: error.message }));
  process.exit(1);
}
```

### C.3 Rust 端调用

```rust
use tokio::process::Command;
use std::path::Path;

pub async fn parse_vitepress_config(project_path: &str) -> Result<VitePressConfig, Error> {
    // 查找配置文件
    let config_path = find_config_file(project_path)?;

    // 创建临时提取脚本
    let script = include_str!("../scripts/extract-config.mjs");
    let script_path = std::env::temp_dir().join("extract-config.mjs");
    std::fs::write(&script_path, script)?;

    // 执行脚本
    let output = Command::new("node")
        .arg(&script_path)
        .arg(&config_path)
        .current_dir(project_path)
        .output()
        .await?;

    if !output.status.success() {
        return Err(Error::ConfigParseError(
            String::from_utf8_lossy(&output.stderr).to_string()
        ));
    }

    // 解析 JSON 输出
    let config: VitePressConfig = serde_json::from_slice(&output.stdout)?;
    Ok(config)
}

fn find_config_file(project_path: &str) -> Result<String, Error> {
    let candidates = [
        ".vitepress/config.mts",
        ".vitepress/config.mjs",
        ".vitepress/config.ts",
        ".vitepress/config.js",
    ];

    for candidate in candidates {
        let path = Path::new(project_path).join(candidate);
        if path.exists() {
            return Ok(path.to_string_lossy().to_string());
        }
    }

    Err(Error::ConfigNotFound)
}
```

---

## 附录 D：依赖检测逻辑

```rust
#[derive(Serialize)]
pub struct DependencyStatus {
    pub installed: bool,
    pub pnpm_lock_exists: bool,
    pub node_modules_exists: bool,
    pub pnpm_store_exists: bool,
    pub outdated: bool,
}

pub async fn check_dependencies(project_path: &str) -> Result<DependencyStatus, Error> {
    let project = Path::new(project_path);

    let pnpm_lock = project.join("pnpm-lock.yaml");
    let node_modules = project.join("node_modules");
    let pnpm_store = node_modules.join(".pnpm");

    let pnpm_lock_exists = pnpm_lock.exists();
    let node_modules_exists = node_modules.exists();
    let pnpm_store_exists = pnpm_store.exists();

    // 简单判断：node_modules/.pnpm 存在则认为已安装
    let installed = pnpm_lock_exists && pnpm_store_exists;

    // TODO: 更精确的 outdated 检测需要比较 pnpm-lock.yaml 的修改时间
    let outdated = false;

    Ok(DependencyStatus {
        installed,
        pnpm_lock_exists,
        node_modules_exists,
        pnpm_store_exists,
        outdated,
    })
}
```
