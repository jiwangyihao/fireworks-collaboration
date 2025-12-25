<template>
  <div class="p-6 space-y-6">
    <h2 class="text-2xl font-bold">GitHub 操作面板</h2>

    <!-- 用户信息 -->
    <div v-if="userInfo" class="card bg-base-200 shadow-xl">
      <div class="card-body">
        <h3 class="card-title">当前用户</h3>
        <div class="flex items-center gap-3">
          <img
            :src="userInfo.avatar_url"
            :alt="userInfo.login"
            class="w-12 h-12 rounded-full"
          />
          <div>
            <p class="font-bold">{{ userInfo.name || userInfo.login }}</p>
            <p class="text-sm opacity-70">{{ userInfo.email }}</p>
          </div>
        </div>
      </div>
    </div>

    <!-- 仓库操作 -->
    <div class="card bg-base-100 shadow-xl">
      <div class="card-body">
        <h3 class="card-title">仓库操作</h3>

        <div class="form-control">
          <label class="label">
            <span class="label-text">目标仓库</span>
          </label>
          <div class="flex gap-2">
            <input
              v-model="targetRepo.owner"
              type="text"
              placeholder="owner"
              class="input input-bordered flex-1"
            />
            <span class="self-center">/</span>
            <input
              v-model="targetRepo.name"
              type="text"
              placeholder="repository"
              class="input input-bordered flex-1"
            />
          </div>
        </div>

        <div class="flex gap-2 mt-4">
          <button
            @click="checkForkStatus"
            class="btn btn-outline"
            :class="{ loading: loading.checkFork }"
          >
            检查Fork状态
          </button>
          <button
            @click="forkRepo"
            class="btn btn-primary"
            :class="{ loading: loading.fork }"
            :disabled="!targetRepo.owner || !targetRepo.name"
          >
            Fork仓库
          </button>
          <button
            v-if="
              forkStatus &&
              forkStatus.isForked &&
              forkStatus.syncStatus &&
              !forkStatus.syncStatus.isSynced
            "
            @click="syncForkRepo"
            class="btn btn-success"
            :class="{ loading: loading.syncFork }"
          >
            同步Fork
          </button>
        </div>

        <!-- Fork状态显示 -->
        <div
          v-if="forkStatus"
          class="alert mt-4"
          :class="{
            'alert-success':
              forkStatus.isForked && forkStatus.syncStatus?.isSynced,
            'alert-warning':
              forkStatus.isForked &&
              forkStatus.syncStatus &&
              !forkStatus.syncStatus.isSynced,
            'alert-info': !forkStatus.isForked,
          }"
        >
          <div class="flex flex-col w-full">
            <span class="font-semibold">{{ forkStatus.message }}</span>

            <!-- 同步状态详情 -->
            <div
              v-if="forkStatus.isForked && forkStatus.syncStatus"
              class="mt-2 text-sm"
            >
              <div class="flex items-center gap-4">
                <span
                  v-if="forkStatus.syncStatus.isSynced"
                  class="badge badge-success gap-1"
                >
                  <BaseIcon icon="lucide--check" size="xs" /> 已同步
                </span>
                <span v-else class="badge badge-warning"> 需要同步 </span>

                <span
                  v-if="forkStatus.syncStatus.behindBy > 0"
                  class="text-orange-600"
                >
                  落后 {{ forkStatus.syncStatus.behindBy }} 个提交
                </span>

                <span
                  v-if="forkStatus.syncStatus.aheadBy > 0"
                  class="text-blue-600"
                >
                  领先 {{ forkStatus.syncStatus.aheadBy }} 个提交
                </span>
              </div>

              <!-- 同步建议 -->
              <div class="mt-2 text-xs opacity-75 flex items-center gap-1">
                <BaseIcon icon="lucide--lightbulb" size="sm" />
                建议同步Fork以获取上游仓库的最新更改
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>

    <!-- Pull Request 创建 -->
    <div class="card bg-base-100 shadow-xl">
      <div class="card-body">
        <h3 class="card-title">创建 Pull Request</h3>

        <div class="form-control">
          <label class="label">
            <span class="label-text">PR 标题</span>
          </label>
          <input
            v-model="pullRequest.title"
            type="text"
            placeholder="Pull Request 标题"
            class="input input-bordered"
          />
        </div>

        <div class="form-control">
          <label class="label">
            <span class="label-text">PR 描述</span>
          </label>
          <textarea
            v-model="pullRequest.body"
            placeholder="Pull Request 描述"
            class="textarea textarea-bordered h-24"
          ></textarea>
        </div>

        <div class="grid grid-cols-2 gap-4">
          <div class="form-control">
            <label class="label">
              <span class="label-text">源分支</span>
            </label>
            <input
              v-model="pullRequest.head"
              type="text"
              placeholder="username:branch"
              class="input input-bordered"
            />
          </div>
          <div class="form-control">
            <label class="label">
              <span class="label-text">目标分支</span>
            </label>
            <input
              v-model="pullRequest.base"
              type="text"
              placeholder="main"
              class="input input-bordered"
            />
          </div>
        </div>

        <div class="form-control">
          <label class="cursor-pointer label">
            <span class="label-text">草稿PR</span>
            <input
              v-model="pullRequest.draft"
              type="checkbox"
              class="checkbox"
            />
          </label>
        </div>

        <button
          @click="createPR"
          class="btn btn-success mt-4"
          :class="{ loading: loading.createPR }"
          :disabled="
            !pullRequest.title || !pullRequest.head || !pullRequest.base
          "
        >
          创建 Pull Request
        </button>
      </div>
    </div>

    <!-- SSH密钥管理 -->
    <div class="card bg-base-100 shadow-xl">
      <div class="card-body">
        <h3 class="card-title">SSH 密钥管理</h3>

        <div class="flex gap-2 mb-4">
          <button
            @click="loadSSHKeys"
            class="btn btn-outline"
            :class="{ loading: loading.loadKeys }"
          >
            刷新密钥列表
          </button>
          <button @click="showAddKeyModal = true" class="btn btn-primary">
            添加SSH密钥
          </button>
        </div>

        <div v-if="sshKeys.length > 0" class="space-y-2">
          <div v-for="key in sshKeys" :key="key.id" class="card bg-base-200">
            <div class="card-body p-4">
              <div class="flex justify-between items-center">
                <div>
                  <h4 class="font-bold">{{ key.title }}</h4>
                  <p class="text-sm opacity-70">
                    {{ key.key.substring(0, 50) }}...
                  </p>
                  <p class="text-xs opacity-50">
                    添加于: {{ new Date(key.created_at).toLocaleDateString() }}
                  </p>
                </div>
                <button
                  @click="deleteKey(key.id)"
                  class="btn btn-error btn-sm"
                  :class="{ loading: loading.deleteKey === key.id }"
                >
                  删除
                </button>
              </div>
            </div>
          </div>
        </div>

        <div v-else-if="!loading.loadKeys" class="text-center py-4 opacity-70">
          暂无SSH密钥
        </div>
      </div>
    </div>

    <!-- 添加SSH密钥模态框 -->
    <div v-if="showAddKeyModal" class="modal modal-open">
      <div class="modal-box">
        <h3 class="font-bold text-lg">添加SSH密钥</h3>

        <div class="form-control mt-4">
          <label class="label">
            <span class="label-text">密钥标题</span>
          </label>
          <input
            v-model="newSSHKey.title"
            type="text"
            placeholder="例如: My Computer"
            class="input input-bordered"
          />
        </div>

        <div class="form-control mt-4">
          <label class="label">
            <span class="label-text">公钥内容</span>
          </label>
          <textarea
            v-model="newSSHKey.key"
            placeholder="ssh-rsa AAAAB3NzaC1yc2E..."
            class="textarea textarea-bordered h-32"
          ></textarea>
        </div>

        <div class="modal-action">
          <button
            @click="addSSHKey"
            class="btn btn-primary"
            :class="{ loading: loading.addKey }"
            :disabled="!newSSHKey.title || !newSSHKey.key"
          >
            添加
          </button>
          <button @click="showAddKeyModal = false" class="btn">取消</button>
        </div>
      </div>
    </div>

    <!-- 操作结果显示 -->
    <div
      v-if="operationResult"
      class="alert mt-4"
      :class="{
        'alert-success': operationResult.type === 'success',
        'alert-error': operationResult.type === 'error',
        'alert-info': operationResult.type === 'info',
      }"
    >
      <span>{{ operationResult.message }}</span>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from "vue";
