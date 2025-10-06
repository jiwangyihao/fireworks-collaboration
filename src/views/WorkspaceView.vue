<template>
  <div class="p-4 pt-16 space-y-6">
    <div class="flex justify-between items-center">
      <div>
        <h1 class="text-2xl font-bold">工作区管理</h1>
        <p class="text-sm text-base-content/60" v-if="hasWorkspace">
          根目录：{{ current?.rootPath }} · 最近更新：{{ current?.updatedAt }}
        </p>
        <p class="text-sm text-base-content/60" v-else>未加载工作区，请创建或加载配置。</p>
      </div>
      <div class="flex gap-2">
        <button class="btn btn-sm" @click="toggleCreateForm">
          {{ showCreateForm ? '取消创建' : hasWorkspace ? '创建新工作区' : '创建工作区' }}
        </button>
        <button class="btn btn-sm btn-outline" :disabled="loadingWorkspace" @click="handleRefreshWorkspace">
          <span v-if="loadingWorkspace" class="loading loading-spinner loading-xs"></span>
          <span v-else>刷新</span>
        </button>
        <button class="btn btn-sm btn-outline" :disabled="loadingWorkspace" @click="handleCloseWorkspace">
          关闭工作区
        </button>
      </div>
    </div>

    <div v-if="lastError" class="alert alert-error shadow-sm">
      <span>错误：{{ lastError }}</span>
      <button class="btn btn-sm btn-ghost" @click="workspaceStore.setError(null)">忽略</button>
    </div>

    <div v-if="uiMessage" class="alert alert-success shadow-sm">
      <span>{{ uiMessage }}</span>
      <button class="btn btn-sm btn-ghost" @click="uiMessage = ''">关闭</button>
    </div>

    <!-- Create / Load Workspace -->
    <section class="card bg-base-100 shadow-sm" v-if="showCreateForm || !hasWorkspace">
      <div class="card-body space-y-4">
        <h2 class="card-title text-lg">{{ hasWorkspace ? '创建新工作区' : '初始化工作区' }}</h2>
        <form class="grid gap-4 md:grid-cols-2" @submit.prevent="handleCreateWorkspace">
          <label class="form-control">
            <span class="label-text">名称</span>
            <input v-model="createForm.name" type="text" class="input input-bordered input-sm" required />
          </label>
          <label class="form-control">
            <span class="label-text">根目录路径</span>
            <input v-model="createForm.rootPath" type="text" class="input input-bordered input-sm" required />
          </label>
          <label class="form-control md:col-span-2">
            <span class="label-text">元数据（可选，JSON 对象）</span>
            <textarea v-model="createForm.metadata" class="textarea textarea-bordered textarea-sm" rows="3" placeholder='{"owner": "team"}'></textarea>
          </label>
          <div class="flex gap-2 md:col-span-2">
            <button class="btn btn-sm btn-primary" type="submit" :disabled="loadingWorkspace">
              <span v-if="loadingWorkspace" class="loading loading-spinner loading-xs"></span>
              <span v-else>创建并加载</span>
            </button>
            <label class="form-control">
              <span class="label-text">或加载现有工作区文件</span>
              <div class="flex gap-2">
                <input v-model="loadPath" type="text" class="input input-bordered input-sm flex-1" placeholder="workspace.json" />
                <button class="btn btn-sm" type="button" :disabled="!loadPath" @click="handleLoadWorkspace">加载</button>
              </div>
            </label>
          </div>
        </form>
      </div>
    </section>

    <section v-if="hasWorkspace" class="card bg-base-100 shadow-sm">
      <div class="card-body space-y-4">
        <div class="flex flex-wrap items-center gap-2 justify-between">
          <h2 class="card-title text-lg">仓库列表</h2>
          <div class="flex gap-2">
            <label class="form-control">
              <span class="label-text">保存工作区至</span>
              <div class="flex gap-2">
                <input v-model="savePath" type="text" class="input input-bordered input-sm flex-1" placeholder="workspace.json" />
                <button class="btn btn-sm" type="button" :disabled="!savePath || loadingWorkspace" @click="handleSaveWorkspace">
                  <span v-if="loadingWorkspace" class="loading loading-spinner loading-xs"></span>
                  <span v-else>保存</span>
                </button>
              </div>
            </label>
            <button class="btn btn-sm btn-outline" type="button" @click="workspaceStore.refreshRepositories">刷新列表</button>
          </div>
        </div>

        <form class="grid gap-4 md:grid-cols-4" @submit.prevent="handleAddRepository">
          <label class="form-control">
            <span class="label-text">仓库 ID</span>
            <input v-model="newRepoForm.id" type="text" class="input input-sm input-bordered" required />
          </label>
          <label class="form-control">
            <span class="label-text">名称</span>
            <input v-model="newRepoForm.name" type="text" class="input input-sm input-bordered" required />
          </label>
          <label class="form-control">
            <span class="label-text">相对路径</span>
            <input v-model="newRepoForm.path" type="text" class="input input-sm input-bordered" required />
          </label>
          <label class="form-control">
            <span class="label-text">远程 URL</span>
            <input v-model="newRepoForm.remoteUrl" type="text" class="input input-sm input-bordered" required />
          </label>
          <label class="form-control md:col-span-4">
            <span class="label-text">标签（逗号分隔，可选）</span>
            <input v-model="newRepoForm.tags" type="text" class="input input-sm input-bordered" placeholder="frontend,critical" />
          </label>
          <div class="md:col-span-4 flex gap-2">
            <button class="btn btn-primary btn-sm" type="submit">添加仓库</button>
            <button class="btn btn-ghost btn-sm" type="button" @click="resetNewRepoForm">重置</button>
          </div>
        </form>

        <div class="overflow-x-auto">
          <table class="table table-sm">
            <thead>
              <tr>
                <th class="w-12 text-center">
                  <input
                    type="checkbox"
                    class="checkbox checkbox-xs"
                    :checked="allSelected"
                    :indeterminate="someSelected"
                    @change="workspaceStore.selectAll(!allSelected)"
                  />
                </th>
                <th class="w-10"></th>
                <th>名称</th>
                <th>路径</th>
                <th>远程</th>
                <th>标签</th>
                <th class="text-center">启用</th>
                <th class="text-right">操作</th>
              </tr>
            </thead>
            <tbody>
              <tr
                v-for="repo in repositories"
                :key="repo.id"
                draggable="true"
                @dragstart="onDragStart(repo.id)"
                @dragover.prevent
                @drop.prevent="onDrop(repo.id)"
                :class="draggingRepoId === repo.id ? 'bg-base-200' : ''"
              >
                <td class="text-center">
                  <input type="checkbox" class="checkbox checkbox-xs" :checked="selectedRepoIds.includes(repo.id)" @change="workspaceStore.toggleRepositorySelection(repo.id)" />
                </td>
                <td>
                  <span class="cursor-grab" title="拖拽调整顺序">☰</span>
                </td>
                <td class="font-semibold">{{ repo.name }}</td>
                <td><code class="text-xs">{{ repo.path }}</code></td>
                <td>
                  <a v-if="repo.remoteUrl" :href="repo.remoteUrl" target="_blank" class="link link-hover text-xs">{{ repo.remoteUrl }}</a>
                  <span v-else class="badge badge-warning badge-xs">未配置</span>
                </td>
                <td>
                  <div class="flex flex-wrap gap-1">
                    <span v-for="tag in repo.tags" :key="tag" class="badge badge-outline badge-xs">{{ tag }}</span>
                  </div>
                </td>
                <td class="text-center">
                  <input type="checkbox" class="toggle toggle-xs" :checked="repo.enabled" @change="workspaceStore.toggleRepositoryEnabled(repo.id)" />
                </td>
                <td class="text-right">
                  <div class="flex justify-end gap-2">
                    <button class="btn btn-ghost btn-xs" type="button" @click="editTags(repo)">标签</button>
                    <button class="btn btn-ghost btn-xs" type="button" @click="workspaceStore.removeRepository(repo.id)">删除</button>
                  </div>
                </td>
              </tr>
              <tr v-if="repositories.length === 0">
                <td colspan="8" class="text-center text-sm text-base-content/60">暂无仓库，先通过上方表单添加。</td>
              </tr>
            </tbody>
          </table>
        </div>
      </div>
    </section>

    <!-- Status Section -->
    <section v-if="hasWorkspace" class="card bg-base-100 shadow-sm">
      <div class="card-body space-y-4">
        <div class="flex flex-wrap items-center gap-2 justify-between">
          <h2 class="card-title text-lg">跨仓库状态</h2>
          <div class="flex gap-2">
            <button class="btn btn-sm btn-outline" :disabled="loadingStatus" @click="applyFilters(true)">
              <span v-if="loadingStatus" class="loading loading-spinner loading-xs"></span>
              <span v-else>刷新</span>
            </button>
            <button class="btn btn-sm" @click="workspaceStore.clearStatusCache">清空缓存并刷新</button>
          </div>
        </div>

        <form class="grid gap-4 md:grid-cols-5" @submit.prevent="applyFilters(false)">
          <label class="form-control">
            <span class="label-text">仓库名称</span>
            <input v-model="filterName" type="text" class="input input-bordered input-sm" placeholder="模糊匹配" />
          </label>
          <label class="form-control">
            <span class="label-text">分支关键字</span>
            <input v-model="filterBranch" type="text" class="input input-bordered input-sm" placeholder="main" />
          </label>
          <label class="form-control">
            <span class="label-text">同步状态</span>
            <select v-model="syncStateSelection" multiple class="select select-bordered select-sm h-24">
              <option value="clean">已同步</option>
              <option value="ahead">超前</option>
              <option value="behind">落后</option>
              <option value="diverged">分叉</option>
              <option value="detached">游离</option>
              <option value="unknown">未知</option>
            </select>
          </label>
          <label class="form-control">
            <span class="label-text">本地变更</span>
            <select v-model="dirtyFilter" class="select select-bordered select-sm">
              <option value="all">全部</option>
              <option value="dirty">仅有改动</option>
              <option value="clean">仅干净</option>
            </select>
          </label>
          <label class="form-control">
            <span class="label-text">选项</span>
            <div class="flex flex-col gap-2">
              <label class="label cursor-pointer justify-start gap-2">
                <input type="checkbox" class="checkbox checkbox-xs" v-model="includeDisabled" />
                <span class="label-text">包含已禁用仓库</span>
              </label>
              <span class="text-xs text-base-content/60" v-if="status?.autoRefreshSecs">自动刷新：每 {{ status?.autoRefreshSecs }} 秒</span>
            </div>
          </label>
          <div class="md:col-span-5 flex gap-2">
            <button class="btn btn-primary btn-sm" type="submit">应用筛选</button>
            <button class="btn btn-ghost btn-sm" type="button" @click="resetFilters">重置</button>
          </div>
        </form>

        <div v-if="status" class="grid gap-4 lg:grid-cols-4 md:grid-cols-2">
          <div class="stat">
            <div class="stat-title">仓库总数</div>
            <div class="stat-value text-primary">{{ status.total }}</div>
            <div class="stat-desc">刷新 {{ status.refreshed }} · 缓存 {{ status.cached }}</div>
          </div>
          <div class="stat">
            <div class="stat-title">工作树</div>
            <div class="stat-value">{{ status.summary.workingStates.clean }} / {{ status.summary.workingStates.dirty }}</div>
            <div class="stat-desc">干净 / 脏仓库</div>
          </div>
          <div class="stat">
            <div class="stat-title">同步状态</div>
            <div class="stat-value">{{ status.summary.syncStates.clean }}</div>
            <div class="stat-desc">领先 {{ status.summary.syncStates.ahead }} · 落后 {{ status.summary.syncStates.behind }}</div>
          </div>
          <div class="stat" v-if="status.summary.errorCount > 0">
            <div class="stat-title">异常仓库</div>
            <div class="stat-value text-error">{{ status.summary.errorCount }}</div>
            <div class="stat-desc text-xs break-words">{{ status.summary.errorRepositories?.join(', ') }}</div>
          </div>
        </div>

        <div class="overflow-x-auto" v-if="status">
          <table class="table table-xs">
            <thead>
              <tr>
                <th>仓库</th>
                <th>分支</th>
                <th>同步</th>
                <th>ahead/behind</th>
                <th>工作树</th>
                <th>未跟踪</th>
                <th>更新时间</th>
                <th>错误</th>
              </tr>
            </thead>
            <tbody>
              <tr v-for="item in status.statuses" :key="item.repoId">
                <td>
                  <div class="flex items-center gap-2">
                    <span class="font-semibold">{{ item.name }}</span>
                    <span v-if="!item.enabled" class="badge badge-ghost badge-xs">禁用</span>
                    <button class="btn btn-ghost btn-2xs" type="button" title="仅刷新此仓库" @click="workspaceStore.invalidateStatusEntry(item.repoId)">刷新</button>
                  </div>
                </td>
                <td>{{ item.currentBranch ?? '—' }}</td>
                <td>{{ formatSyncState(item.syncState) }}</td>
                <td>{{ item.ahead }} / {{ item.behind }}</td>
                <td>
                  <span :class="workingStateClass(item.workingState)">{{ formatWorkingState(item.workingState) }}</span>
                </td>
                <td>{{ item.untracked }}</td>
                <td>{{ item.statusTimestamp }}</td>
                <td>
                  <span v-if="item.error" class="badge badge-error badge-xs">{{ item.error }}</span>
                </td>
              </tr>
              <tr v-if="status.statuses.length === 0">
                <td colspan="8" class="text-center text-sm text-base-content/60">没有匹配的仓库。</td>
              </tr>
            </tbody>
          </table>
        </div>
      </div>
    </section>

    <!-- Batch Operations -->
    <section v-if="hasWorkspace" class="card bg-base-100 shadow-sm">
      <div class="card-body space-y-4">
        <div class="flex flex-wrap items-center gap-2 justify-between">
          <h2 class="card-title text-lg">批量操作</h2>
          <span class="text-sm text-base-content/60">已选仓库：{{ selectedRepoIds.length || '全部' }}</span>
        </div>
        <form class="grid gap-4 md:grid-cols-4" @submit.prevent="handleBatchSubmit">
          <label class="form-control">
            <span class="label-text">操作类型</span>
            <select v-model="batchOperation" class="select select-bordered select-sm">
              <option value="clone">批量 Clone</option>
              <option value="fetch">批量 Fetch</option>
              <option value="push">批量 Push</option>
            </select>
          </label>
          <label class="form-control">
            <span class="label-text">最大并发</span>
            <input v-model.number="batchConcurrency" type="number" min="1" class="input input-sm input-bordered" placeholder="默认为配置" />
          </label>
          <label class="form-control" v-if="batchOperation !== 'push'">
            <span class="label-text">Depth (可选)</span>
            <input v-model.number="batchDepth" type="number" min="1" class="input input-sm input-bordered" placeholder="不限制" />
          </label>
          <label class="form-control" v-if="batchOperation !== 'push'">
            <span class="label-text">Filter (可选)</span>
            <input v-model="batchFilter" type="text" class="input input-sm input-bordered" placeholder="blob:none" />
          </label>
          <label class="form-control" v-if="batchOperation === 'fetch'">
            <span class="label-text">预设</span>
            <select v-model="batchPreset" class="select select-bordered select-sm">
              <option value="remote">仅远程</option>
              <option value="branches">分支</option>
              <option value="branches+tags">分支+标签</option>
              <option value="tags">仅标签</option>
            </select>
          </label>
          <label class="form-control" v-if="batchOperation === 'push'">
            <span class="label-text">远程名称</span>
            <input v-model="batchRemote" type="text" class="input input-sm input-bordered" placeholder="origin" />
          </label>
          <label class="form-control" v-if="batchOperation === 'push'">
            <span class="label-text">用户名</span>
            <input v-model="batchUsername" type="text" class="input input-sm input-bordered" placeholder="可选" />
          </label>
          <label class="form-control" v-if="batchOperation === 'push'">
            <span class="label-text">密码 / Token</span>
            <input v-model="batchPassword" type="password" class="input input-sm input-bordered" placeholder="可选" />
          </label>
          <label class="form-control" v-if="batchOperation === 'clone'">
            <span class="label-text">递归子模块</span>
            <label class="label cursor-pointer justify-start gap-2">
              <input type="checkbox" class="checkbox checkbox-xs" v-model="cloneRecurseSubmodules" />
              <span class="label-text">自动初始化/更新</span>
            </label>
          </label>
          <div class="md:col-span-4 flex gap-2">
            <button class="btn btn-primary btn-sm" type="submit">启动批量任务</button>
            <button class="btn btn-ghost btn-sm" type="button" @click="workspaceStore.selectAll(false)">清空选中</button>
            <span v-if="lastBatchTaskId" class="text-sm text-base-content/60">最近批量任务：{{ lastBatchTaskId }}</span>
          </div>
        </form>

        <div
          v-if="activeBatchTask"
          class="space-y-2 rounded-lg border border-base-200 bg-base-200/40 p-3"
        >
          <div class="flex flex-wrap items-center justify-between gap-2">
            <div>
              <p class="font-semibold">
                {{ batchOperationLabel }}
                <span class="text-xs text-base-content/60">#{{ activeBatchTask.id }}</span>
              </p>
              <p class="text-xs text-base-content/60">
                状态：{{ translateTaskState(activeBatchTask.state) }}
                <span v-if="activeBatchProgress?.phase"> · {{ activeBatchProgress.phase }}</span>
              </p>
            </div>
            <div class="flex items-center gap-2">
              <span :class="batchStateClass(activeBatchTask.state)">
                {{ translateTaskState(activeBatchTask.state) }}
              </span>
              <button
                v-if="isBatchCancelable"
                class="btn btn-xs btn-outline"
                type="button"
                :disabled="cancelingBatch"
                @click="handleCancelBatch"
              >
                <span v-if="cancelingBatch" class="loading loading-spinner loading-2xs"></span>
                <span v-else>取消任务</span>
              </button>
            </div>
          </div>
          <progress class="progress progress-primary" :value="activeBatchProgress?.percent ?? 0" max="100"></progress>
          <p class="text-xs text-base-content/60">
            进度：{{ activeBatchProgress?.percent ?? 0 }}%
          </p>
          <p v-if="activeBatchError" class="text-xs text-error">
            错误：{{ activeBatchError.message }}
          </p>
        </div>
        <div v-else-if="lastBatchTaskId" class="text-xs text-base-content/60">
          最近批量任务 {{ lastBatchTaskId }} 已结束。
        </div>
      </div>
    </section>

    <!-- Team Config Template -->
    <section v-if="hasWorkspace" class="card bg-base-100 shadow-sm">
      <div class="card-body space-y-4">
        <h2 class="card-title text-lg">团队配置模板</h2>
        <div class="grid gap-4 md:grid-cols-2">
          <form class="space-y-3" @submit.prevent="handleExportTemplate">
            <h3 class="font-semibold">导出模板</h3>
            <label class="form-control">
              <span class="label-text">导出路径（可选）</span>
              <input v-model="exportPath" type="text" class="input input-sm input-bordered" placeholder="team-config-template.json" />
            </label>
            <div class="grid grid-cols-2 gap-2">
              <label class="label cursor-pointer justify-start gap-2">
                <input type="checkbox" class="checkbox checkbox-xs" v-model="exportIncludeIpPool" />
                <span class="label-text">IP 池运行态</span>
              </label>
              <label class="label cursor-pointer justify-start gap-2">
                <input type="checkbox" class="checkbox checkbox-xs" v-model="exportIncludeIpPoolFile" />
                <span class="label-text">IP 池文件</span>
              </label>
              <label class="label cursor-pointer justify-start gap-2">
                <input type="checkbox" class="checkbox checkbox-xs" v-model="exportIncludeProxy" />
                <span class="label-text">代理配置</span>
              </label>
              <label class="label cursor-pointer justify-start gap-2">
                <input type="checkbox" class="checkbox checkbox-xs" v-model="exportIncludeTls" />
                <span class="label-text">TLS 配置</span>
              </label>
              <label class="label cursor-pointer justify-start gap-2">
                <input type="checkbox" class="checkbox checkbox-xs" v-model="exportIncludeCredential" />
                <span class="label-text">凭证策略</span>
              </label>
            </div>
            <button class="btn btn-primary btn-sm" type="submit">导出模板</button>
            <p v-if="exportResult" class="text-xs text-base-content/60">已导出到：{{ exportResult }}</p>
          </form>

          <form class="space-y-3" @submit.prevent="handleImportTemplate">
            <h3 class="font-semibold">导入模板</h3>
            <label class="form-control">
              <span class="label-text">模板路径（可选）</span>
              <input v-model="importPath" type="text" class="input input-sm input-bordered" placeholder="team-config-template.json" />
            </label>
            <div class="grid grid-cols-2 gap-2">
              <label class="label cursor-pointer justify-start gap-2">
                <input type="checkbox" class="checkbox checkbox-xs" v-model="importIncludeIpPool" />
                <span class="label-text">IP 池运行态</span>
              </label>
              <label class="label cursor-pointer justify-start gap-2">
                <input type="checkbox" class="checkbox checkbox-xs" v-model="importIncludeIpPoolFile" />
                <span class="label-text">IP 池文件</span>
              </label>
              <label class="label cursor-pointer justify-start gap-2">
                <input type="checkbox" class="checkbox checkbox-xs" v-model="importIncludeProxy" />
                <span class="label-text">代理配置</span>
              </label>
              <label class="label cursor-pointer justify-start gap-2">
                <input type="checkbox" class="checkbox checkbox-xs" v-model="importIncludeTls" />
                <span class="label-text">TLS 配置</span>
              </label>
              <label class="label cursor-pointer justify-start gap-2">
                <input type="checkbox" class="checkbox checkbox-xs" v-model="importIncludeCredential" />
                <span class="label-text">凭证策略</span>
              </label>
            </div>
            <label class="form-control">
              <span class="label-text">策略（全部节应用）</span>
              <select v-model="importStrategy" class="select select-bordered select-sm">
                <option value="overwrite">覆盖</option>
                <option value="keepLocal">保留本地</option>
                <option value="merge">合并</option>
              </select>
            </label>
            <button class="btn btn-primary btn-sm" type="submit">导入模板</button>
            <div v-if="lastTemplateReport" class="text-xs text-base-content/60 space-y-1">
              <p>版本：{{ lastTemplateReport.schemaVersion }}</p>
              <p v-if="lastTemplateReport.backupPath">备份：{{ lastTemplateReport.backupPath }}</p>
              <p v-if="lastTemplateReport.applied.length">应用节：{{ lastTemplateReport.applied.map((s) => s.section).join(', ') }}</p>
              <p v-if="lastTemplateReport.skipped.length">跳过：{{ lastTemplateReport.skipped.map((s) => s.section).join(', ') }}</p>
              <p v-if="lastTemplateReport.warnings.length" class="text-error">警告：{{ lastTemplateReport.warnings.join('; ') }}</p>
            </div>
          </form>
        </div>
      </div>
    </section>
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, reactive, ref, watch } from 'vue';
import { storeToRefs } from 'pinia';
import { useWorkspaceStore } from '../stores/workspace';
import { useTasksStore } from '../stores/tasks';
import type {
  RepositoryInfo,
  WorkspaceBatchCloneRequest,
  WorkspaceBatchFetchRequest,
  WorkspaceBatchPushRequest,
} from '../api/workspace';
import type { SectionStrategy } from '../api/config';
import { cancelTask } from '../api/tasks';

