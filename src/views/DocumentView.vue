<script setup lang="ts">
/**
 * DocumentView - VitePress 项目文档视图
 * Redesigned with Card UI to match ProjectView
 */
import { ref, computed, onMounted, onUnmounted, watch, nextTick } from "vue";
import { useRoute, useRouter } from "vue-router";
import { listen } from "@tauri-apps/api/event";
import { useToastStore } from "../stores/toast";
import { useDocumentStore } from "../stores/document";
import { open } from "@tauri-apps/plugin-shell";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  type DocTreeNode,
  startDevServer,
  stopDevServer,
} from "../api/vitepress";

import BaseIcon from "../components/BaseIcon.vue";
import DocumentTree from "../components/document/DocumentTree.vue";
import InputModal from "../components/InputModal.vue";
import ConfirmModal from "../components/ConfirmModal.vue";

const route = useRoute();
const router = useRouter();
const toastStore = useToastStore();
const docStore = useDocumentStore();

// ============================================================================
// Store mapping
// ============================================================================
const worktreePath = computed(() => docStore.worktreePath);
const detection = computed(() => docStore.detection);

const needsInstall = computed(() => docStore.needsInstall);
const isInstalling = computed(() => docStore.isInstalling);
const installLogs = computed(() => docStore.installLogs);
const docTree = computed(() => docStore.docTree);
const loadingTree = computed(() => docStore.loadingTree);
const error = computed(() => docStore.error);
const selectedPath = computed(() => docStore.selectedPath);

// Editor related mapping (E2)
const currentBlocks = computed(() => docStore.currentBlocks);
const isDirty = computed(() => docStore.isDirty);
const isSaving = computed(() => docStore.isSaving);
const loadingContent = computed(() => docStore.loadingContent);

// Break the reactivity loop: Only update this when loading finishes
const staticInitialBlocks = ref<any[]>([]);
watch(loadingContent, (isLoading) => {
  if (!isLoading && docStore.currentBlocks) {
    // Determine if we should update static blocks.
    // We only update if it's a fresh load (loading just finished).
    // Using JSON parse/stringify to be safe.
    staticInitialBlocks.value = JSON.parse(
      JSON.stringify(docStore.currentBlocks)
    );
  }
});

import BlockEditor from "../components/editor/BlockEditor.vue";

async function handleSaveDocument() {
  try {
    await docStore.saveDocumentContent();
    toastStore.success("保存成功");
  } catch (e) {
    toastStore.error(`保存失败: ${e}`);
  }
}

function handleEditorChange(blocks: any[]) {
  docStore.updateEditorBlocks(blocks);
}

// Auto-save logic (E2.2)
let autoSaveTimer: ReturnType<typeof setTimeout> | null = null;
watch(
  () => docStore.isDirty,
  (dirty) => {
    if (dirty && !isSaving.value) {
      if (autoSaveTimer) clearTimeout(autoSaveTimer);
      autoSaveTimer = setTimeout(async () => {
        try {
          await docStore.saveDocumentContent();
          console.log("Auto-save successful");
        } catch (e) {
          console.error("Auto-save failed:", e);
        }
      }, 2000); // 2 seconds debounce
    }
  }
);

// Filtering
const showHiddenFiles = ref(false);
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
];

const filteredDocTree = computed(() => {
  if (!docTree.value) return null;

  const node = { ...docTree.value }; // Shallow copy

  if (node.children) {
    // 1. 始终过滤掉预览目录
    let children = node.children.filter(
      (child) => child.name !== "_fireworks_preview"
    );

    // 2. 如果未开启显示隐藏文件，过滤掉忽略列表中的文件
    if (!showHiddenFiles.value) {
      children = children.filter(
        (child) => !IGNORED_NAMES.includes(child.name)
      );
    }
    node.children = children;
  }
  return node;
});

const worktreeLabel = computed(() => {
  // 1. Try contentRoot (ignore "." or "./")
  if (
    detection.value?.contentRoot &&
    detection.value.contentRoot !== "." &&
    detection.value.contentRoot !== "./"
  ) {
    return detection.value.contentRoot;
  }

  // 2. Fallback to path parsing
  const raw = route.params.worktreePath;
  const pathStr = Array.isArray(raw) ? raw.join("/") : raw || "";
  if (!pathStr) return "Document";

  // Decode and normalize
  const decoded = decodeURIComponent(pathStr).replace(/\\/g, "/");
  // Remove trailing slashes
  const trimmed = decoded.replace(/\/+$/, "");
  // Split
  const parts = trimmed.split("/");

  let name = parts.pop();

  // If result is "." or empty, try parent
  if (name === "." || !name) {
    name = parts.pop();
  }

  return name || "Document";
});