import { getUserInfo } from "../../utils/github-auth";
import BaseIcon from "../../components/BaseIcon.vue";
import {
  forkRepository,
  createPullRequest,
  listSSHKeys,
  addSSHKey as addSSHKeyAPI,
  deleteSSHKey,
  checkIfForked,
  syncFork,
} from "../../utils/github-api";

// 响应式数据
const userInfo = ref<any>(null);
const targetRepo = ref({
  owner: "HIT-Fireworks",
  name: "fireworks-notes-society",
});
const forkStatus = ref<{
  isForked: boolean;
  message: string;
  syncStatus?: {
    aheadBy: number;
    behindBy: number;
    isSynced: boolean;
  };
  forkData?: any;
} | null>(null);

const pullRequest = ref({
  title: "",
  body: "",
  head: "",
  base: "main",
  draft: false,
});

const sshKeys = ref<any[]>([]);
const showAddKeyModal = ref(false);
const newSSHKey = ref({ title: "", key: "" });

const loading = ref({
  checkFork: false,
  fork: false,
  createPR: false,
  loadKeys: false,
  addKey: false,
  deleteKey: null as number | null,
  syncFork: false,
});

const operationResult = ref<{
  type: "success" | "error" | "info";
  message: string;
} | null>(null);

// 显示操作结果
function showResult(type: "success" | "error" | "info", message: string) {
  operationResult.value = { type, message };
  setTimeout(() => {
    operationResult.value = null;
  }, 5000);
}

