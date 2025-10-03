<template>
  <div class="card bg-base-100 shadow-sm">
    <div class="card-body">
      <div class="flex justify-between items-center mb-4">
        <h4 class="card-title text-base">已保存的凭证 ({{ credentials.length }})</h4>
      </div>

      <div v-if="loading && credentials.length === 0" class="flex justify-center py-12">
        <span class="loading loading-spinner loading-md"></span>
      </div>

      <div v-else-if="credentials.length === 0" class="text-center py-12 opacity-60">
        <svg
          xmlns="http://www.w3.org/2000/svg"
          width="48"
          height="48"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="1"
          stroke-linecap="round"
          stroke-linejoin="round"
          class="mx-auto mb-4 opacity-40"
        >
          <rect x="3" y="11" width="18" height="11" rx="2" ry="2"></rect>
          <path d="M7 11V7a5 5 0 0 1 10 0v4"></path>
        </svg>
        <p>暂无凭证。点击上方的"添加凭证"按钮开始使用。</p>
      </div>

      <div v-else class="space-y-3">
        <div
          v-for="cred in credentials"
          :key="`${cred.host}-${cred.username}`"
          class="card bg-base-200 shadow-sm hover:shadow-md transition-shadow"
          :class="{
            'border-2 border-error': cred.isExpired,
            'border-2 border-warning': !cred.isExpired && isExpiringSoon(cred.expiresAt),
          }"
        >
          <div class="card-body p-4">
            <div class="flex justify-between items-start">
              <div class="flex-1 space-y-1">
                <div class="flex items-center gap-2">
                  <strong class="text-base">{{ cred.host }}</strong>
                  <div
                    v-if="cred.isExpired"
                    class="badge badge-error badge-sm"
                  >
                    已过期
                  </div>
                  <div
                    v-else-if="isExpiringSoon(cred.expiresAt)"
                    class="badge badge-warning badge-sm"
                  >
                    即将过期
                  </div>
                </div>
                <div class="text-sm opacity-70">用户名: {{ cred.username }}</div>
                <div class="text-sm opacity-70 font-mono">
                  密码: {{ cred.maskedPassword }}
                </div>
                <div class="text-xs opacity-50 mt-2">
                  <span>创建于: {{ formatDate(cred.createdAt) }}</span>
                  <span v-if="cred.expiresAt">
                    | 过期于: {{ formatDate(cred.expiresAt) }}
                  </span>
                  <span v-if="cred.lastUsedAt">
                    | 最后使用: {{ formatDate(cred.lastUsedAt) }}
                  </span>
                </div>
              </div>

              <div class="flex gap-1">
                <button
                  class="btn btn-sm btn-ghost btn-square"
                  @click="editCredential(cred)"
                  title="编辑"
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
                  >
                    <path
                      d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7"
                    ></path>
                    <path
                      d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z"
                    ></path>
                  </svg>
                </button>
                <button
                  class="btn btn-sm btn-ghost btn-square text-error"
                  @click="deleteCredential(cred)"
                  title="删除"
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
                  >
                    <polyline points="3 6 5 6 21 6"></polyline>
                    <path
                      d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"
                    ></path>
                    <line x1="10" y1="11" x2="10" y2="17"></line>
                    <line x1="14" y1="11" x2="14" y2="17"></line>
                  </svg>
                </button>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>

    <!-- Delete Confirmation Dialog -->
    <ConfirmDialog
      :show="showDeleteConfirm"
      title="删除凭证"
      :message="`确定要删除凭证 ${credentialToDelete?.host} (${credentialToDelete?.username}) 吗？此操作不可撤销。`"
      variant="danger"
      @confirm="handleDeleteConfirm"
      @cancel="handleDeleteCancel"
    />
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue';
import { useCredentialStore } from '../stores/credential';
import { formatTimestamp, isExpiringSoon } from '../api/credential';
import type { CredentialInfo } from '../api/credential';
import ConfirmDialog from './ConfirmDialog.vue';

const credentialStore = useCredentialStore();

const emit = defineEmits<{
  edit: [credential: CredentialInfo];
}>();

const credentials = computed(() => credentialStore.sortedCredentials);
const loading = computed(() => credentialStore.loading);

const showDeleteConfirm = ref(false);
const credentialToDelete = ref<CredentialInfo | null>(null);

const formatDate = (timestamp: number) => {
  return formatTimestamp(timestamp);
};

const editCredential = (cred: CredentialInfo) => {
  emit('edit', cred);
};

const deleteCredential = async (cred: CredentialInfo) => {
  credentialToDelete.value = cred;
  showDeleteConfirm.value = true;
};

const handleDeleteConfirm = async () => {
  if (!credentialToDelete.value) return;
  
  const cred = credentialToDelete.value;
  showDeleteConfirm.value = false;
  credentialToDelete.value = null;
  
  try {
    await credentialStore.delete(cred.host, cred.username);
  } catch (error: any) {
    console.error('Failed to delete credential:', error);
    alert(`删除失败: ${error.message || error}`);
  }
};

const handleDeleteCancel = () => {
  showDeleteConfirm.value = false;
  credentialToDelete.value = null;
};
</script>
