# E1 阶段实施交接文档（VitePress 项目管理）

> 本文档记录 E1 阶段的实施成果，包括 VitePress 项目检测、配置解析、文档树管理、Dev Server 管理以及文档视图 UI。

---

## 1. 实施概述

**阶段目标**：建立 VitePress 项目管理基础设施，包括项目检测、依赖管理、文档树展示、实时预览和文档 CRUD 操作。

**完成日期**：2026-01-02

**状态**：✅ 已完成

---

## 2. 交付物清单

### 2.1 后端命令（Rust/Tauri）

| 文件                                      | 说明                                 |
| ----------------------------------------- | ------------------------------------ |
| `src-tauri/src/app/commands/vitepress.rs` | VitePress 命令模块（12+ Tauri 命令） |
| `src-tauri/src/app/setup.rs`              | 命令注册与状态管理                   |
| `src-tauri/capabilities/default.json`     | Shell 执行权限配置                   |

### 2.2 前端 API 层

| 文件                   | 说明                     |
| ---------------------- | ------------------------ |
| `src/api/vitepress.ts` | VitePress Tauri API 封装 |

### 2.3 状态管理

| 文件                     | 说明                        |
| ------------------------ | --------------------------- |
| `src/stores/document.ts` | 文档视图状态（Pinia Store） |

### 2.4 组件

| 文件                                           | 说明                 |
| ---------------------------------------------- | -------------------- |
| `src/views/DocumentView.vue`                   | 文档编辑视图主页面   |
| `src/components/document/DocumentTree.vue`     | 文档树容器组件       |
| `src/components/document/DocumentTreeItem.vue` | 文档树节点递归组件   |
| `src/components/ConfirmModal.vue`              | 确认对话框（优化版） |
| `src/components/InputModal.vue`                | 输入对话框           |

---

## 3. 后端命令详解

### 3.1 项目检测与配置

| 命令                             | 说明                                                  |
| -------------------------------- | ----------------------------------------------------- |
| `vitepress_detect_project`       | 检测指定路径是否为 VitePress 项目                     |
| `vitepress_parse_config`         | 解析 VitePress 配置（通过 Node.js 脚本执行）          |
| `vitepress_check_dependencies`   | 检查 pnpm 依赖安装状态                                |
| `vitepress_install_dependencies` | 删除 node_modules 后执行 `pnpm install`（带进度事件） |

### 3.2 文档树管理

| 命令                        | 说明                                    |
| --------------------------- | --------------------------------------- |
| `vitepress_get_doc_tree`    | 递归获取文档树（含 Git 状态、标题提取） |
| `vitepress_read_document`   | 读取文档内容（含 Frontmatter 解析）     |
| `vitepress_save_document`   | 保存文档内容                            |
| `vitepress_create_document` | 创建新 Markdown 文件（含默认模板）      |
| `vitepress_create_folder`   | 创建文件夹（自动创建 index.md）         |
| `vitepress_rename`          | 重命名文件/文件夹                       |
| `vitepress_delete`          | 删除文件/文件夹                         |

### 3.3 Dev Server 管理

| 命令                         | 说明                                        |
| ---------------------------- | ------------------------------------------- |
| `vitepress_start_dev_server` | 启动 VitePress Dev Server（自动提取 URL）   |
| `vitepress_stop_dev_server`  | 停止 Dev Server（Windows 使用 taskkill /T） |

### 3.4 状态管理结构

```rust
pub struct DevServerState {
    pub servers: Mutex<HashMap<u32, CommandChild>>,
}
```

用于跟踪运行中的 Dev Server 进程，支持多实例管理。

---

## 4. 前端 API 封装

### 4.1 类型定义

```typescript
interface VitePressDetection {
  isVitepress: boolean;
  configPath?: string;
  contentRoot?: string;
  projectName?: string;
}

interface DevServerInfo {
  url: string;
  port: number;
  processId: number;
  status: "starting" | "running" | "stopped" | "error";
}

interface DocTreeNode {
  name: string;
  path: string;
  nodeType: "file" | "folder";
  title?: string;
  children?: DocTreeNode[];
  gitStatus?: "clean" | "modified" | "staged" | "untracked" | "conflict";
  order?: number;
}
```

### 4.2 API 函数

```typescript
// 项目检测
detectProject(path: string): Promise<VitePressDetection>
parseConfig(projectPath: string): Promise<VitePressConfig>
checkDependencies(projectPath: string): Promise<DependencyStatus>
installDependencies(projectPath: string): Promise<void>

// 文档树
getDocTree(projectPath: string, contentRoot?: string): Promise<DocTreeNode>
readDocument(path: string): Promise<DocumentContent>
saveDocument(path: string, content: string): Promise<SaveResult>

// CRUD
createDocument(dir: string, name: string, template?: string): Promise<string>
createFolder(parent: string, name: string): Promise<string>
renameItem(oldPath: string, newName: string): Promise<string>
deleteItem(path: string): Promise<boolean>

// Dev Server
startDevServer(projectPath: string, port?: number): Promise<DevServerInfo>
stopDevServer(processId: number): Promise<void>
```