const workspaceStore = useWorkspaceStore();
const { current, repositories, loadingWorkspace, loadingStatus, status, selectedRepoIds, lastError, lastBatchTaskId, lastTemplateReport, lastBatchOperation } = storeToRefs(workspaceStore);

const tasksStore = useTasksStore();
const { items: taskItems, progressById, lastErrorById } = storeToRefs(tasksStore);

const hasWorkspace = computed(() => workspaceStore.hasWorkspace);
const showCreateForm = ref(false);
const createForm = reactive({ name: '', rootPath: '', metadata: '' });
const loadPath = ref('');
const savePath = ref('');
const newRepoForm = reactive({ id: '', name: '', path: '', remoteUrl: '', tags: '' });
const draggingRepoId = ref<string | null>(null);

const filterName = ref('');
const filterBranch = ref('');
const includeDisabled = ref(false);
const dirtyFilter = ref<'all' | 'dirty' | 'clean'>('all');
const syncStateSelection = ref<string[]>([]);
const autoRefreshTimer = ref<number | null>(null);

const batchOperation = ref<'clone' | 'fetch' | 'push'>('clone');
const batchConcurrency = ref<number | null>(null);
const batchDepth = ref<number | null>(null);
const batchFilter = ref('');
const batchPreset = ref('remote');
const batchRemote = ref('origin');
const batchUsername = ref('');
const batchPassword = ref('');
const cloneRecurseSubmodules = ref(true);
const uiMessage = ref('');