// ============================================================================
// CRUD UI State
// ============================================================================
const contextMenu = ref({
  visible: false,
  x: 0,
  y: 0,
  node: null as DocTreeNode | null,
});

const showInputModal = ref(false);
const inputModalTitle = ref("");
const inputModalPlaceholder = ref("");
const inputModalValue = ref("");
const inputAction = ref<"new-file" | "new-folder" | "rename" | null>(null);

const isSidebarOpen = ref(true);
function toggleSidebar() {
  isSidebarOpen.value = !isSidebarOpen.value;
}

const showConfirmModal = ref(false);
const deleteTargetName = ref("");

// Install Progress Enhancement
const installLogRef = ref<HTMLDivElement | null>(null);
const installProgress = ref({
  resolved: 0,
  reused: 0,
  downloaded: 0,
  added: 0,
  done: false,
});

// ANSI 转 HTML（支持常见颜色）
function ansiToHtml(text: string): string {
  const ansiColors: Record<string, string> = {
    "30": "color:#000",
    "31": "color:#e74c3c",
    "32": "color:#2ecc71",
    "33": "color:#f1c40f",
    "34": "color:#3498db",
    "35": "color:#9b59b6",
    "36": "color:#1abc9c",
    "37": "color:#ecf0f1",
    "90": "color:#7f8c8d",
    "91": "color:#e74c3c",
    "92": "color:#2ecc71",
    "93": "color:#f1c40f",
    "94": "color:#3498db",
    "95": "color:#9b59b6",
    "96": "color:#1abc9c",
    "97": "color:#fff",
    "1": "font-weight:bold",
  };

  return text
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/\x1b\[([0-9;]+)m/g, (_, codes: string) => {
      const parts = codes.split(";");
      const styles = parts
        .map((c) => ansiColors[c])
        .filter(Boolean)
        .join(";");
      if (styles) return `<span style="${styles}">`;
      if (codes === "0" || codes === "") return "</span>";
      return "";
    })
    .replace(/\x1b\[0m/g, "</span>");
}

// 解析 pnpm 进度输出
function parsePnpmProgress(line: string) {
  // pnpm 进度格式可能包含 ANSI 码，先清理
  const cleanLine = line.replace(/\x1b\[[0-9;]*m/g, "");

  // 匹配格式如: Progress: resolved 150, reused 148, downloaded 2, added 150
  // 或: Progress: resolved 150, reused 148, downloaded 2, added 150, done
  const match = cleanLine.match(
    /Progress:\s*resolved\s+(\d+).*?reused\s+(\d+).*?downloaded\s+(\d+).*?added\s+(\d+)/i
  );
  if (match) {
    installProgress.value = {
      resolved: parseInt(match[1]) || 0,
      reused: parseInt(match[2]) || 0,
      downloaded: parseInt(match[3]) || 0,
      added: parseInt(match[4]) || 0,
      done: cleanLine.toLowerCase().includes("done"),
    };
  }
}

// 自动滚动到底部
function scrollLogsToBottom() {
  nextTick(() => {
    if (installLogRef.value) {
      installLogRef.value.scrollTop = installLogRef.value.scrollHeight;
    }
  });
}

// ============================================================================
// Actions
// ============================================================================

async function handleInstallDependencies() {
  if (isInstalling.value) return;

  // 重置进度状态
  installProgress.value = {
    resolved: 0,
    reused: 0,
    downloaded: 0,
    added: 0,
    done: false,
  };

  const unlistenProgress = await listen<string>(
    "vitepress://install-progress",
    (e) => {
      docStore.installLogs.push(e.payload);
      parsePnpmProgress(e.payload);
      scrollLogsToBottom();
    }
  );

  const unlistenFinish = await listen<boolean>(
    "vitepress://install-finish",
    (e) => {
      // 重置安装状态
      docStore.isInstalling = false;
      installProgress.value.done = true;

      if (e.payload) {
        toastStore.success("依赖安装成功");
        docStore.checkProjectDependencies();
      } else {
        toastStore.error("依赖安装失败");
      }
      unlistenProgress();
      unlistenFinish(); // Self-remove
    }
  );

  try {
    docStore.installLogs.length = 0;
    await docStore.installProjectDependencies();
  } catch (e) {
    toastStore.error(`启动安装失败: ${e}`);
    unlistenProgress();
    unlistenFinish();
  }
}

const isDevServerRunning = ref(false);
const devServerUrl = ref(""); // 完整的当前文档预览 URL
const devServerBaseUrl = ref(""); // 基础 URL（用于 Block 组件预览）
const devServerPid = ref<number | null>(null);
const isPreviewOpen = ref(false);
const previewIframeRef = ref<HTMLIFrameElement | null>(null);

async function handleStartDevServer() {
  if (!worktreePath.value || isDevServerRunning.value) return;

  try {
    toastStore.info("正在启动预览服务...");
    const info = await startDevServer(worktreePath.value);

    // Strip ANSI codes (handle both standard \x1b[...m and broken [...m sequences)
    const cleanUrl = info.url.replace(/(?:\x1b\[|\x9b\[|\[)[\d;]*m/g, "");

    isDevServerRunning.value = true;
    devServerBaseUrl.value = cleanUrl; // 基础 URL 用于 Block 组件
    devServerUrl.value = cleanUrl;
    devServerPid.value = info.processId;
    isPreviewOpen.value = true; // 自动展开预览栏

    // 如果有选中的文档，计算对应的预览 URL
    if (selectedPath.value) {
      const targetUrl = getPreviewUrlForDocument(selectedPath.value, cleanUrl);
      if (targetUrl) {
        devServerUrl.value = targetUrl;
      }
    }

    toastStore.success(`服务已启动: ${cleanUrl}`);
  } catch (e) {
    toastStore.error(`启动失败: ${e}`);
    isDevServerRunning.value = false;
  }
}

/** 计算文档对应的预览 URL */
function getPreviewUrlForDocument(
  docPath: string,
  baseUrl: string
): string | null {
  try {
    const basePath = worktreePath.value || "";
    let relativePath = docPath.replace(/\\/g, "/"); // 统一为正斜杠
    const baseNormalized = basePath.replace(/\\/g, "/");

    if (relativePath.startsWith(baseNormalized)) {
      relativePath = relativePath.slice(baseNormalized.length);
    }

    // 去掉开头的斜杠和 .md 后缀
    relativePath = relativePath.replace(/^\/+/, "").replace(/\.md$/i, "");

    // 如果是 index，VitePress 通常可以直接访问目录
    if (relativePath.endsWith("/index")) {
      relativePath = relativePath.slice(0, -6) + "/";
    }

    // 构建完整 URL
    const cleanBaseUrl = baseUrl.replace(/\/$/, "");
    return `${cleanBaseUrl}/${relativePath}`;
  } catch (e) {
    console.warn("Failed to calculate preview URL:", e);
    return null;
  }
}

async function handleStopDevServer() {
  if (!devServerPid.value) return;
  try {
    await stopDevServer(devServerPid.value, worktreePath.value || undefined);
    isDevServerRunning.value = false;
    devServerUrl.value = "";
    devServerBaseUrl.value = "";
    devServerPid.value = null;
    isPreviewOpen.value = false;
    toastStore.info("预览服务已停止");
  } catch (e) {
    toastStore.error(`停止失败: ${e}`);
  }
}

async function handleRestartDevServer() {
  if (!worktreePath.value) return;

  toastStore.info("正在重启预览服务...");

  // 先停止
  if (devServerPid.value) {
    try {
      await stopDevServer(devServerPid.value, worktreePath.value || undefined);
    } catch (e) {
      console.warn("Stop failed during restart:", e);
    }
  }

  // 重置 PID 但保留 URL 不变（防止 iframe 重载）
  devServerPid.value = null;
  // 注意：不清空 devServerUrl 和 isDevServerRunning，让 iframe 保持显示

  // 重新启动
  try {
    const info = await startDevServer(worktreePath.value);
    const cleanUrl = info.url.replace(/(?:\x1b\[|\x9b\[|\[)[\d;]*m/g, "");

    isDevServerRunning.value = true;
    devServerBaseUrl.value = cleanUrl;
    devServerUrl.value = cleanUrl;
    devServerPid.value = info.processId;

    // 刷新 iframe（保持当前路径）
    if (previewIframeRef.value) {
      try {
        previewIframeRef.value.contentWindow?.location.reload();
      } catch (e) {
        // 跨域可能失败，改用重新设置 src
        const currentSrc = previewIframeRef.value.src;
        previewIframeRef.value.src = "";
        previewIframeRef.value.src = currentSrc;
      }
    }

    toastStore.success(`服务已重启`);
  } catch (e) {
    toastStore.error(`重启失败: ${e}`);
    isDevServerRunning.value = false;
    devServerUrl.value = "";
    devServerBaseUrl.value = "";
  }
}

// 自动重启预览（文件结构变化时调用）
async function triggerPreviewRestartIfNeeded() {
  if (isDevServerRunning.value && devServerPid.value) {
    await handleRestartDevServer();
  }
}

function handleOpenBrowser() {
  if (devServerUrl.value) {
    open(devServerUrl.value);
  }
}

async function handleOpenInternal() {
  if (devServerUrl.value) {
    isPreviewOpen.value = true;
  }
}

function handleSelectDocument(node: DocTreeNode) {
  if (node.nodeType === "file") {
    docStore.selectDocument(node.path);

    // 同步预览 iframe 到对应页面
    if (isPreviewOpen.value && devServerUrl.value && previewIframeRef.value) {
      // 需要从当前 URL 中提取 base URL（去掉路径部分）
      const urlObj = new URL(devServerUrl.value);
      const baseUrl = `${urlObj.protocol}//${urlObj.host}`;
      const targetUrl = getPreviewUrlForDocument(node.path, baseUrl);
      if (targetUrl) {
        previewIframeRef.value.src = targetUrl;
      }
    }
  }
}

// Context Menu Logic
function closeContextMenu() {
  contextMenu.value.visible = false;
}

onMounted(async () => {
  document.addEventListener("click", closeContextMenu);

  // Auto maximize window
  try {
    const currentWindow = getCurrentWindow();
    await currentWindow.maximize();
  } catch (e) {
    console.warn("Failed to maximize window:", e);
  }

  // Initialize
  const path = Array.isArray(route.params.worktreePath)
    ? route.params.worktreePath.join("/")
    : route.params.worktreePath || "";

  if (path) {
    // 每次进入都重置状态，确保数据是最新的
    docStore.resetState();
    docStore.bindProject(path);
  }
});

onUnmounted(() => {
  document.removeEventListener("click", closeContextMenu);
  if (autoSaveTimer) clearTimeout(autoSaveTimer);
  docStore.resetState();
});

watch(
  () => route.params.worktreePath,
  (newPath) => {
    const path = Array.isArray(newPath) ? newPath.join("/") : newPath || "";
    if (path) {
      docStore.bindProject(path);
    }
  }
);

function handleContextMenu({
  event,
  node,
}: {
  event: MouseEvent;
  node: DocTreeNode;
}) {
  contextMenu.value = {
    visible: true,
    x: event.clientX,
    y: event.clientY,
    node,
  };
}

function handleContextAction(
  action: "new-file" | "new-folder" | "rename" | "delete"
) {
  closeContextMenu();
  const node = contextMenu.value.node;
  if (!node) return;

  switch (action) {
    case "new-file":
      inputAction.value = "new-file";
      inputModalTitle.value = "新建文件";
      inputModalPlaceholder.value = "请输入文件名 (例如: guide.md)";
      inputModalValue.value = "";
      showInputModal.value = true;
      break;
    case "new-folder":
      inputAction.value = "new-folder";
      inputModalTitle.value = "新建文件夹";
      inputModalPlaceholder.value = "请输入文件夹名";
      inputModalValue.value = "";
      showInputModal.value = true;
      break;
    case "rename":
      inputAction.value = "rename";
      inputModalTitle.value = "重命名";
      inputModalPlaceholder.value = "请输入新名称";
      inputModalValue.value = node.name;
      showInputModal.value = true;
      break;
    case "delete":
      deleteTargetName.value = node.name;
      showConfirmModal.value = true;
      break;
  }
}

function handleToolbarAction(action: "new-file" | "new-folder") {
  const contentRoot = detection.value?.contentRoot || "";
  const rootDir = worktreePath.value
    ? worktreePath.value + (contentRoot ? `/${contentRoot}` : "")
    : "";

  if (!rootDir) return;

  // Mock node for root context
  contextMenu.value.node = {
    path: rootDir,
    name: "root",
    nodeType: "folder",
    children: [],
    title: "Root",
    gitStatus: null,
    order: null,
  };

  if (action === "new-file") {
    inputAction.value = "new-file";
    inputModalTitle.value = "新建文档 (根目录)";
    inputModalPlaceholder.value = "请输入文件名 (例如: guide.md)";
  } else {
    inputAction.value = "new-folder";
    inputModalTitle.value = "新建文件夹 (根目录)";
    inputModalPlaceholder.value = "请输入文件夹名";
  }

  inputModalValue.value = "";
  showInputModal.value = true;
}

async function handleInputConfirm(value: string) {
  const node = contextMenu.value.node;
  if (!node) return;

  try {
    if (inputAction.value === "rename") {
      await docStore.renameItem(node.path, value);
      toastStore.success("重命名成功");
      // 重命名可能影响 sidebar，触发重启
      await triggerPreviewRestartIfNeeded();
    } else {
      let dir = node.path;
      if (node.nodeType === "file") {
        toastStore.warning("请在文件夹上右键新建");
        return;
      }

      if (inputAction.value === "new-file") {
        await docStore.createDocument(dir, value);
        toastStore.success("文件创建成功");
        // 新建文件影响 sidebar
        await triggerPreviewRestartIfNeeded();
      } else if (inputAction.value === "new-folder") {
        await docStore.createFolder(dir, value);
        toastStore.success("文件夹创建成功");
        // 新建文件夹影响 sidebar
        await triggerPreviewRestartIfNeeded();
      }
    }
  } catch (e) {
    toastStore.error(`操作失败: ${e}`);
  }
}

async function handleDeleteConfirm() {
  const node = contextMenu.value.node;
  if (!node) return;

  try {
    await docStore.deleteItem(node.path);
    toastStore.success("删除成功");
    // 删除文件影响 sidebar
    await triggerPreviewRestartIfNeeded();
  } catch (e) {
    toastStore.error(`删除失败: ${e}`);
  }
}
</script>

<template>
  <main class="page flex flex-col h-full overflow-hidden bg-base-100">
    <!-- Global Page Header -->
    <div
      class="flex items-center justify-between gap-4 h-14 flex-shrink-0 mb-2"
    >
      <div class="flex items-center gap-3">
        <button
          class="btn btn-sm btn-ghost btn-square"
          @click="toggleSidebar"
          :title="isSidebarOpen ? '收起侧边栏' : '展开侧边栏'"
        >
          <BaseIcon
            :icon="
              isSidebarOpen ? 'ph--sidebar-simple-fill' : 'ph--sidebar-simple'
            "
            size="md"
          />
        </button>
        <button
          class="btn btn-sm btn-ghost gap-1 font-normal"
          @click="router.push('/project')"
        >
          <BaseIcon icon="ph--arrow-left" size="sm" />
          返回
        </button>

        <h2 class="font-bold text-xl truncate max-w-md m-0!">文档编辑</h2>
        <span
          class="badge badge-neutral badge-sm badge-outline flex-shrink-0 gap-1 font-mono opacity-80"
          title="当前工作区"
        >
          <BaseIcon icon="lucide--git-branch" size="xs" />
          {{ worktreeLabel }}
        </span>

        <!-- Preview Controls -->
        <template v-if="!needsInstall">
          <!-- Running State: Dropdown -->
          <div
            v-if="isDevServerRunning"
            class="dropdown dropdown-end self-stretch"
          >
            <div
              tabindex="0"
              role="button"
              class="btn btn-xs btn-primary gap-1.5"
            >
              <BaseIcon icon="ph--check-circle" size="xs" />
              预览运行中
              <BaseIcon icon="ph--caret-down" size="xs" />
            </div>
            <ul
              tabindex="0"
              class="dropdown-content z-[2] menu menu-xs p-2 shadow-xl bg-base-100 rounded-xl w-48 border border-base-content/10 gap-0.5 mt-1 text-base-content/80 font-medium not-prose m-0 list-none [&_li>*]:rounded-md [&_li>*]:py-1.5 [&_li>*]:px-2"
            >
              <li>
                <a
                  class="border border-transparent hover:border-base-content/10 hover:bg-base-200"
                  @click="handleOpenBrowser"
                >
                  <BaseIcon icon="ph--globe" size="sm" /> 浏览器打开
                </a>
              </li>
              <li>
                <a
                  class="border border-transparent hover:border-base-content/10 hover:bg-base-200"
                  @click="handleOpenInternal"
                >
                  <BaseIcon icon="ph--browsers" size="sm" /> 内置预览
                </a>
              </li>
              <li>
                <a
                  class="border border-transparent hover:border-base-content/10 hover:bg-base-200"
                  @click="handleRestartDevServer"
                >
                  <BaseIcon icon="ph--arrow-clockwise" size="sm" /> 重启预览
                </a>
              </li>
              <li>
                <a
                  class="text-error hover:text-error border border-transparent hover:border-error/20 hover:bg-error/5"
                  @click="handleStopDevServer"
                >
                  <BaseIcon icon="ph--stop-circle" size="sm" /> 停止服务
                </a>
              </li>
            </ul>
          </div>

          <!-- Idle State: Start Button -->
          <button
            v-else
            class="btn btn-xs btn-primary gap-1.5 ml-2"
            @click="handleStartDevServer"
          >
            <BaseIcon icon="ph--play" size="xs" />
            启动预览
          </button>
        </template>
      </div>
    </div>

    <!-- Main Content Area (Card Layout) -->
    <div class="flex flex-1 overflow-hidden h-full">
      <!-- Sidebar Card (StatusCard Emulation) - Wrapper for animation -->
      <div
        class="flex-shrink-0 transition-all duration-300 ease-in-out overflow-hidden"
        :class="isSidebarOpen ? 'w-80 opacity-100 mr-4' : 'w-0 opacity-0 mr-0'"
      >
        <!-- Actual Content Card (Fixed Width to prevent internal reflow) -->
        <div
          class="w-80 flex flex-col card border-2 border-base-content/15 bg-base-100 h-full"
        >
          <!-- Card Header (Clean, no bg/border) -->
          <div
            class="h-14 flex items-center justify-between px-4 flex-shrink-0"
          >
            <h3
              class="font-bold text-base flex items-center gap-2 text-base-content m-0!"
            >
              <BaseIcon
                icon="ph--tree-structure"
                size="md"
                class="text-primary"
              />
              文档目录
            </h3>
            <div class="flex gap-0.5" v-if="!needsInstall">
              <div
                class="tooltip tooltip-left"
                :data-tip="showHiddenFiles ? '隐藏系统文件' : '显示所有文件'"
              >
                <button
                  class="btn btn-xs btn-ghost btn-square"
                  :class="{ 'text-primary bg-primary/10': showHiddenFiles }"
                  @click="showHiddenFiles = !showHiddenFiles"
                >
                  <BaseIcon
                    :icon="showHiddenFiles ? 'ph--eye' : 'ph--eye-slash'"
                    size="sm"
                  />
                </button>
              </div>
              <div class="w-px h-4 bg-base-content/10 mx-1 self-center"></div>
              <div class="tooltip tooltip-left" data-tip="新建文档">
                <button
                  class="btn btn-xs btn-ghost btn-square"
                  @click="handleToolbarAction('new-file')"
                >
                  <BaseIcon icon="ph--file-plus" size="sm" />
                </button>
              </div>
              <div class="tooltip tooltip-left" data-tip="新建文件夹">
                <button
                  class="btn btn-xs btn-ghost btn-square"
                  @click="handleToolbarAction('new-folder')"
                >
                  <BaseIcon icon="ph--folder-plus" size="sm" />
                </button>
              </div>
              <div class="w-px h-4 bg-base-content/10 mx-1 self-center"></div>
              <div class="tooltip tooltip-left" data-tip="重新安装依赖">
                <button
                  class="btn btn-xs btn-ghost btn-square"
                  @click="handleInstallDependencies"
                  :disabled="isInstalling"
                >
                  <span
                    v-if="isInstalling"
                    class="loading loading-spinner loading-xs"
                  ></span>
                  <BaseIcon v-else icon="ph--package" size="sm" />
                </button>
              </div>
            </div>
          </div>

          <!-- Scrollable Content -->
          <div
            class="flex-1 overflow-y-auto !overflow-x-hidden scrollbar-thin px-4 pb-4"
          >
            <!-- Loading -->
            <div
              v-if="loadingTree"
              class="h-full flex items-center justify-center"
            >
              <span
                class="loading loading-spinner loading-md text-primary/50"
              ></span>
            </div>

            <!-- Error -->
            <div
              v-else-if="error"
              class="h-full flex flex-col items-center justify-center text-error p-4 text-center"
            >
              <BaseIcon icon="ph--warning-circle" size="lg" class="mb-2" />
              <p class="text-xs">{{ error }}</p>
            </div>

            <!-- Install Needed or Installing -->
            <div
              v-else-if="needsInstall || isInstalling"
              class="h-full flex flex-col items-center justify-center gap-4 text-center p-4"
            >
              <div
                v-if="isInstalling"
                class="flex flex-col items-center gap-3 w-full"
              >
                <!-- 进度条 -->
                <div class="w-full" v-if="installProgress.resolved > 0">
                  <div
                    class="flex justify-between text-xs text-base-content/60 mb-1"
                  >
                    <span>已解析: {{ installProgress.resolved }}</span>
                    <span>已下载: {{ installProgress.downloaded }}</span>
                  </div>
                  <progress
                    class="progress progress-primary w-full"
                    :value="installProgress.added"
                    :max="installProgress.resolved || 100"
                  ></progress>
                  <div class="text-xs text-base-content/50 mt-1 text-center">
                    复用: {{ installProgress.reused }} | 新增:
                    {{ installProgress.added }}
                  </div>
                </div>
                <div v-else class="flex items-center gap-2">
                  <span
                    class="loading loading-spinner loading-md text-primary"
                  ></span>
                  <p class="text-sm">正在安装依赖...</p>
                </div>

                <!-- 终端输出（带颜色和自动滚动） -->
                <div
                  ref="installLogRef"
                  class="text-xs font-mono h-32 overflow-y-auto w-full bg-base-300 p-2 rounded text-left scrollbar-thin"
                >
                  <div
                    v-for="(log, i) in installLogs"
                    :key="i"
                    v-html="ansiToHtml(log)"
                  ></div>
                </div>
              </div>
              <template v-else>
                <BaseIcon
                  icon="ph--package"
                  size="xl"
                  class="text-warning opacity-80"
                />
                <p class="text-xs text-base-content/70">需要安装依赖才能继续</p>
                <button
                  class="btn btn-sm btn-primary w-full shadow-sm"
                  @click="handleInstallDependencies"
                >
                  安装依赖
                </button>
              </template>
            </div>

            <!-- Doc Tree -->
            <DocumentTree
              v-else-if="filteredDocTree"
              :tree="filteredDocTree"
              :loading="loadingTree"
              :selected-path="selectedPath"
              @select="handleSelectDocument"
              @contextmenu="handleContextMenu"
            />

            <!-- Empty -->
            <div
              v-else
              class="h-full flex flex-col items-center justify-center text-base-content/40"
            >
              <BaseIcon icon="ph--folder-dashed" size="xl" class="mb-2" />
              <p class="text-xs">暂无文档 (请右键新建)</p>
            </div>
          </div>
        </div>
      </div>

      <!-- Editor Card -->
      <div
        class="flex-1 flex flex-col card border-2 border-base-content/15 bg-base-100 overflow-hidden min-w-0"
      >
        <!-- Card Header (Clean) -->
        <div class="h-14 flex items-center px-4 justify-between flex-shrink-0">
          <div
            v-if="selectedPath"
            class="text-sm text-base-content/80 flex items-center gap-2 overflow-hidden"
          >
            <BaseIcon
              icon="ph--file-text"
              size="md"
              class="text-primary flex-shrink-0"
            />
            <span class="font-bold text-base truncate">{{
              selectedPath.split("/").pop()
            }}</span>
            <span class="text-base-content/40 text-xs truncate ml-2 max-w-xs">{{
              selectedPath
            }}</span>
          </div>
          <div v-else class="text-sm text-base-content/40 italic">
            未选择文档
          </div>

          <div class="flex items-center gap-2 shrink-0">
            <!-- Save Button -->
            <button
              v-if="selectedPath"
              class="btn btn-xs gap-1.5"
              :class="
                isDirty ? 'btn-primary shadow-sm' : 'btn-ghost opacity-50'
              "
              @click="handleSaveDocument"
              :disabled="!isDirty || isSaving"
            >
              <span
                v-if="isSaving"
                class="loading loading-spinner loading-xs"
              ></span>
              <BaseIcon
                v-else
                :icon="
                  isDirty ? 'ph--floppy-disk-back-fill' : 'ph--floppy-disk'
                "
                size="sm"
              />
              {{ isSaving ? "保存中..." : isDirty ? "保存" : "已保存" }}
            </button>

            <button
              v-if="isDevServerRunning && !isPreviewOpen"
              class="btn btn-sm btn-ghost btn-square"
              title="打开预览面板"
              @click="isPreviewOpen = true"
            >
              <BaseIcon
                icon="ph--sidebar-simple"
                size="md"
                class="rotate-180"
              />
            </button>
            <div class="badge badge-sm badge-outline">Editor E2</div>
          </div>
        </div>

        <!-- DEBUG INFO -->

        <!-- Main Content -->
        <div
          class="flex-1 relative border-t-2 border-dashed border-base-200 bg-base-50/10 m-4 rounded-lg overflow-hidden"
        >
          <!-- Empty State -->
          <div
            v-if="!selectedPath"
            class="absolute inset-0 flex flex-col items-center justify-center text-base-content/30 gap-6"
          >
            <div
              class="w-32 h-32 bg-base-100 rounded-full flex items-center justify-center shadow-sm border border-base-200"
            >
              <BaseIcon
                icon="ph--cursor-click"
                size="2xl"
                class="text-base-content/10"
              />
            </div>
            <div class="text-center">
              <p class="text-lg font-medium text-base-content/60">准备就绪</p>
              <p class="text-sm mt-1">从左侧选择文档以开始编辑</p>
            </div>
          </div>

          <!-- Loading State -->
          <div
            v-else-if="loadingContent"
            class="absolute inset-0 flex flex-col items-center justify-center text-base-content/30 gap-4"
          >
            <span
              class="loading loading-spinner loading-lg text-primary/30"
            ></span>
            <p class="text-sm">正在加载文档内容...</p>
          </div>

          <!-- Block Editor -->
          <div
            v-else
            class="absolute inset-0 flex flex-col p-4 overflow-hidden"
          >
            <BlockEditor
              :key="selectedPath || 'editor'"
              :initial-content="staticInitialBlocks"
              :file-path="selectedPath || undefined"
              :project-root="worktreePath || undefined"
              :dev-server-url="devServerBaseUrl || undefined"
              @change="handleEditorChange"
            />
          </div>
        </div>
      </div>

      <!-- Right Preview Pane (Collapsible) -->
      <div
        class="flex-shrink-0 transition-all duration-300 ease-in-out overflow-hidden"
        :class="
          isPreviewOpen ? 'w-[500px] opacity-100 ml-4' : 'w-0 opacity-0 ml-0'
        "
      >
        <div
          class="w-[500px] flex flex-col card border-2 border-base-content/15 bg-base-100 h-full"
        >
          <!-- Preview Header -->
          <div
            class="h-14 flex items-center justify-between px-4 flex-shrink-0"
          >
            <h3
              class="font-bold text-base flex items-center gap-2 text-base-content m-0!"
            >
              <BaseIcon icon="ph--browsers" size="md" class="text-secondary" />
              实时预览
            </h3>
            <div class="flex items-center gap-1">
              <button
                class="btn btn-xs btn-ghost btn-square"
                title="在浏览器打开"
                @click="handleOpenBrowser"
              >
                <BaseIcon icon="ph--globe" size="sm" />
              </button>
              <button
                class="btn btn-xs btn-ghost btn-square"
                title="关闭预览"
                @click="isPreviewOpen = false"
              >
                <BaseIcon icon="ph--x" size="sm" />
              </button>
            </div>
          </div>

          <!-- Iframe Content -->
          <div class="flex-1 bg-white relative">
            <iframe
              v-if="isDevServerRunning && devServerUrl"
              ref="previewIframeRef"
              :src="devServerUrl"
              class="absolute inset-0 w-full h-full border-none"
            ></iframe>
            <div
              v-else
              class="absolute inset-0 flex flex-col items-center justify-center text-base-content/40 bg-base-50"
            >
              <BaseIcon icon="ph--plug-slash" size="xl" class="mb-2" />
              <p class="text-xs">预览服务未启动</p>
            </div>
          </div>
        </div>
      </div>
    </div>

    <!-- Context Menu -->
    <div
      v-if="contextMenu.visible"
      class="fixed z-50 bg-base-100 min-w-[160px] max-w-xs rounded-xl shadow-xl border border-base-content/10 p-2 transform origin-top-left flex flex-col gap-0.5 not-prose text-base font-normal my-0 mx-0"
      :style="{ top: contextMenu.y + 'px', left: contextMenu.x + 'px' }"
      @click.stop
    >
      <ul
        class="menu menu-xs p-0 gap-0.5 w-full m-0 list-none [&_li>*]:rounded-md [&_li>*]:py-1.5 [&_li>*]:px-2 text-base-content/80 font-medium inset-0"
      >
        <!-- Folder Actions -->
        <template v-if="contextMenu.node?.nodeType === 'folder'">
          <li @click="handleContextAction('new-file')">
            <a
              class="border border-transparent hover:border-base-content/10 hover:bg-base-200 hover:text-base-content"
              ><BaseIcon icon="ph--file-plus" size="sm" />新建文件</a
            >
          </li>
          <li @click="handleContextAction('new-folder')">
            <a
              class="border border-transparent hover:border-base-content/10 hover:bg-base-200 hover:text-base-content"
              ><BaseIcon icon="ph--folder-plus" size="sm" />新建文件夹</a
            >
          </li>
          <div class="h-px bg-base-content/10 my-0.5 mx-1"></div>
        </template>

        <!-- Common Actions -->
        <li @click="handleContextAction('rename')">
          <a
            class="border border-transparent hover:border-base-content/10 hover:bg-base-200 hover:text-base-content"
            ><BaseIcon icon="ph--pencil" size="sm" />重命名</a
          >
        </li>
        <li @click="handleContextAction('delete')">
          <a
            class="text-error hover:text-error border border-transparent hover:border-error/20 hover:bg-error/5"
            ><BaseIcon icon="ph--trash" size="sm" />删除</a
          >
        </li>
      </ul>
    </div>

    <!-- Modals -->
    <InputModal
      v-model="showInputModal"
      :title="inputModalTitle"
      :default-value="inputModalValue"
      :placeholder="inputModalPlaceholder"
      @confirm="handleInputConfirm"
    />

    <ConfirmModal
      v-model="showConfirmModal"
      title="确认删除"
      confirm-text="删除"
      confirm-variant="error"
      @confirm="handleDeleteConfirm"
    >
      <p>确定要删除 {{ deleteTargetName }} 吗？</p>
      <p class="text-sm text-base-content/70 mt-2">此操作无法撤销。</p>
    </ConfirmModal>
  </main>
</template>