---

## 5. 文档视图功能

### 5.1 侧边栏

- **文档树展示**：递归展示 VitePress 项目的 Markdown 文件结构
- **Git 状态标记**：显示文件的 modified/staged/untracked/conflict 状态
- **标题提取**：从 Frontmatter 或 `# ` 标题自动提取显示名称
- **文件过滤**：默认隐藏系统文件夹（public、scripts、assets、README.md、CONTRIBUTING.md 等）
- **工具栏按钮**：显示/隐藏文件、新建文档、新建文件夹、重新安装依赖
- **DaisyUI Tooltip**：所有工具栏按钮使用 `tooltip tooltip-left` 显示提示

### 5.2 上下文菜单

| 操作       | 说明                          |
| ---------- | ----------------------------- |
| 新建文件   | 在选中文件夹下创建 .md 文件   |
| 新建文件夹 | 创建文件夹并自动生成 index.md |
| 重命名     | 重命名文件/文件夹             |
| 删除       | 删除文件/文件夹（带确认）     |

### 5.3 预览控制

| 功能       | 说明                              |
| ---------- | --------------------------------- |
| 启动预览   | 启动 VitePress Dev Server         |
| 浏览器打开 | 在系统浏览器中打开预览 URL        |
| 内置预览   | 在右侧面板中显示 iframe 预览      |
| 重启预览   | 停止并重新启动 Dev Server         |
| 停止服务   | 停止 Dev Server（完整进程树终止） |

### 5.4 预览同步

- **自动展开**：启动预览后自动展开右侧预览面板
- **文档同步**：在文档树中选择文件时，预览面板自动导航到对应页面
- **重启保持**：重启预览时保持当前 iframe 页面，完成后自动刷新
- **自动重启**：文件创建/删除/重命名时自动重启预览（适配 VitePress 自动 sidebar 插件）

### 5.5 依赖安装增强

- **真实进度条**：解析 pnpm 的 `Progress: resolved X, reused Y, downloaded Z, added N` 输出
- **彩色终端输出**：ANSI 转义码转 HTML，支持 16 色显示
- **自动滚动**：终端输出自动滚动到底部
- **干净重装**：重新安装依赖时先删除 `node_modules` 目录
- **FORCE_COLOR**：设置环境变量强制 pnpm 输出颜色

---

## 6. UI/UX 优化

### 6.1 预览下拉菜单

- 样式与上下文菜单保持一致
- 使用 DaisyUI dropdown 组件
- 统一的 padding 和 border-radius

### 6.2 ConfirmModal 改进

- 减少冗余 padding（`p-4` + `mt-4`）
- 更紧凑的布局
- 支持 `confirmVariant` 控制按钮颜色

### 6.3 工作区按钮修复

- 添加 `@click.stop` 防止事件冒泡
- 推送/删除按钮不再触发行点击导航

### 6.4 标题提取修复

- 修复 YAML 注释被误判为 Markdown 标题的 Bug
- 正确跳过 Frontmatter 区域后再搜索 `# ` 标题

---

## 7. 权限配置

### 7.1 Shell 执行权限

```json
{
  "identifier": "shell:allow-execute",
  "allow": [
    { "name": "node", "cmd": "node", "args": true },
    { "name": "pnpm", "cmd": "pnpm", "args": true },
    { "name": "cmd", "cmd": "cmd", "args": true }
  ]
}
```

### 7.2 其他权限

- `shell:allow-open`：使用系统浏览器打开 URL
- `window:allow-maximize`：窗口最大化

---

## 8. 技术要点

### 8.1 Windows 进程终止

VitePress Dev Server 通过 `cmd /C pnpm run docs:dev` 启动，形成进程树：

```
cmd.exe (PID: xxx)
  └── pnpm.cmd
        └── node.exe (vite)
```

单独调用 `child.kill()` 只能终止 `cmd.exe`，子进程成为孤儿进程。

**解决方案**：使用 `taskkill /F /T /PID xxx` 终止整个进程树。

```rust
#[cfg(target_os = "windows")]
{
    let _ = std::process::Command::new("taskkill")
        .args(["/F", "/T", "/PID", &process_id.to_string()])
        .output();
}
```

### 8.2 ANSI 转义码处理

VitePress Dev Server 和 pnpm 输出包含颜色转义码：

```typescript
// 清理 ANSI 码（用于 URL 提取）
const cleanUrl = info.url.replace(/(?:\x1b\[|\x9b\[|\[)[\d;]*m/g, "");

// ANSI 转 HTML（用于彩色终端显示）
function ansiToHtml(text: string): string {
  const ansiColors: Record<string, string> = {
    "31": "color:#e74c3c",
    "32": "color:#2ecc71",
    "33": "color:#f1c40f",
    "34": "color:#3498db",
    "35": "color:#9b59b6",
    "36": "color:#1abc9c",
    "1": "font-weight:bold",
  };
  return text.replace(/\x1b\[([0-9;]+)m/g, (_, codes) => {
    const styles = codes
      .split(";")
      .map((c) => ansiColors[c])
      .filter(Boolean)
      .join(";");
    return styles ? `<span style="${styles}">` : "</span>";
  });
}
```