const exportPath = ref('');
const exportIncludeIpPool = ref(true);
const exportIncludeIpPoolFile = ref(true);
const exportIncludeProxy = ref(true);
const exportIncludeTls = ref(true);
const exportIncludeCredential = ref(true);
const exportResult = ref('');

const importPath = ref('');
const importIncludeIpPool = ref(true);
const importIncludeIpPoolFile = ref(true);
const importIncludeProxy = ref(true);
const importIncludeTls = ref(true);
const importIncludeCredential = ref(true);
const importStrategy = ref<SectionStrategy>('overwrite');

const allSelected = computed(() => repositories.value.length > 0 && repositories.value.every((repo) => selectedRepoIds.value.includes(repo.id)));
const someSelected = computed(() => selectedRepoIds.value.length > 0 && !allSelected.value);
const cancelingBatch = ref(false);

const activeBatchTask = computed(() => {
  if (!lastBatchTaskId.value) return null;
  return taskItems.value.find((task) => task.id === lastBatchTaskId.value) ?? null;
});

const activeBatchProgress = computed(() => {
  const id = activeBatchTask.value?.id;
  if (!id) return null;
  return progressById.value[id] ?? null;
});

const activeBatchError = computed(() => {
  const id = activeBatchTask.value?.id;
  if (!id) return undefined;
  return lastErrorById.value[id];
});

