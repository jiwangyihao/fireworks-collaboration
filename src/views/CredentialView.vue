<template>
  <div class="p-4 pt-16 space-y-4">
    <div class="flex justify-between items-center">
      <h2 class="text-2xl font-bold">凭证管理</h2>
      <div class="flex gap-2">
        <button
          v-if="!needsUnlock"
          class="btn btn-sm btn-outline"
          @click="refreshCredentials"
          :disabled="credentialStore.loading"
        >
          <svg
            xmlns="http://www.w3.org/2000/svg"
            width="16"
            height="16"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
            stroke-linejoin="round"
            :class="{ 'animate-spin': credentialStore.loading }"
          >
            <polyline points="23 4 23 10 17 10"></polyline>
            <polyline points="1 20 1 14 7 14"></polyline>
            <path
              d="M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15"
            ></path>
          </svg>
          刷新
        </button>
      </div>
    </div>

    <!-- Error Alert -->
    <div v-if="credentialStore.error" class="alert alert-error shadow-sm">
      <svg
        xmlns="http://www.w3.org/2000/svg"
        class="stroke-current shrink-0 h-6 w-6"
        fill="none"
        viewBox="0 0 24 24"
      >
        <path
          stroke-linecap="round"
          stroke-linejoin="round"
          stroke-width="2"
          d="M10 14l2-2m0 0l2-2m-2 2l-2-2m2 2l2 2m7-2a9 9 0 11-18 0 9 9 0 0118 0z"
        />
      </svg>
      <div>
        <h3 class="font-bold">操作失败</h3>
        <div class="text-sm">{{ credentialStore.error }}</div>
      </div>
      <button class="btn btn-sm btn-ghost" @click="credentialStore.clearError()">
        关闭
      </button>
    </div>

    <!-- Unlock Prompt -->
    <div v-if="needsUnlock" class="card bg-warning text-warning-content shadow-sm">
      <div class="card-body">
        <h3 class="card-title">
          <svg
            xmlns="http://www.w3.org/2000/svg"
            width="20"
            height="20"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
            stroke-linejoin="round"
          >
            <rect x="3" y="11" width="18" height="11" rx="2" ry="2"></rect>
            <path d="M7 11V7a5 5 0 0 1 9.9-1"></path>
          </svg>
          凭证存储已加密
        </h3>
        <p>您的凭证存储使用主密码加密，需要解锁后才能访问。</p>
        <div class="card-actions justify-end">
          <button class="btn btn-sm btn-primary" @click="showUnlockDialog = true">
            解锁存储
          </button>
        </div>
      </div>
    </div>

    <!-- Credential Manager -->
    <div v-else class="space-y-4">
      <div class="flex gap-2">
        <button
          class="btn btn-primary btn-sm"
          @click="showAddForm = !showAddForm"
        >
          <svg
            v-if="!showAddForm"
            xmlns="http://www.w3.org/2000/svg"
            width="16"
            height="16"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
            stroke-linejoin="round"
          >
            <line x1="12" y1="5" x2="12" y2="19"></line>
            <line x1="5" y1="12" x2="19" y2="12"></line>
          </svg>
          <svg
            v-else
            xmlns="http://www.w3.org/2000/svg"
            width="16"
            height="16"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
            stroke-linejoin="round"
          >
            <line x1="18" y1="6" x2="6" y2="18"></line>
            <line x1="6" y1="6" x2="18" y2="18"></line>
          </svg>
          {{ showAddForm ? '取消添加' : '添加凭证' }}
        </button>
        <button
          class="btn btn-outline btn-sm"
          @click="exportLog"
          :disabled="exporting || credentialStore.loading"
        >
          <span v-if="exporting" class="loading loading-spinner loading-xs"></span>
          <svg
            v-else
            xmlns="http://www.w3.org/2000/svg"
            width="16"
            height="16"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
            stroke-linejoin="round"
          >
            <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"></path>
            <polyline points="7 10 12 15 17 10"></polyline>
            <line x1="12" y1="15" x2="12" y2="3"></line>
          </svg>
          {{ exporting ? '导出中...' : '导出审计日志' }}
        </button>
      </div>

      <CredentialForm
        v-if="showAddForm"
        @success="onAddSuccess"
        @cancel="showAddForm = false"
      />

      <CredentialList @edit="onEdit" />
    </div>

    <!-- Master Password Dialog -->
    <MasterPasswordDialog
      :show="showUnlockDialog"
      :is-first-time="isFirstTime"
      @close="showUnlockDialog = false"
      @success="onUnlockSuccess"
    />
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue';
import { useCredentialStore } from '../stores/credential';
import CredentialForm from '../components/CredentialForm.vue';
import CredentialList from '../components/CredentialList.vue';
import MasterPasswordDialog from '../components/MasterPasswordDialog.vue';

const credentialStore = useCredentialStore();

const showAddForm = ref(false);
const showUnlockDialog = ref(false);
const isFirstTime = ref(false);
const exporting = ref(false);

const needsUnlock = computed(() => credentialStore.needsUnlock);

onMounted(async () => {
  try {
    // Load credential config from app config
    // For now, use default config (you can load from actual app config)
    credentialStore.setConfig({
      storage: 'system', // or 'file' or 'memory'
      auditMode: true,
    });

    // Try to refresh credentials
    await credentialStore.refresh();

    // If using file storage and no credentials, show unlock dialog
    if (needsUnlock.value) {
      showUnlockDialog.value = true;
    }
  } catch (error: any) {
    console.error('Failed to initialize credential view:', error);
  }
});

const refreshCredentials = async () => {
  try {
    await credentialStore.refresh();
  } catch (error: any) {
    console.error('Failed to refresh credentials:', error);
  }
};

const onAddSuccess = () => {
  showAddForm.value = false;
  refreshCredentials();
};

const onEdit = (credential: any) => {
  // TODO: Implement edit mode
  alert(`编辑功能待实现: ${credential.host} (${credential.username})`);
};

const onUnlockSuccess = () => {
  showUnlockDialog.value = false;
  refreshCredentials();
};

const exportLog = async () => {
  exporting.value = true;
  try {
    const logJson = await credentialStore.exportLog();
    // Create a download link
    const blob = new Blob([logJson], { type: 'application/json' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `credential-audit-${Date.now()}.json`;
    a.click();
    URL.revokeObjectURL(url);
  } catch (error: any) {
    console.error('Failed to export audit log:', error);
    credentialStore.error = `导出失败: ${error.message || error}`;
  } finally {
    exporting.value = false;
  }
};
</script>