### 8.3 文档路径到 URL 转换

```typescript
// 文件路径 → VitePress URL
// C:\project\课内笔记\微积分.md → /课内笔记/微积分
let relativePath = node.path.replace(/\\/g, "/");
relativePath = relativePath.replace(/^\/+/, "").replace(/\.md$/i, "");
const targetUrl = `${baseUrl}/${relativePath}`;
```

---

## 9. 测试验证

### 9.1 手动测试清单

- [x] 项目检测（VitePress 项目识别）
- [x] 依赖安装（pnpm install 流式输出）
- [x] 文档树加载（含 Git 状态）
- [x] 文件 CRUD（创建/重命名/删除）
- [x] Dev Server 启动/停止
- [x] 预览同步（选择文件 → iframe 导航）
- [x] 预览重启（保持页面状态）
- [x] 自动重启（文件变更后触发）

### 9.2 已知限制

- 标题提取仅支持简单的 YAML frontmatter 和 `# ` 标题
- 预览同步假设 VitePress 使用默认路由规则
- Windows 上需要 taskkill 才能完整终止进程

---

## 10. 后续阶段衔接

| 阶段        | 依赖 E1 的内容                                   |
| ----------- | ------------------------------------------------ |
| E2 块编辑器 | `readDocument`/`saveDocument` 用于编辑器数据读写 |
| E3 块工具栏 | 预览面板可用于实时查看编辑效果                   |
| E4 PDF 导入 | 使用 `createDocument` 创建导入的文档             |

---

## 附：文件变更总结

```diff
+ src-tauri/src/app/commands/vitepress.rs       # VitePress 命令模块
M src-tauri/src/app/setup.rs                    # 注册命令
M src-tauri/capabilities/default.json           # 权限配置

+ src/api/vitepress.ts                          # 前端 API 封装
+ src/stores/document.ts                        # 文档状态管理

+ src/views/DocumentView.vue                    # 文档编辑视图
+ src/components/document/DocumentTree.vue      # 文档树容器
+ src/components/document/DocumentTreeItem.vue  # 文档树节点

M src/components/ConfirmModal.vue               # 优化样式
M src/views/ProjectView.vue                     # 修复按钮点击

M src/router/index.ts                           # 添加 /document 路由
```

---

## 附录：配置示例

### VitePress 检测结果

```json
{
  "isVitepress": true,
  "configPath": ".vitepress/config.mts",
  "contentRoot": ".",
  "projectName": "fireworks-notes-society"
}
```

### 文档树节点

```json
{
  "name": "课内笔记",
  "path": "C:/Users/.../课内笔记",
  "nodeType": "folder",
  "title": "课内笔记",
  "children": [
    {
      "name": "微积分.md",
      "path": "C:/Users/.../课内笔记/微积分.md",
      "nodeType": "file",
      "title": "微积分入门",
      "gitStatus": "modified"
    }
  ]
}
```

---

## 9. 最终 UX 优化 (E1.9)

### 9.1 依赖安装体验

- **实时日志流**：
  - 前端监听 `vitepress://install-progress` 事件
  - 使用 `ansi-to-html` 解析 ANSI 颜色码，还原真实终端体验
  - 实现 `scrollLogsToBottom` 自动跟随输出
- **进度解析**：
  - 正则解析 pnpm 输出 `Progress: resolved (\d+), reused (\d+), downloaded (\d+), added (\d+)`
  - 动态计算并展示进度条 (progress-primary)
- **重新安装**：
  - 侧边栏新增 "重新安装依赖" 功能
  - 实现前端 Loading 状态与按钮禁用逻辑

### 9.2 工作区删除安全机制

- **脏状态检测优化**：
  - 后端 `git_status` 排除 `node_modules` 等非跟踪文件
  - 精确识别 Uncommitted Changes
- **强制删除保护**：
  - 检测到脏状态时弹出确认模态框 (`ConfirmModal`)
  - 修复了模态框关闭时过早重置删除状态导致的 Loading 动画丢失问题
  - 维持 `deletingWorktreePath` 状态直到删除操作完全结束
- **状态重置修复**：
  - 修复：删除工作区后重建同名工作区，DocumentView 显示旧状态
  - 方案：`docStore.resetState()` 现在会清除 `worktreePath`，强制 `bindProject` 触发重新初始化

### 9.3 UI 细节打磨

- **表单布局**：
  - ProjectView 新建分支表单统一使用 `flex-row items-center gap-3`
  - 修复 Label 与 Input 间距丢失问题
  - 修复 Select 在 Loading 状态下的交互逻辑
- **导航栏**：
  - DocumentView 返回按钮移除 `pl-0`，恢复标准按钮内边距和交互反馈
