<script setup lang="ts">
import { onMounted, ref, computed, inject, Ref, watch } from "vue";
import { storeToRefs } from "pinia";
import { useProjectStore } from "../stores/project";
import { useToastStore } from "../stores/toast";

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
const languagePercentages = computed(() => projectStore.languagePercentages);

// 工作区创建表单
const showWorktreeForm = ref(false);
const worktreeForm = ref({
  branch: "",
});

// 创建工作区
async function handleCreateWorktree() {
  if (!worktreeForm.value.branch) {
    toastStore.error("请输入分支名称");
    return;
  }

  try {
    // TODO: 调用后端创建工作区API
    toastStore.success(`工作区 ${worktreeForm.value.branch} 创建中...`);
    showWorktreeForm.value = false;
    worktreeForm.value = { branch: "" };

    // 刷新本地仓库状态
    await projectStore.checkLocalRepo();
  } catch (error: any) {
    console.error("创建工作区失败:", error);
    toastStore.error(`创建工作区失败: ${error.message || error}`);
  }
}

// 删除工作区
async function handleDeleteWorktree(path: string) {
  if (!confirm(`确定删除工作区？\n${path}`)) {
    return;
  }

  try {
    // TODO: 调用后端删除工作区API
    toastStore.success("工作区已删除");
    await projectStore.checkLocalRepo();
  } catch (error: any) {
    console.error("删除工作区失败:", error);
    toastStore.error(`删除工作区失败: ${error.message || error}`);
  }
}

// 格式化数字
function formatNumber(num: number): string {
  if (num >= 1000) {
    return (num / 1000).toFixed(1) + "k";
  }
  return num.toString();
}