const isBatchCancelable = computed(() => {
  const state = activeBatchTask.value?.state;
  return state === 'pending' || state === 'running';
});

const batchOperationLabel = computed(() => {
  switch (lastBatchOperation.value) {
    case 'clone':
      return '批量 Clone';
    case 'fetch':
      return '批量 Fetch';
    case 'push':
      return '批量 Push';
    default:
      return '批量任务';
  }
});

watch(
  () => lastBatchTaskId.value,
  () => {
    cancelingBatch.value = false;
  },
);

watch(
  () => activeBatchTask.value?.state,
  async (state, prev) => {
    if (!state || state === prev) return;
    if (!['completed', 'failed', 'canceled'].includes(state)) return;

    cancelingBatch.value = false;
    const taskId = activeBatchTask.value?.id ?? lastBatchTaskId.value ?? '';
    const taskLabel = taskId ? `批量任务 ${taskId}` : '批量任务';

    if (state === 'completed') {
      uiMessage.value = `${taskLabel} 已完成。`;
    } else if (state === 'failed') {
      workspaceStore.setError(`${taskLabel} 失败，请查看任务日志。`);
    } else if (state === 'canceled') {
      uiMessage.value = `${taskLabel} 已取消。`;
    }

    try {
      if (hasWorkspace.value) {
        await workspaceStore.refreshRepositories();
        await workspaceStore.fetchStatuses({ forceRefresh: true });
      }
    } catch (error: any) {
      workspaceStore.setError(error?.message ?? String(error));
    }
  },
);

