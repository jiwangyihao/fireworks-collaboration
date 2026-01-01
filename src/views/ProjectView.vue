```vue
<script setup lang="ts">
import { onMounted, ref, computed, inject, Ref, watch, reactive } from "vue";
import { storeToRefs } from "pinia";
import { useProjectStore } from "../stores/project";
import { useToastStore } from "../stores/toast";
import { invoke } from "../api/tauri";

// 导入可复用组件
import StatusCard from "../components/StatusCard.vue";
import SyncStatusBadge from "../components/SyncStatusBadge.vue";
import LanguageBar from "../components/LanguageBar.vue";
import ConfirmModal from "../components/ConfirmModal.vue";
import EmptyState from "../components/EmptyState.vue";
import AvatarGroup, { type AvatarItem } from "../components/AvatarGroup.vue";
import BaseIcon from "../components/BaseIcon.vue";
import { formatNumber, relativeTime } from "../utils/format";

const projectStore = useProjectStore();
const {
  upstreamRepo,
  forkRepo,
  hasFork,
  localStatus,
  contributors,
  languages,
  latestRelease,
  forkBranches,
  forkCommits,
  loadingState,
  lastError,
} = storeToRefs(projectStore);

const authenticated = inject<Ref<boolean>>("authenticated", ref(false));
const toastStore = useToastStore();

// 监听错误变化，自动显示toast
watch(lastError, (error) => {
  if (error) {
    toastStore.error(error);
    projectStore.setError(null); // 清除store中的错误
  }
});

// 计算属性
const isLoading = computed(() => loadingState.value !== "idle");

// 贡献者头像列表（转换为 AvatarItem 格式）
const contributorAvatars = computed<AvatarItem[]>(() =>
  contributors.value.map((c) => ({
    avatarUrl: c.avatar_url,
    name: c.login,
    url: c.html_url,
  }))
);

// 工作区创建表单
const showWorktreeForm = ref(false);
const worktreeCreateMode = ref<"new" | "remote">("new"); // 创建模式: new=新建, remote=从远端
const worktreeForm = ref({
  branch: "", // 新建模式时的分支名
  selectedRemoteBranch: "", // 远端模式时选择的远端分支
});

// 远端分支列表
const remoteBranches = ref<string[]>([]);
const loadingRemoteBranches = ref(false);

// 加载远端分支
async function loadRemoteBranches() {
  if (!localStatus.value?.path) return;

  loadingRemoteBranches.value = true;
  try {
    const { getRemoteBranches } = await import("../api/tasks");
    remoteBranches.value = await getRemoteBranches(
      localStatus.value.path,
      "origin",
      true // fetch first
    );
  } catch (error) {
    console.error("加载远端分支失败:", error);
    toastStore.error("加载远端分支失败");
  } finally {
    loadingRemoteBranches.value = false;
  }
}

// 从远端分支名提取本地分支名 (origin/feature-x -> feature-x)
function extractBranchName(remoteBranch: string): string {
  const parts = remoteBranch.split("/");
  return parts.length > 1 ? parts.slice(1).join("/") : remoteBranch;
}

// 删除确认对话框
const showDeleteModal = ref(false);
const pendingDeletePath = ref<string>("");

// 创建工作区
async function handleCreateWorktree() {
  if (!localStatus.value?.path) {
    toastStore.error("请先克隆仓库");
    return;
  }

  // 根据模式确定分支名和远端引用
  let branchName: string;
  let fromRemote: string | undefined;

  if (worktreeCreateMode.value === "new") {
    // 新建模式：用户输入分支名
    if (!worktreeForm.value.branch) {
      toastStore.error("请输入分支名称");
      return;
    }
    branchName = worktreeForm.value.branch;
    fromRemote = undefined;
  } else {
    // 远端模式：从远端分支创建
    if (!worktreeForm.value.selectedRemoteBranch) {
      toastStore.error("请选择远端分支");
      return;
    }
    // 自动提取分支名 (origin/feature-x -> feature-x)
    branchName = extractBranchName(worktreeForm.value.selectedRemoteBranch);
    fromRemote = worktreeForm.value.selectedRemoteBranch;
  }

  try {
    const { addWorktree } = await import("../api/tasks");
    const { appDataDir } = await import("@tauri-apps/api/path");

    const dataDir = await appDataDir();
    const worktreePath = `${dataDir}/worktrees/${branchName}`;

    await addWorktree(
      localStatus.value.path,
      worktreePath,
      branchName,
      true, // 创建新分支
      fromRemote // 从远端分支创建（如果有）
    );

    const message = fromRemote
      ? `工作区 ${branchName} 创建成功（基于 ${fromRemote}）`
      : `工作区 ${branchName} 创建成功`;
    toastStore.success(message);
    showWorktreeForm.value = false;
    worktreeForm.value = { branch: "", selectedRemoteBranch: "" };
    worktreeCreateMode.value = "new";

    // 刷新本地仓库状态
    await projectStore.checkLocalRepo();
  } catch (error: any) {
    console.error("创建工作区失败:", error);
    toastStore.error(`创建工作区失败: ${error.message || error}`);
  }
}

// 显示删除确认对话框
function showDeleteConfirm(worktreePath: string) {
  pendingDeletePath.value = worktreePath;
  deleteRemoteBranch.value = false; // 重置选项
  showDeleteModal.value = true;
}

// 删除远端分支选项
const deleteRemoteBranch = ref(false);

// 确认删除工作区
async function confirmDeleteWorktree() {
  if (!localStatus.value?.path || !pendingDeletePath.value) {
    toastStore.error("参数错误");
    return;
  }

  // 从worktrees列表中获取要删除的分支名
  const worktreeToDelete = localStatus.value.worktrees?.find(
    (wt) => wt.path === pendingDeletePath.value
  );
  const branchToDelete = worktreeToDelete?.branch;

  try {
    const { removeWorktree, deleteGitBranch } = await import("../api/tasks");

    // 删除 worktree（如果需要删除远端分支，也在这里处理）
    await removeWorktree(
      localStatus.value.path,
      pendingDeletePath.value,
      false, // force
      deleteRemoteBranch.value, // delete_remote_branch
      "origin", // remote
      true // use_stored_credential
    );

    // 删除本地分支（如果有分支名）
    if (branchToDelete && branchToDelete !== "(detached)") {
      try {
        await deleteGitBranch(localStatus.value.path, branchToDelete, true);
      } catch (e) {
        console.warn("删除分支失败（可能分支不存在）:", e);
      }
    }

    toastStore.success(
      deleteRemoteBranch.value
        ? "工作区已删除，远端分支也已删除"
        : "工作区已删除"
    );
    await projectStore.checkLocalRepo();
  } catch (error: any) {
    console.error("删除工作区失败:", error);
    toastStore.error(`删除工作区失败: ${error.message || error}`);
  } finally {
    pendingDeletePath.value = "";
    deleteRemoteBranch.value = false;
    showDeleteModal.value = false;
  }
}

// Fork仓库
async function handleFork() {
  try {
    await projectStore.forkUpstream();
  } catch (error) {
    console.error("Fork失败:", error);
  }
}

// 同步Fork
async function handleSyncFork() {
  try {
    await projectStore.syncForkRepo();
    toastStore.success("同步成功");
  } catch (error) {
    console.error("同步失败:", error);
  }
}

// 强制同步Fork（丢弃所有变更）
async function handleForceSyncFork() {
  if (
    !confirm(
      "⚠️ 强制同步将丢弃 Fork 仓库中的所有变更，使其与上游完全一致。\n\n此操作不可撤销，确定继续吗？"
    )
  ) {
    return;
  }
  try {
    await projectStore.forceSyncForkRepo();
    toastStore.success("强制同步成功");
  } catch (error) {
    console.error("强制同步失败:", error);
  }
}

// Clone仓库
// 引入任务Store
import { useTasksStore } from "../stores/tasks";
const tasksStore = useTasksStore();
const cloningTaskId = ref<string | null>(null);

const cloneProgressDetails = computed(() => {
  if (!cloningTaskId.value) return null;
  const progress = tasksStore.progressById[cloningTaskId.value];
  return progress
    ? { percent: progress.percent, phase: progress.phase }
    : { percent: 0, phase: "Starting..." };
});

// 监听克隆任务状态
watch(
  () => {
    if (!cloningTaskId.value) return null;
    const task = tasksStore.items.find((t) => t.id === cloningTaskId.value);
    return task?.state;
  },
  async (newState) => {
    if (newState === "completed") {
      await projectStore.checkLocalRepo();
      toastStore.success("克隆成功");
      projectStore.loadingState = "idle";
      cloningTaskId.value = null;
    } else if (newState === "failed") {
      const err = tasksStore.lastErrorById[cloningTaskId.value!];
      toastStore.error(`克隆失败: ${err?.message || "未知错误"}`);
      projectStore.loadingState = "idle";
      cloningTaskId.value = null;
    }
  }
);

async function handleClone() {
  if (!projectStore.hasFork || !projectStore.forkRepo) {
    toastStore.error("请先Fork仓库");
    return;
  }

  // 防止重复点击
  if (projectStore.loadingState === "cloning") return;

  try {
    projectStore.loadingState = "cloning";
    const clonePath = await projectStore.getClonePath();
    const cloneUrl =
      projectStore.forkRepo.clone_url ||
      projectStore.forkRepo.html_url + ".git";

    // 调用后端Git clone命令 (返回TaskId)
    const { startGitClone } = await import("../api/tasks");
    const taskId = await startGitClone(cloneUrl, clonePath);
    cloningTaskId.value = taskId;

    toastStore.success("克隆任务已启动");
    // 不再这里手动idle，交由watch处理
  } catch (error: any) {
    console.error("启动克隆失败:", error);
    toastStore.error(`启动克隆失败: ${error.message || error}`);
    projectStore.loadingState = "idle";
  }
}

// 同步本地仓库（fetch）
async function handleFetch() {
  if (!localStatus.value?.path || !projectStore.forkRepo) {
    toastStore.error("本地仓库不存在");
    return;
  }

  try {
    projectStore.loadingState = "fetching" as any;
    const { startGitFetch } = await import("../api/tasks");
    const cloneUrl =
      projectStore.forkRepo.clone_url ||
      projectStore.forkRepo.html_url + ".git";
    await startGitFetch(cloneUrl, localStatus.value.path);
    toastStore.success("同步任务已启动");

    // 延迟刷新
    setTimeout(async () => {
      await projectStore.checkLocalRepo();
    }, 2000);
  } catch (error: any) {
    console.error("同步失败:", error);
    toastStore.error(`同步失败: ${error.message || error}`);
  } finally {
    projectStore.loadingState = "idle";
  }
}

// 推送本地仓库（push）
async function handlePush() {
  if (!localStatus.value?.path) {
    toastStore.error("本地仓库不存在");
    return;
  }

  try {
    projectStore.loadingState = "pushing" as any;
    const { startGitPush } = await import("../api/tasks");
    await startGitPush({
      dest: localStatus.value.path,
      useStoredCredential: true,
    });
    toastStore.success("推送任务已启动");

    // 延迟刷新
    setTimeout(async () => {
      await projectStore.checkLocalRepo();
    }, 2000);
  } catch (error: any) {
    console.error("推送失败:", error);
    toastStore.error(`推送失败: ${error.message || error}`);
  } finally {
    projectStore.loadingState = "idle";
  }
}

const pushingWorktreePaths = reactive(new Set<string>());

async function handlePushWorktree(wtPath: string) {
  if (pushingWorktreePaths.has(wtPath)) return;

  try {
    // Debug: 检查凭据是否存在
    try {
      const creds = await invoke("get_credential", {
        host: "github.com",
        username: null,
      });
      if (!creds) {
        toastStore.error("未找到 GitHub 凭据，正在尝试重新同步...");
        const token = await import("../utils/github-auth").then((m) =>
          m.loadAccessToken()
        );
        if (token) {
          await import("../utils/github-auth").then((m) =>
            m.syncCredentialToBackend(token)
          );
          toastStore.success("凭据已重新同步，请重试");
        } else {
          toastStore.error("无法获取 Access Token，请重新登录");
        }
        return;
      }
    } catch (e) {
      console.error("Check credential failed:", e);
    }

    pushingWorktreePaths.add(wtPath);
    const { startGitPush } = await import("../api/tasks");
    const pushArgs = {
      dest: wtPath,
      useStoredCredential: true,
    };
    console.log("[DEBUG] Calling startGitPush with args:", pushArgs);
    const taskId = await startGitPush(pushArgs);

    // 监听任务完成
    const unwatch = watch(
      () => tasksStore.items.find((t) => t.id === taskId)?.state,
      (state) => {
        if (state === "completed" || state === "failed") {
          pushingWorktreePaths.delete(wtPath);
          if (state === "completed") toastStore.success(`工作区推送成功`);
          else {
            const err = tasksStore.lastErrorById[taskId];
            toastStore.error(`工作区推送失败: ${err?.message || "未知错误"}`);
          }
          unwatch();
        }
      }
    );

    toastStore.success("推送任务已启动");
  } catch (error: any) {
    pushingWorktreePaths.delete(wtPath);
    toastStore.error(`启动推送失败: ${error.message}`);
  }
}

// 刷新数据
async function refresh() {
  await projectStore.refresh();
}

// 页面加载
onMounted(async () => {
  await projectStore.loadAllData();
});
</script>

<template>
  <main class="page">
    <div class="flex items-center gap-4 h-14">
      <h2 class="m-0!">项目管理</h2>
      <button
        class="btn btn-sm btn-outline"
        :disabled="isLoading"
        @click="refresh"
      >
        <span
          v-if="isLoading"
          class="loading loading-spinner loading-xs"
        ></span>
        <span v-else>刷新</span>
      </button>
    </div>

    <div class="flex flex-1 w-full gap-4 not-prose h-full overflow-hidden">
      <!-- 左栏：远端仓库 -->
      <div class="w-1/2 flex flex-col gap-3 overflow-auto">
        <!-- 上游仓库卡片 -->
        <StatusCard
          title="上游仓库"
          badge="Upstream"
          badge-variant="primary"
          :loading="loadingState === 'loading-upstream'"
          :flex="true"
        >
          <template #icon>
            <BaseIcon icon="ph--package" size="md" />
          </template>
          <!-- 内容 -->
          <template v-if="upstreamRepo">
            <div class="flex items-start gap-3">
              <div class="avatar">
                <div
                  class="w-12 rounded-full ring-2 ring-primary ring-offset-base-100 ring-offset-1"
                >
                  <img
                    :src="upstreamRepo.owner.avatar_url"
                    :alt="upstreamRepo.owner.login"
                  />
                </div>
              </div>
              <div class="flex-1 min-w-0">
                <h3 class="text-base font-bold">
                  <a
                    :href="upstreamRepo.html_url"
                    target="_blank"
                    class="link link-hover"
                    >{{ upstreamRepo.full_name }}</a
                  >
                </h3>
                <p
                  v-if="upstreamRepo.description"
                  class="text-sm text-base-content/70 mt-1"
                >
                  {{ upstreamRepo.description }}
                </p>
              </div>
            </div>

            <div class="flex flex-wrap items-center gap-x-4 gap-y-1 text-sm">
              <span class="flex items-center gap-1"
                ><BaseIcon icon="lucide--star" size="sm" class="text-warning" />
                <strong>{{
                  formatNumber(upstreamRepo.stargazers_count)
                }}</strong></span
              >
              <span class="flex items-center gap-1"
                ><BaseIcon icon="lucide--git-fork" size="sm" /><strong>{{
                  formatNumber(upstreamRepo.forks_count)
                }}</strong></span
              >
              <span class="flex items-center gap-1"
                ><BaseIcon icon="lucide--eye" size="sm" /><strong>{{
                  formatNumber(upstreamRepo.watchers_count)
                }}</strong></span
              >
              <span class="flex items-center gap-1"
                ><BaseIcon icon="lucide--bug" size="sm" /><strong>{{
                  upstreamRepo.open_issues_count
                }}</strong></span
              >
              <span v-if="upstreamRepo.language" class="flex items-center gap-1"
                ><span class="w-2 h-2 rounded-full bg-primary"></span
                >{{ upstreamRepo.language }}</span
              >
              <span
                v-if="upstreamRepo.license"
                class="flex items-center gap-1 text-base-content/60"
                ><BaseIcon icon="lucide--scroll-text" size="sm" />
                {{ upstreamRepo.license.spdx_id }}</span
              >
            </div>

            <!-- 语言分布 -->
            <template v-if="Object.keys(languages).length">
              <LanguageBar :languages="languages" :show-legend="true" />
            </template>
            <template v-else>
              <div class="space-y-1.5">
                <div class="skeleton h-2 w-full rounded-full"></div>
                <div class="flex gap-2">
                  <div class="skeleton h-3 w-16"></div>
                  <div class="skeleton h-3 w-12"></div>
                  <div class="skeleton h-3 w-14"></div>
                </div>
              </div>
            </template>

            <!-- 贡献者 + 时间 + 版本 同一行 -->
            <div class="flex items-center justify-between">
              <!-- 贡献者区域 -->
              <template v-if="contributorAvatars.length">
                <div class="flex items-center gap-2">
                  <span class="text-xs text-base-content/50">贡献者</span>
                  <AvatarGroup :items="contributorAvatars" :max="5" size="sm" />
                </div>
              </template>
              <template v-else>
                <div class="flex items-center gap-2">
                  <span class="text-xs text-base-content/50">贡献者</span>
                  <div class="flex -space-x-2">
                    <div class="skeleton w-6 h-6 rounded-full"></div>
                    <div class="skeleton w-6 h-6 rounded-full"></div>
                    <div class="skeleton w-6 h-6 rounded-full"></div>
                    <div class="skeleton w-6 h-6 rounded-full"></div>
                  </div>
                </div>
              </template>

              <!-- 版本和时间区域 -->
              <div class="flex items-center gap-2 text-xs">
                <!-- 有 Release 时显示版本标签 -->
                <a
                  v-if="latestRelease"
                  :href="latestRelease.html_url"
                  target="_blank"
                  class="badge badge-success badge-xs gap-1"
                  ><BaseIcon icon="lucide--tag" size="xs" />
                  {{ latestRelease.tag_name }}</a
                >
                <!-- 正在加载时（上游仓库未加载）显示 skeleton -->
                <div
                  v-else-if="!upstreamRepo"
                  class="skeleton h-4 w-14 rounded"
                ></div>
                <!-- 加载完成但无 Release 时不显示任何内容 -->
                <span class="text-base-content/50">{{
                  relativeTime(upstreamRepo.pushed_at)
                }}</span>
              </div>
            </div>

            <!-- Topics标签 -->
            <div
              v-if="upstreamRepo.topics?.length"
              class="flex flex-wrap gap-1"
            >
              <span
                v-for="topic in upstreamRepo.topics"
                :key="topic"
                class="badge badge-outline badge-xs hover:badge-primary cursor-pointer"
                >{{ topic }}</span
              >
            </div>
          </template>
          <template v-else>
            <div class="flex gap-3">
              <div class="skeleton h-14 w-14 rounded-xl"></div>
              <div class="flex-1">
                <div class="skeleton h-5 w-3/4 mb-2"></div>
                <div class="skeleton h-3 w-full"></div>
              </div>
            </div>
          </template>
        </StatusCard>

        <!-- Fork 卡片 -->
        <StatusCard
          title="你的 Fork"
          :loading="loadingState === 'loading-fork'"
          variant="gradient"
        >
          <template #icon>
            <BaseIcon icon="ph--git-fork" size="md" />
          </template>
          <template #header-actions>
            <a
              v-if="hasFork && forkRepo"
              :href="forkRepo.html_url"
              target="_blank"
              class="btn btn-ghost btn-xs"
            >
              <BaseIcon icon="lucide--external-link" size="xs" /> 打开
            </a>
            <button
              v-if="!hasFork"
              class="btn btn-primary btn-sm"
              :disabled="loadingState === 'forking' || !authenticated"
              @click="handleFork"
            >
              <span
                v-if="loadingState === 'forking'"
                class="loading loading-spinner loading-xs"
              ></span>
              <span v-else>创建 Fork</span>
            </button>
          </template>

          <template v-if="hasFork && forkRepo">
            <!-- Fork信息和同步状态 -->
            <div class="flex items-center gap-3">
              <div class="avatar">
                <div class="w-8 rounded-full">
                  <img
                    :src="forkRepo.owner.avatar_url"
                    :alt="forkRepo.owner.login"
                  />
                </div>
              </div>
              <div class="flex-1 min-w-0">
                <a
                  :href="forkRepo.html_url"
                  target="_blank"
                  class="font-medium link link-hover text-sm"
                  >{{ forkRepo.full_name }}</a
                >
                <div class="flex items-center gap-2 mt-1">
                  <template v-if="forkRepo.syncStatus?.isSynced">
                    <span class="badge badge-success badge-sm gap-1"
                      ><BaseIcon icon="lucide--check" size="xs" /> 已同步</span
                    >
                  </template>
                  <template v-else>
                    <span
                      v-if="forkRepo.syncStatus?.aheadBy"
                      class="badge badge-info badge-sm gap-1"
                      ><BaseIcon icon="lucide--arrow-up" size="xs" />{{
                        forkRepo.syncStatus.aheadBy
                      }}
                      ahead</span
                    >
                    <span
                      v-if="forkRepo.syncStatus?.behindBy"
                      class="badge badge-warning badge-sm gap-1"
                      ><BaseIcon icon="lucide--arrow-down" size="xs" />{{
                        forkRepo.syncStatus.behindBy
                      }}
                      behind</span
                    >
                    <!-- 同步按钮 -->
                    <button
                      class="btn btn-warning btn-xs"
                      :disabled="loadingState === 'syncing-fork'"
                      @click="handleSyncFork"
                    >
                      <span
                        v-if="loadingState === 'syncing-fork'"
                        class="loading loading-spinner loading-xs"
                      ></span>
                      <span v-else>同步</span>
                    </button>
                    <!-- 强制同步按钮 -->
                    <button
                      class="btn btn-error btn-xs"
                      :disabled="loadingState === 'syncing-fork'"
                      @click="handleForceSyncFork"
                      title="丢弃fork变更，完全与上游同步"
                    >
                      强制
                    </button>
                  </template>
                </div>
              </div>
            </div>

            <!-- 分支列表 -->
            <div>
              <div class="text-xs text-base-content/60 mb-1">
                <template v-if="forkBranches.length">
                  分支 ({{ forkBranches.length }})
                </template>
                <template v-else> 分支 </template>
              </div>
              <template v-if="forkBranches.length">
                <div class="flex flex-wrap gap-1">
                  <span
                    v-for="branch in forkBranches.slice(0, 5)"
                    :key="branch.name"
                    class="badge badge-outline badge-xs"
                    :class="{
                      'badge-primary': branch.name === forkRepo.default_branch,
                    }"
                  >
                    <BaseIcon icon="lucide--git-branch" size="xs" />
                    {{ branch.name }}
                  </span>
                  <span
                    v-if="forkBranches.length > 5"
                    class="badge badge-ghost badge-xs"
                    >+{{ forkBranches.length - 5 }}</span
                  >
                </div>
              </template>
              <template v-else>
                <div class="flex flex-wrap gap-1">
                  <div class="skeleton h-4 w-16 rounded"></div>
                  <div class="skeleton h-4 w-20 rounded"></div>
                  <div class="skeleton h-4 w-14 rounded"></div>
                </div>
              </template>
            </div>

            <!-- 最近Commits -->
            <div>
              <div class="text-xs text-base-content/60 mb-1">最近提交</div>
              <template v-if="forkCommits.length">
                <div class="space-y-1">
                  <a
                    v-for="commit in forkCommits.slice(0, 3)"
                    :key="commit.sha"
                    :href="commit.html_url"
                    target="_blank"
                    class="flex items-center gap-2 text-xs hover:text-primary transition-colors"
                  >
                    <div v-if="commit.author" class="avatar">
                      <div class="w-4 rounded-full">
                        <img
                          :src="commit.author.avatar_url"
                          :alt="commit.author.login"
                        />
                      </div>
                    </div>
                    <span class="truncate flex-1">{{
                      commit.commit.message.split("\n")[0]
                    }}</span>
                    <span class="text-[10px] text-base-content/40 shrink-0">{{
                      commit.sha.slice(0, 7)
                    }}</span>
                  </a>
                </div>
              </template>
              <template v-else>
                <div class="space-y-1.5">
                  <div class="flex items-center gap-2">
                    <div class="skeleton w-4 h-4 rounded-full"></div>
                    <div class="skeleton h-3 flex-1 rounded"></div>
                    <div class="skeleton h-3 w-10 rounded"></div>
                  </div>
                  <div class="flex items-center gap-2">
                    <div class="skeleton w-4 h-4 rounded-full"></div>
                    <div class="skeleton h-3 flex-1 rounded"></div>
                    <div class="skeleton h-3 w-10 rounded"></div>
                  </div>
                  <div class="flex items-center gap-2">
                    <div class="skeleton w-4 h-4 rounded-full"></div>
                    <div class="skeleton h-3 flex-1 rounded"></div>
                    <div class="skeleton h-3 w-10 rounded"></div>
                  </div>
                </div>
              </template>
            </div>
          </template>
          <template v-else-if="!hasFork">
            <p class="text-sm text-base-content/60 mt-2">
              Fork 后可在自己的仓库中修改，然后通过 Pull Request 贡献代码
            </p>
          </template>
        </StatusCard>
      </div>

      <!-- 分隔线 -->
      <div class="divider divider-horizontal m-0 text-base-content/30">
        <BaseIcon icon="lucide--arrow-right" size="md" />
      </div>

      <!-- 右栏：本地仓库/工作区 -->
      <div class="w-1/2 flex flex-col gap-3 overflow-auto">
        <!-- 本地仓库卡片 -->
        <StatusCard
          title="本地仓库"
          badge="Local"
          badge-variant="accent"
          :loading="loadingState === 'loading-local'"
        >
          <template #icon>
            <BaseIcon icon="ph--hard-drives" size="md" />
          </template>
          <template v-if="localStatus?.exists">
            <!-- 本地仓库信息 -->
            <div class="flex items-center gap-3">
              <div
                class="w-10 h-10 rounded-lg bg-base-200 flex items-center justify-center text-base-content/70"
              >
                <BaseIcon icon="lucide--folder-open" size="md" />
              </div>
              <div class="flex-1 min-w-0">
                <div
                  class="font-medium text-sm truncate"
                  :title="localStatus.path || ''"
                >
                  {{ localStatus.path?.split(/[/\\]/).pop() || "repository" }}
                </div>
                <div class="flex items-center gap-2 mt-1">
                  <!-- 分支 -->
                  <span class="badge badge-outline badge-sm">
                    <BaseIcon icon="lucide--git-branch" size="xs" />
                    {{ localStatus.currentBranch || "main" }}
                  </span>
                  <!-- 状态 -->
                  <span
                    v-if="localStatus.workingTreeClean"
                    class="badge badge-success badge-sm gap-1"
                    ><BaseIcon icon="lucide--check" size="xs" /> 干净</span
                  >
                  <span v-else class="badge badge-warning badge-sm gap-1"
                    ><BaseIcon icon="lucide--alert-triangle" size="xs" />
                    有改动</span
                  >
                </div>
              </div>
            </div>

            <!-- 同步状态和操作按钮 -->
            <div class="flex items-center gap-2 mt-3 flex-wrap">
              <!-- 跟踪分支 -->
              <span
                v-if="localStatus.trackingBranch"
                class="text-xs text-base-content/60 mr-1 flex items-center gap-1"
                :title="'跟踪远端分支: ' + localStatus.trackingBranch"
              >
                <BaseIcon icon="lucide--link" size="xs" />
                {{ localStatus.trackingBranch }}
              </span>

              <!-- ahead/behind 状态 -->
              <SyncStatusBadge
                :ahead="localStatus.ahead"
                :behind="localStatus.behind"
                :tracking-branch="localStatus.trackingBranch"
              />

              <!-- 操作按钮 -->
              <div class="flex-1"></div>
              <button
                class="btn btn-xs btn-outline"
                :disabled="loadingState !== 'idle'"
                @click="handleFetch"
                title="从远程拉取更新"
              >
                <span
                  v-if="loadingState === 'fetching'"
                  class="loading loading-spinner loading-xs"
                ></span>
                <span v-else>同步</span>
              </button>
              <button
                class="btn btn-xs btn-primary"
                :disabled="loadingState !== 'idle' || localStatus.ahead === 0"
                @click="handlePush"
                title="推送本地提交到远程"
              >
                <span
                  v-if="loadingState === 'pushing'"
                  class="loading loading-spinner loading-xs"
                ></span>
                <span v-else>推送</span>
              </button>
            </div>
          </template>
          <template v-else>
            <!-- 未克隆时的 Hero 样式提示 -->
            <div class="hero bg-base-200 rounded-lg mt-3">
              <div class="hero-content text-center py-6">
                <div>
                  <p
                    class="text-base-content/60 mb-4 h-12 flex flex-col items-center justify-center"
                  >
                    <template
                      v-if="loadingState === 'cloning' && cloneProgressDetails"
                    >
                      <progress
                        class="progress progress-primary w-56"
                        :value="cloneProgressDetails.percent"
                        max="100"
                      ></progress>
                      <div class="text-xs mt-1 opacity-70">
                        {{ cloneProgressDetails.phase }} ({{
                          cloneProgressDetails.percent
                        }}%)
                      </div>
                    </template>
                    <template v-else>
                      尚未克隆到本地{{ !hasFork ? "，请先 Fork 仓库" : "" }}
                    </template>
                  </p>
                  <button
                    class="btn btn-primary"
                    :disabled="!hasFork || loadingState === 'cloning'"
                    @click="handleClone"
                  >
                    <span
                      v-if="loadingState === 'cloning'"
                      class="loading loading-spinner loading-sm"
                    ></span>
                    <span v-else
                      ><BaseIcon icon="lucide--download" size="sm" />
                      克隆仓库</span
                    >
                  </button>
                </div>
              </div>
            </div>
          </template>
        </StatusCard>

        <!-- 工作区卡片 -->
        <StatusCard title="工作区" :flex="true">
          <template #icon>
            <BaseIcon icon="ph--tree-structure" size="md" />
          </template>
          <template #header-actions>
            <span class="badge badge-ghost badge-xs"
              >{{
                localStatus?.worktrees?.filter((w) => !w.isMainWorktree)
                  ?.length || 0
              }}
              个</span
            >
            <button
              class="btn btn-primary btn-xs"
              :disabled="!localStatus?.exists"
              @click="showWorktreeForm = !showWorktreeForm"
            >
              <template v-if="showWorktreeForm">取消</template>
              <template v-else
                ><BaseIcon icon="lucide--plus" size="xs" /> 新建</template
              >
            </button>
          </template>

          <!-- 创建工作区表单 -->
          <form
            v-if="showWorktreeForm"
            class="space-y-3 mt-2 p-4 bg-base-200 rounded-xl"
            @submit.prevent="handleCreateWorktree"
          >
            <!-- 模式切换 -->
            <div class="flex gap-2">
              <button
                type="button"
                class="btn btn-sm flex-1"
                :class="
                  worktreeCreateMode === 'new' ? 'btn-primary' : 'btn-ghost'
                "
                @click="worktreeCreateMode = 'new'"
              >
                <BaseIcon icon="lucide--file-plus" size="sm" /> 新建分支
              </button>
              <button
                type="button"
                class="btn btn-sm flex-1"
                :class="
                  worktreeCreateMode === 'remote' ? 'btn-primary' : 'btn-ghost'
                "
                @click="
                  worktreeCreateMode = 'remote';
                  loadRemoteBranches();
                "
              >
                <BaseIcon icon="lucide--refresh-cw" size="sm" /> 从远端拉取
              </button>
            </div>

            <!-- 新建模式：输入分支名 -->
            <div v-if="worktreeCreateMode === 'new'" class="form-control">
              <label class="label py-1">
                <span class="label-text text-xs font-medium">分支名称</span>
              </label>
              <input
                v-model="worktreeForm.branch"
                type="text"
                class="input input-bordered input-sm"
                placeholder="feature/my-feature"
              />
            </div>

            <!-- 远端模式：选择远端分支 -->
            <div v-else class="form-control">
              <label class="label py-1">
                <span class="label-text text-xs font-medium">选择远端分支</span>
              </label>
              <div
                v-if="loadingRemoteBranches"
                class="flex items-center gap-2 text-sm text-base-content/60"
              >
                <span class="loading loading-spinner loading-xs"></span>
                正在加载远端分支...
              </div>
              <select
                v-else
                v-model="worktreeForm.selectedRemoteBranch"
                class="select select-bordered select-sm"
              >
                <option value="" disabled>请选择远端分支</option>
                <option
                  v-for="branch in remoteBranches.filter(
                    (b) => !b.includes('/main') && !b.includes('/master')
                  )"
                  :key="branch"
                  :value="branch"
                >
                  {{ branch }}
                </option>
              </select>
              <label
                v-if="worktreeForm.selectedRemoteBranch"
                class="label py-0"
              >
                <span class="label-text-alt text-xs text-success">
                  将创建本地分支：{{
                    extractBranchName(worktreeForm.selectedRemoteBranch)
                  }}
                </span>
              </label>
            </div>

            <div class="flex justify-end">
              <button
                type="submit"
                class="btn btn-primary btn-sm"
                :disabled="
                  worktreeCreateMode === 'new'
                    ? !worktreeForm.branch
                    : !worktreeForm.selectedRemoteBranch
                "
              >
                创建工作区
              </button>
            </div>
          </form>

          <!-- 工作区列表（只显示非主分支） -->
          <div
            v-if="
              localStatus?.worktrees?.filter((w) => !w.isMainWorktree).length
            "
            class="space-y-1.5 mt-2"
          >
            <div
              v-for="wt in localStatus.worktrees.filter(
                (w) => !w.isMainWorktree
              )"
              :key="wt.path"
              class="group flex flex-col gap-1.5 px-3 py-2.5 rounded-xl border border-base-content/10 bg-base-200/30 hover:border-primary/50 transition-all"
            >
              <!-- 第一行：分支 & PR & 操作 -->
              <div class="flex items-center justify-between w-full">
                <div class="flex items-center gap-2 min-w-0">
                  <!-- 分支名称 (普通标题样式) -->
                  <div class="flex items-center gap-2">
                    <span class="font-bold text-sm select-all">{{
                      wt.branch
                    }}</span>
                  </div>

                  <!-- PR状态 -->
                  <a
                    v-if="wt.linkedPR"
                    :href="wt.linkedPRUrl || '#'"
                    target="_blank"
                    class="badge badge-success badge-xs gap-1 hover:badge-outline h-5 font-normal text-white"
                    title="已关联PR"
                  >
                    <BaseIcon icon="lucide--tag" size="xs" />
                    #{{ wt.linkedPR }}
                  </a>
                  <span
                    v-else
                    class="badge badge-ghost badge-xs h-5 font-normal text-base-content/60"
                  >
                    无PR
                  </span>

                  <!-- 路径 (移到这里) -->
                  <span
                    class="text-xs text-base-content/40 font-mono truncate max-w-[150px] ml-1"
                    :title="wt.path"
                  >
                    {{ wt.path.split(/[/\\]/).slice(-2).join("/") }}
                  </span>
                </div>

                <!-- 操作按钮组 (右侧仅保留按钮) -->
                <div class="flex items-center gap-2 ml-auto">
                  <div
                    class="flex items-center gap-1 opacity-0 group-hover:opacity-100 transition-opacity"
                  >
                    <button
                      class="btn btn-ghost btn-xs btn-square text-primary"
                      title="推送变更"
                      @click="handlePushWorktree(wt.path)"
                      :disabled="pushingWorktreePaths.has(wt.path)"
                    >
                      <span
                        v-if="pushingWorktreePaths.has(wt.path)"
                        class="loading loading-spinner loading-xs"
                      ></span>
                      <BaseIcon v-else icon="lucide--arrow-up" size="sm" />
                    </button>

                    <button
                      class="btn btn-ghost btn-xs btn-square text-error"
                      title="删除工作区"
                      @click="showDeleteConfirm(wt.path)"
                    >
                      <BaseIcon icon="lucide--trash-2" size="sm" />
                    </button>
                  </div>
                </div>
              </div>

              <!-- 第二行：状态详情 (Tracking & Status badges) -->
              <div class="flex items-center gap-2 text-xs w-full">
                <!-- 跟踪分支 (Badge style) -->
                <span
                  v-if="wt.trackingBranch"
                  class="badge badge-ghost badge-xs gap-1.5 min-h-[20px] h-auto border-base-content/20"
                  :title="'跟踪远端: ' + wt.trackingBranch"
                >
                  <BaseIcon icon="lucide--link" size="xs" />
                  <span class="font-mono">{{ wt.trackingBranch }}</span>
                </span>

                <!-- 状态徽章 -->
                <SyncStatusBadge
                  :ahead="wt.ahead"
                  :behind="wt.behind"
                  :tracking-branch="wt.trackingBranch"
                />
              </div>
            </div>
          </div>

          <!-- 空状态 -->
          <EmptyState
            v-else-if="localStatus?.exists && !isLoading"
            title="暂无分支工作区"
            description="点击 '+ 新建' 创建分支工作区"
          >
            <template #icon>
              <BaseIcon icon="ph--folder-notch-open" size="2xl" />
            </template>
          </EmptyState>

          <!-- 未克隆提示 -->
          <EmptyState
            v-else-if="!localStatus?.exists && !isLoading"
            title="请先克隆仓库"
          >
            <template #icon>
              <BaseIcon icon="ph--mailbox" size="2xl" />
            </template>
          </EmptyState>
        </StatusCard>
      </div>
    </div>
  </main>

  <!-- 删除确认对话框 -->
  <ConfirmModal
    v-model="showDeleteModal"
    title="确认删除"
    confirm-text="删除"
    confirm-variant="error"
    @confirm="confirmDeleteWorktree"
  >
    <p class="py-2">
      确定要删除这个工作区吗？<br />
      <code class="text-sm text-error break-all">{{ pendingDeletePath }}</code>
    </p>

    <!-- 删除远端分支选项 -->
    <div class="form-control mb-4">
      <label class="label cursor-pointer justify-start gap-3">
        <input
          v-model="deleteRemoteBranch"
          type="checkbox"
          class="checkbox checkbox-warning"
        />
        <span class="label-text">同时删除远端分支</span>
      </label>
      <p v-if="deleteRemoteBranch" class="text-xs text-warning ml-9">
        将执行 git push origin --delete &lt;branch&gt;
      </p>
    </div>

    <p class="text-warning text-sm">此操作不可撤销！</p>
  </ConfirmModal>
</template>

<style scoped>
.vertical-lr {
  writing-mode: vertical-lr;
}
</style>