// 相对时间
function relativeTime(dateStr: string): string {
  const date = new Date(dateStr);
  const now = new Date();
  const diff = now.getTime() - date.getTime();
  const days = Math.floor(diff / (1000 * 60 * 60 * 24));

  if (days === 0) return "今天";
  if (days === 1) return "昨天";
  if (days < 7) return `${days} 天前`;
  if (days < 30) return `${Math.floor(days / 7)} 周前`;
  if (days < 365) return `${Math.floor(days / 30)} 个月前`;
  return `${Math.floor(days / 365)} 年前`;
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
async function handleClone() {
  if (!projectStore.hasFork || !projectStore.forkRepo) {
    toastStore.error("请先Fork仓库");
    return;
  }

  try {
    projectStore.loadingState = "cloning";
    const clonePath = await projectStore.getClonePath();
    const cloneUrl =
      projectStore.forkRepo.clone_url ||
      projectStore.forkRepo.html_url + ".git";

    // 调用后端Git clone命令
    const { startGitClone } = await import("../api/tasks");
    await startGitClone(cloneUrl, clonePath);

    toastStore.success("克隆任务已启动");

    // 延迟后刷新本地仓库状态
    setTimeout(async () => {
      await projectStore.checkLocalRepo();
    }, 2000);
  } catch (error: any) {
    console.error("克隆失败:", error);
    toastStore.error(`克隆失败: ${error.message || error}`);
  } finally {
    projectStore.loadingState = "idle";
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
        <div class="card border-2 border-base-content/15 bg-base-100 flex-1">
          <div class="card-body p-4 flex-1 gap-3">
            <!-- 卡片头部 -->
            <div class="flex items-center justify-between">
              <h4 class="font-semibold text-sm flex items-center gap-2">
                📦 上游仓库
                <span
                  v-if="loadingState === 'loading-upstream'"
                  class="loading loading-spinner loading-xs"
                ></span>
              </h4>
              <span class="badge badge-primary badge-sm">Upstream</span>
            </div>

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

              <!-- 统计信息 - 更紧凑 -->
              <div class="flex flex-wrap items-center gap-x-4 gap-y-1 text-sm">
                <span class="flex items-center gap-1"
                  ><span class="text-warning">⭐</span
                  ><strong>{{
                    formatNumber(upstreamRepo.stargazers_count)
                  }}</strong></span
                >
                <span class="flex items-center gap-1"
                  >🔀<strong>{{
                    formatNumber(upstreamRepo.forks_count)
                  }}</strong></span
                >
                <span class="flex items-center gap-1"
                  >👁️<strong>{{
                    formatNumber(upstreamRepo.watchers_count)
                  }}</strong></span
                >
                <span class="flex items-center gap-1"
                  >🐛<strong>{{ upstreamRepo.open_issues_count }}</strong></span
                >
                <span
                  v-if="upstreamRepo.language"
                  class="flex items-center gap-1"
                  ><span class="w-2 h-2 rounded-full bg-primary"></span
                  >{{ upstreamRepo.language }}</span
                >
                <span v-if="upstreamRepo.license" class="text-base-content/60"
                  >📜 {{ upstreamRepo.license.spdx_id }}</span
                >
              </div>

              <!-- 语言分布 -->
              <div v-if="Object.keys(languages).length">
                <div class="flex h-2 rounded-full overflow-hidden bg-base-300">
                  <div
                    v-for="(percent, lang) in languagePercentages"
                    :key="lang"
                    :style="{ width: `${percent}%` }"
                    class="h-full"
                    :title="`${lang}: ${percent}%`"
                    :class="{
                      'bg-blue-500': lang === 'TypeScript',
                      'bg-yellow-400': lang === 'JavaScript',
                      'bg-purple-500': lang === 'Vue',
                      'bg-orange-500': lang === 'Rust',
                      'bg-emerald-500': lang === 'CSS',
                      'bg-primary': ![
                        'TypeScript',
                        'JavaScript',
                        'Vue',
                        'Rust',
                        'CSS',
                      ].includes(lang as string),
                    }"
                  ></div>
                </div>
                <div
                  class="flex flex-wrap gap-2 mt-1 text-[10px] text-base-content/60"
                >
                  <span
                    v-for="(percent, lang) in languagePercentages"
                    :key="lang"
                    class="flex items-center gap-1"
                  >
                    <span
                      class="w-1.5 h-1.5 rounded-full"
                      :class="{
                        'bg-blue-500': lang === 'TypeScript',
                        'bg-yellow-400': lang === 'JavaScript',
                        'bg-purple-500': lang === 'Vue',
                        'bg-orange-500': lang === 'Rust',
                        'bg-emerald-500': lang === 'CSS',
                        'bg-primary': ![
                          'TypeScript',
                          'JavaScript',
                          'Vue',
                          'Rust',
                          'CSS',
                        ].includes(lang as string),
                      }"
                    ></span>
                    {{ lang }} {{ percent }}%
                  </span>
                </div>
              </div>

              <!-- 贡献者 + 时间 + 版本 同一行 -->
              <div class="flex items-center justify-between">
                <div v-if="contributors.length" class="flex items-center gap-2">
                  <span class="text-xs text-base-content/50">贡献者</span>
                  <div class="avatar-group -space-x-3">
                    <div
                      v-for="contrib in contributors.slice(0, 5)"
                      :key="contrib.login"
                      class="avatar"
                    >
                      <a
                        :href="contrib.html_url"
                        target="_blank"
                        :title="contrib.login"
                        class="w-6 rounded-full ring ring-base-100 hover:ring-primary hover:z-10"
                      >
                        <img :src="contrib.avatar_url" :alt="contrib.login" />
                      </a>
                    </div>
                    <div
                      v-if="contributors.length > 5"
                      class="avatar placeholder"
                    >
                      <div
                        class="bg-neutral text-neutral-content w-6 rounded-full text-[9px]"
                      >
                        +{{ contributors.length - 5 }}
                      </div>
                    </div>
                  </div>
                </div>
                <div class="flex items-center gap-2 text-xs">
                  <a
                    v-if="latestRelease"
                    :href="latestRelease.html_url"
                    target="_blank"
                    class="badge badge-success badge-xs"
                    >🏷️ {{ latestRelease.tag_name }}</a
                  >
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
          </div>
        </div>

        <!-- Fork 卡片 -->
        <div
          class="card border-2 border-base-content/15 bg-gradient-to-r from-secondary/5 to-primary/5"
        >
          <div class="card-body p-4">
            <div class="flex items-center justify-between">
              <h4 class="font-semibold text-sm flex items-center gap-2">
                🍴 你的 Fork
                <span
                  v-if="loadingState === 'loading-fork'"
                  class="loading loading-spinner loading-xs"
                ></span>
              </h4>
              <div class="flex items-center gap-2">
                <a
                  v-if="hasFork && forkRepo"
                  :href="forkRepo.html_url"
                  target="_blank"
                  class="btn btn-ghost btn-xs"
                >
                  打开 ↗
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
              </div>
            </div>

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
                      <span class="badge badge-success badge-sm">✓ 已同步</span>
                    </template>
                    <template v-else>
                      <span
                        v-if="forkRepo.syncStatus?.aheadBy"
                        class="badge badge-info badge-sm"
                        >↑{{ forkRepo.syncStatus.aheadBy }} ahead</span
                      >
                      <span
                        v-if="forkRepo.syncStatus?.behindBy"
                        class="badge badge-warning badge-sm"
                        >↓{{ forkRepo.syncStatus.behindBy }} behind</span
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
              <div v-if="forkBranches.length">
                <div class="text-xs text-base-content/60 mb-1">
                  分支 ({{ forkBranches.length }})
                </div>
                <div class="flex flex-wrap gap-1">
                  <span
                    v-for="branch in forkBranches.slice(0, 5)"
                    :key="branch.name"
                    class="badge badge-outline badge-xs"
                    :class="{
                      'badge-primary': branch.name === forkRepo.default_branch,
                    }"
                  >
                    🌿 {{ branch.name }}
                  </span>
                  <span
                    v-if="forkBranches.length > 5"
                    class="badge badge-ghost badge-xs"
                    >+{{ forkBranches.length - 5 }}</span
                  >
                </div>
              </div>

              <!-- 最近Commits -->
              <div v-if="forkCommits.length">
                <div class="text-xs text-base-content/60 mb-1">最近提交</div>
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
              </div>
            </template>
            <template v-else-if="!hasFork">
              <p class="text-sm text-base-content/60 mt-2">
                Fork 后可在自己的仓库中修改，然后通过 Pull Request 贡献代码
              </p>
            </template>
          </div>
        </div>
      </div>

      <!-- 分隔线 -->
      <div class="divider divider-horizontal m-0 text-base-content/30">→</div>

      <!-- 右栏：本地仓库/工作区 -->
      <div class="w-1/2 flex flex-col gap-3 overflow-auto">
        <!-- 本地仓库卡片 -->
        <div class="card border-2 border-base-content/15 bg-base-100">
          <div class="card-body p-4 gap-3">
            <div class="flex items-center justify-between">
              <h4 class="font-semibold text-sm flex items-center gap-2">
                📂 本地仓库
                <span
                  v-if="loadingState === 'loading-local'"
                  class="loading loading-spinner loading-xs"
                ></span>
              </h4>
              <span class="badge badge-accent badge-sm">Local</span>
            </div>

            <template v-if="localStatus?.exists">
              <!-- 本地仓库信息 -->
              <div class="flex items-center gap-3">
                <div class="avatar placeholder">
                  <div class="w-8 rounded-full bg-accent/20 text-accent">
                    <span class="text-lg">📁</span>
                  </div>
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
                      🌿 {{ localStatus.currentBranch || "main" }}
                    </span>
                    <!-- 状态 -->
                    <span
                      v-if="localStatus.workingTreeClean"
                      class="badge badge-success badge-sm"
                      >✓ 干净</span
                    >
                    <span v-else class="badge badge-warning badge-sm"
                      >⚠ 有改动</span
                    >
                  </div>
                </div>
              </div>
            </template>
            <template v-else>
              <!-- 未克隆时的 Hero 样式提示 -->
              <div class="hero bg-base-200 rounded-lg mt-3">
                <div class="hero-content text-center py-6">
                  <div>
                    <p class="text-base-content/60 mb-4">
                      尚未克隆到本地{{ !hasFork ? "，请先 Fork 仓库" : "" }}
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
                      <span v-else>📥 克隆仓库</span>
                    </button>
                  </div>
                </div>
              </div>
            </template>
          </div>
        </div>

        <!-- 工作区卡片 -->
        <div
          class="card border-2 border-base-content/15 bg-base-100 flex-1 overflow-y-auto"
        >
          <div class="card-body p-4 gap-3">
            <div class="flex items-center justify-between">
              <h4 class="font-semibold text-sm flex items-center gap-2">
                📂 工作区
                <span class="badge badge-ghost badge-xs"
                  >{{ localStatus?.worktrees?.length || 0 }} 个</span
                >
              </h4>
              <button
                class="btn btn-primary btn-sm"
                :disabled="!localStatus?.exists"
                @click="showWorktreeForm = !showWorktreeForm"
              >
                {{ showWorktreeForm ? "取消" : "+ 新建" }}
              </button>
            </div>

            <!-- 创建工作区表单 -->
            <form
              v-if="showWorktreeForm"
              class="space-y-3 mt-2 p-4 bg-base-200 rounded-xl"
              @submit.prevent="handleCreateWorktree"
            >
              <div class="form-control">
                <label class="label py-1"
                  ><span class="label-text text-xs font-medium"
                    >分支名称</span
                  ></label
                >
                <input
                  v-model="worktreeForm.branch"
                  type="text"
                  class="input input-bordered input-sm"
                  placeholder="feature/my-feature"
                  required
                />
              </div>
              <div class="flex justify-end">
                <button
                  type="submit"
                  class="btn btn-primary btn-sm"
                  :disabled="!worktreeForm.branch"
                >
                  创建工作区
                </button>
              </div>
            </form>

            <!-- 工作区列表 -->
            <div v-if="localStatus?.worktrees?.length" class="space-y-2 mt-2">
              <div
                v-for="wt in localStatus.worktrees"
                :key="wt.path"
                class="flex items-center gap-3 p-2 rounded-lg bg-base-200/50 hover:bg-base-200 transition-colors"
              >
                <div class="avatar placeholder">
                  <div
                    class="w-6 rounded-full"
                    :class="
                      wt.isMainWorktree
                        ? 'bg-primary/20 text-primary'
                        : 'bg-secondary/20 text-secondary'
                    "
                  >
                    <span class="text-xs">{{
                      wt.isMainWorktree ? "M" : "W"
                    }}</span>
                  </div>
                </div>
                <div class="flex-1 min-w-0">
                  <div class="flex items-center gap-2">
                    <span class="text-sm font-medium truncate">{{
                      wt.branch
                    }}</span>
                    <span
                      v-if="wt.isMainWorktree"
                      class="badge badge-primary badge-xs"
                      >主</span
                    >
                    <span
                      v-if="wt.linkedPR"
                      class="badge badge-success badge-xs"
                      >#{{ wt.linkedPR }}</span
                    >
                  </div>
                  <div
                    class="text-xs text-base-content/50 truncate"
                    :title="wt.path"
                  >
                    {{ wt.path.split(/[/\\]/).slice(-2).join("/") }}
                  </div>
                </div>
                <div class="flex items-center gap-1">
                  <a
                    v-if="wt.linkedPRUrl"
                    :href="wt.linkedPRUrl"
                    target="_blank"
                    class="btn btn-ghost btn-xs"
                    title="查看PR"
                    >🔗</a
                  >
                  <button
                    v-if="!wt.isMainWorktree"
                    class="btn btn-ghost btn-xs text-error"
                    title="删除工作区"
                    @click="handleDeleteWorktree(wt.path)"
                  >
                    ✕
                  </button>
                </div>
              </div>
            </div>

            <!-- 空状态 -->
            <div
              v-else-if="localStatus?.exists && !isLoading"
              class="text-center py-6 text-base-content/50"
            >
              <div class="text-3xl mb-2">📁</div>
              <p class="text-sm">暂无分支工作区</p>
              <p class="text-xs mt-1">点击"+ 新建"创建分支工作区</p>
            </div>

            <!-- 未克隆提示 -->
            <div
              v-else-if="!localStatus?.exists && !isLoading"
              class="text-center py-6 text-base-content/50"
            >
              <div class="text-3xl mb-2">📭</div>
              <p class="text-sm">请先克隆仓库</p>
            </div>
          </div>
        </div>
      </div>
    </div>
  </main>
</template>

<style scoped>
.vertical-lr {
  writing-mode: vertical-lr;
}
</style>