function toggleCreateForm() {
  showCreateForm.value = !showCreateForm.value;
}

function resetNewRepoForm() {
  newRepoForm.id = '';
  newRepoForm.name = '';
  newRepoForm.path = '';
  newRepoForm.remoteUrl = '';
  newRepoForm.tags = '';
}

async function handleCreateWorkspace() {
  try {
    const metadata = createForm.metadata.trim() ? JSON.parse(createForm.metadata) : undefined;
    await workspaceStore.createWorkspace({ name: createForm.name, rootPath: createForm.rootPath, metadata });
    showCreateForm.value = false;
    uiMessage.value = `工作区 ${createForm.name} 创建成功。`;
    await workspaceStore.fetchStatuses();
  } catch (error: any) {
    workspaceStore.setError(error?.message ?? String(error));
  }
}

async function handleLoadWorkspace() {
  if (!loadPath.value) return;
  try {
    await workspaceStore.loadWorkspace(loadPath.value);
    uiMessage.value = `已加载工作区 ${loadPath.value}`;
    await workspaceStore.fetchStatuses();
  } catch (error: any) {
    workspaceStore.setError(error?.message ?? String(error));
  }
}

async function handleSaveWorkspace() {
  if (!savePath.value) return;
  try {
    await workspaceStore.saveWorkspace(savePath.value);
    uiMessage.value = `工作区已保存到 ${savePath.value}`;
  } catch (error: any) {
    workspaceStore.setError(error?.message ?? String(error));
  }
}