// 检查Fork状态
async function checkForkStatus() {
  if (!targetRepo.value.owner || !targetRepo.value.name || !userInfo.value)
    return;

  loading.value.checkFork = true;
  try {
    const result = await checkIfForked(
      targetRepo.value.owner,
      targetRepo.value.name,
      userInfo.value.login
    );

    let message;
    if (result.isForked) {
      if (result.syncStatus?.isSynced) {
        message = "已Fork且与上游仓库同步";
      } else {
        message = "已Fork但需要同步";
      }
    } else {
      message = "尚未Fork该仓库";
    }

    forkStatus.value = {
      isForked: result.isForked,
      message,
      syncStatus: result.syncStatus,
      forkData: result.forkData,
    };
  } catch (error) {
    showResult("error", `检查Fork状态失败: ${error}`);
  } finally {
    loading.value.checkFork = false;
  }
}

// Fork仓库
async function forkRepo() {
  if (!targetRepo.value.owner || !targetRepo.value.name) return;

  loading.value.fork = true;
  try {
    await forkRepository(targetRepo.value.owner, targetRepo.value.name);
    showResult("success", "Fork仓库成功！");
    await checkForkStatus();
  } catch (error) {
    showResult("error", `Fork仓库失败: ${error}`);
  } finally {
    loading.value.fork = false;
  }
}

// 同步Fork仓库
async function syncForkRepo() {
  if (!userInfo.value || !targetRepo.value.name) return;

  loading.value.syncFork = true;
  try {
    await syncFork(userInfo.value.login, targetRepo.value.name);
    showResult("success", "Fork同步成功！");

    // 重新检查状态
    await checkForkStatus();
  } catch (error) {
    showResult("error", `同步Fork失败: ${error}`);
  } finally {
    loading.value.syncFork = false;
  }
}

// 创建PR
async function createPR() {
  if (!targetRepo.value.owner || !targetRepo.value.name) return;

  loading.value.createPR = true;
  try {
    const pr = await createPullRequest(
      targetRepo.value.owner,
      targetRepo.value.name,
      {
        title: pullRequest.value.title,
        body: pullRequest.value.body,
        head: pullRequest.value.head,
        base: pullRequest.value.base,
        draft: pullRequest.value.draft,
      }
    );

    showResult("success", `PR创建成功！PR #${pr.number}`);

    // 清空表单
    pullRequest.value = {
      title: "",
      body: "",
      head: "",
      base: "main",
      draft: false,
    };
  } catch (error) {
    showResult("error", `创建PR失败: ${error}`);
  } finally {
    loading.value.createPR = false;
  }
}

// 加载SSH密钥
async function loadSSHKeys() {
  loading.value.loadKeys = true;
  try {
    sshKeys.value = await listSSHKeys();
  } catch (error) {
    showResult("error", `加载SSH密钥失败: ${error}`);
  } finally {
    loading.value.loadKeys = false;
  }
}

// 添加SSH密钥
async function addSSHKey() {
  if (!newSSHKey.value.title || !newSSHKey.value.key) return;

  loading.value.addKey = true;
  try {
    await addSSHKeyAPI(newSSHKey.value.title, newSSHKey.value.key);
    showResult("success", "SSH密钥添加成功！");

    // 清空表单并关闭模态框
    newSSHKey.value = { title: "", key: "" };
    showAddKeyModal.value = false;

    // 重新加载密钥列表
    await loadSSHKeys();
  } catch (error) {
    showResult("error", `添加SSH密钥失败: ${error}`);
  } finally {
    loading.value.addKey = false;
  }
}

// 删除SSH密钥
async function deleteKey(keyId: number) {
  loading.value.deleteKey = keyId;
  try {
    await deleteSSHKey(keyId);
    showResult("success", "SSH密钥删除成功！");

    // 重新加载密钥列表
    await loadSSHKeys();
  } catch (error) {
    showResult("error", `删除SSH密钥失败: ${error}`);
  } finally {
    loading.value.deleteKey = null;
  }
}

// 页面加载时初始化
onMounted(async () => {
  try {
    const token = localStorage.getItem("github_access_token");
    if (token) {
      userInfo.value = await getUserInfo(token);
      if (userInfo.value) {
        pullRequest.value.head = `${userInfo.value.login}:feature-branch`;
      }
    }
  } catch (error) {
    showResult("error", `获取用户信息失败: ${error}`);
  }
});
</script>