async function handleRefreshWorkspace() {
  await workspaceStore.refreshRepositories();
  await workspaceStore.fetchStatuses();
}

async function handleCloseWorkspace() {
  await workspaceStore.closeWorkspace();
  uiMessage.value = '已关闭当前工作区。';
}

async function handleAddRepository() {
  if (!newRepoForm.id || !newRepoForm.name || !newRepoForm.path || !newRepoForm.remoteUrl) return;
  const tags = newRepoForm.tags
    .split(',')
    .map((tag) => tag.trim())
    .filter((tag) => tag.length > 0);
  await workspaceStore.addRepository({
    id: newRepoForm.id.trim(),
    name: newRepoForm.name.trim(),
    path: newRepoForm.path.trim(),
    remoteUrl: newRepoForm.remoteUrl.trim(),
    tags,
  });
  resetNewRepoForm();
  uiMessage.value = '仓库已添加。';
}

function onDragStart(id: string) {
  draggingRepoId.value = id;
}

async function onDrop(targetId: string) {
  const sourceId = draggingRepoId.value;
  draggingRepoId.value = null;
  if (!sourceId || sourceId === targetId) return;
  const order = repositories.value.map((repo) => repo.id);
  const from = order.indexOf(sourceId);
  const to = order.indexOf(targetId);
  if (from < 0 || to < 0) return;
  order.splice(to, 0, order.splice(from, 1)[0]);
  await workspaceStore.reorderRepositories(order);
}

function editTags(repo: RepositoryInfo) {
  const currentTags = repo.tags.join(',');
  const next = window.prompt(`更新仓库 ${repo.name} 标签（逗号分隔）`, currentTags);
  if (next === null) return;
  const tags = next
    .split(',')
    .map((tag) => tag.trim())
    .filter((tag) => tag.length > 0);
  workspaceStore.updateRepositoryTags(repo.id, tags);
}

function formatSyncState(state: string) {
  switch (state) {
    case 'clean':
      return '已同步';
    case 'ahead':
      return '领先';
    case 'behind':
      return '落后';
    case 'diverged':
      return '分叉';
    case 'detached':
      return '游离';
    default:
      return '未知';
  }
}

function formatWorkingState(state: string) {
  switch (state) {
    case 'clean':
      return '干净';
    case 'dirty':
      return '有改动';
    case 'missing':
      return '缺失';
    case 'error':
      return '错误';
    default:
      return state;
  }
}

function workingStateClass(state: string) {
  if (state === 'dirty') return 'badge badge-warning badge-xs';
  if (state === 'error' || state === 'missing') return 'badge badge-error badge-xs';
  return 'badge badge-ghost badge-xs';
}

function translateTaskState(state?: string) {
  switch (state) {
    case 'pending':
      return '等待中';
    case 'running':
      return '进行中';
    case 'completed':
      return '已完成';
    case 'failed':
      return '失败';
    case 'canceled':
      return '已取消';
    default:
      return state ?? '未知';
  }
}

function batchStateClass(state?: string) {
  switch (state) {
    case 'completed':
      return 'badge badge-success badge-sm';
    case 'failed':
      return 'badge badge-error badge-sm';
    case 'canceled':
      return 'badge badge-warning badge-sm';
    case 'running':
      return 'badge badge-info badge-sm';
    case 'pending':
      return 'badge badge-outline badge-sm';
    default:
      return 'badge badge-outline badge-sm';
  }
}

async function applyFilters(forceRefresh: boolean) {
  const filter: any = {};
  if (filterName.value.trim()) filter.nameContains = filterName.value.trim();
  if (filterBranch.value.trim()) filter.branch = filterBranch.value.trim();
  if (syncStateSelection.value.length > 0) filter.syncStates = syncStateSelection.value.slice();
  if (dirtyFilter.value === 'dirty') filter.hasLocalChanges = true;
  if (dirtyFilter.value === 'clean') filter.hasLocalChanges = false;

  workspaceStore.setStatusQuery({
    includeDisabled: includeDisabled.value,
    filter,
  });
  await workspaceStore.fetchStatuses({ forceRefresh });
}

function resetFilters() {
  filterName.value = '';
  filterBranch.value = '';
  includeDisabled.value = false;
  dirtyFilter.value = 'all';
  syncStateSelection.value = [];
  workspaceStore.setStatusQuery({ filter: {}, includeDisabled: false });
  workspaceStore.fetchStatuses();
}

async function handleBatchSubmit() {
  const repoIds = selectedRepoIds.value.length > 0 ? selectedRepoIds.value.slice() : undefined;
  try {
    if (batchOperation.value === 'clone') {
      const request = {
        repoIds,
        includeDisabled: includeDisabled.value,
        maxConcurrency: batchConcurrency.value ?? undefined,
        depth: batchDepth.value ?? undefined,
        filter: batchFilter.value.trim() || undefined,
        recurseSubmodules: cloneRecurseSubmodules.value,
      } satisfies WorkspaceBatchCloneRequest;
      const taskId = await workspaceStore.startBatchClone(request);
      uiMessage.value = `批量 Clone 已启动，任务 ${taskId}`;
    } else if (batchOperation.value === 'fetch') {
      const request = {
        repoIds,
        includeDisabled: includeDisabled.value,
        maxConcurrency: batchConcurrency.value ?? undefined,
        depth: batchDepth.value ?? undefined,
        filter: batchFilter.value.trim() || undefined,
        preset: batchPreset.value !== 'remote' ? batchPreset.value : undefined,
      } satisfies WorkspaceBatchFetchRequest;
      const taskId = await workspaceStore.startBatchFetch(request);
      uiMessage.value = `批量 Fetch 已启动，任务 ${taskId}`;
    } else {
      const request = {
        repoIds,
        includeDisabled: includeDisabled.value,
        maxConcurrency: batchConcurrency.value ?? undefined,
        remote: batchRemote.value.trim() || undefined,
        username: batchUsername.value.trim() || undefined,
        password: batchPassword.value.trim() || undefined,
      } satisfies WorkspaceBatchPushRequest;
      const taskId = await workspaceStore.startBatchPush(request);
      uiMessage.value = `批量 Push 已启动，任务 ${taskId}`;
    }
  } catch (error: any) {
    workspaceStore.setError(error?.message ?? String(error));
  }
}

async function handleCancelBatch() {
  if (!activeBatchTask.value || !isBatchCancelable.value) return;
  let cancelled = false;
  try {
    cancelingBatch.value = true;
    cancelled = await cancelTask(activeBatchTask.value.id);
    if (!cancelled) {
      uiMessage.value = `批量任务 ${activeBatchTask.value.id} 当前无法取消。`;
    }
  } catch (error: any) {
    workspaceStore.setError(error?.message ?? String(error));
  } finally {
    if (!cancelled) {
      cancelingBatch.value = false;
    }
  }
}

async function handleExportTemplate() {
  try {
    const path = await workspaceStore.exportTeamConfig(exportPath.value.trim() || undefined, {
      includeIpPool: exportIncludeIpPool.value,
      includeIpPoolFile: exportIncludeIpPoolFile.value,
      includeProxy: exportIncludeProxy.value,
      includeTls: exportIncludeTls.value,
      includeCredential: exportIncludeCredential.value,
    });
    exportResult.value = path;
    uiMessage.value = `模板已导出到 ${path}`;
  } catch (error: any) {
    workspaceStore.setError(error?.message ?? String(error));
  }
}

async function handleImportTemplate() {
  try {
    await workspaceStore.importTeamConfig(importPath.value.trim() || undefined, {
      includeIpPool: importIncludeIpPool.value,
      includeIpPoolFile: importIncludeIpPoolFile.value,
      includeProxy: importIncludeProxy.value,
      includeTls: importIncludeTls.value,
      includeCredential: importIncludeCredential.value,
      strategies: {
        ipPool: importStrategy.value,
        ipPoolFile: importStrategy.value,
        proxy: importStrategy.value,
        tls: importStrategy.value,
        credential: importStrategy.value,
      },
    });
    uiMessage.value = '模板导入完成。';
  } catch (error: any) {
    workspaceStore.setError(error?.message ?? String(error));
  }
}

function clearAutoRefreshTimer() {
  if (autoRefreshTimer.value !== null) {
    window.clearInterval(autoRefreshTimer.value);
    autoRefreshTimer.value = null;
  }
}

onMounted(async () => {
  await workspaceStore.initialize();
  if (hasWorkspace.value) {
    await workspaceStore.fetchStatuses();
  }
});

watch(
  () => status.value?.autoRefreshSecs,
  (secs) => {
    clearAutoRefreshTimer();
    if (secs && secs > 0) {
      autoRefreshTimer.value = window.setInterval(() => {
        workspaceStore.fetchStatuses();
      }, secs * 1000);
    }
  },
);

watch(
  () => current.value?.updatedAt,
  async (updated) => {
    if (updated) {
      await workspaceStore.fetchStatuses();
    }
  },
);

onBeforeUnmount(() => {
  clearAutoRefreshTimer();
});
</script>
