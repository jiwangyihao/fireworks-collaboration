<template>
  <div class="p-4 pt-16 space-y-4">
    <div class="flex justify-between items-center">
      <h2 class="text-2xl font-bold">审计日志</h2>
      <div class="flex gap-2">
        <button
          class="btn btn-sm btn-outline"
          @click="refreshLogs"
          :disabled="loading"
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
            :class="{ 'animate-spin': loading }"
          >
            <polyline points="23 4 23 10 17 10"></polyline>
            <polyline points="1 20 1 14 7 14"></polyline>
            <path
              d="M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15"
            ></path>
          </svg>
          刷新
        </button>
        <button
          class="btn btn-sm btn-outline"
          @click="exportLogs"
          :disabled="loading || exporting"
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
          导出
        </button>
        <button
          class="btn btn-sm btn-warning"
          @click="showCleanupDialog = true"
          :disabled="loading || logs.length === 0"
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
          </svg>
          清理旧日志
        </button>
      </div>
    </div>

    <!-- Error Alert -->
    <div v-if="error" class="alert alert-error shadow-sm">
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
        <div class="text-sm">{{ error }}</div>
      </div>
      <button class="btn btn-sm btn-ghost" @click="error = null">关闭</button>
    </div>

    <!-- Filters -->
    <div class="card bg-base-100 shadow-sm">
      <div class="card-body p-4">
        <div class="grid grid-cols-1 md:grid-cols-3 gap-4">
          <div class="form-control">
            <label class="label">
              <span class="label-text">操作类型</span>
            </label>
            <select v-model="filterOperation" class="select select-bordered select-sm">
              <option value="">全部</option>
              <option value="Add">添加</option>
              <option value="Get">获取</option>
              <option value="Update">更新</option>
              <option value="Delete">删除</option>
              <option value="Unlock">解锁</option>
            </select>
          </div>
          <div class="form-control">
            <label class="label">
              <span class="label-text">状态</span>
            </label>
            <select v-model="filterSuccess" class="select select-bordered select-sm">
              <option value="">全部</option>
              <option value="true">成功</option>
              <option value="false">失败</option>
            </select>
          </div>
          <div class="form-control">
            <label class="label">
              <span class="label-text">主机过滤</span>
            </label>
            <input
              v-model="filterHost"
              type="text"
              placeholder="输入主机名..."
              class="input input-bordered input-sm"
            />
          </div>
        </div>
      </div>
    </div>

    <!-- Logs Table -->
    <div class="card bg-base-100 shadow-sm">
      <div class="card-body p-0">
        <div v-if="loading && logs.length === 0" class="flex justify-center py-12">
          <span class="loading loading-spinner loading-md"></span>
        </div>

        <div v-else-if="filteredLogs.length === 0" class="text-center py-12 opacity-60">
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
            <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"></path>
            <polyline points="14 2 14 8 20 8"></polyline>
            <line x1="12" y1="18" x2="12" y2="12"></line>
            <line x1="9" y1="15" x2="15" y2="15"></line>
          </svg>
          <p>{{ logs.length === 0 ? '暂无审计日志' : '没有符合条件的日志' }}</p>
        </div>

        <div v-else class="overflow-x-auto">
          <table class="table table-zebra table-sm">
            <thead>
              <tr>
                <th>时间</th>
                <th>操作</th>
                <th>主机</th>
                <th>用户名</th>
                <th>状态</th>
                <th>备注</th>
              </tr>
            </thead>
            <tbody>
              <tr
                v-for="(log, index) in paginatedLogs"
                :key="index"
                :class="{
                  'bg-error bg-opacity-10': !log.success,
                }"
              >
                <td class="text-xs">{{ formatTimestamp(log.timestamp) }}</td>
                <td>
                  <span class="badge badge-sm" :class="getOperationBadgeClass(log.operation)">
                    {{ translateOperation(log.operation) }}
                  </span>
                </td>
                <td class="font-mono text-xs">{{ log.host }}</td>
                <td class="font-mono text-xs">{{ log.username }}</td>
                <td>
                  <span
                    class="badge badge-sm"
                    :class="log.success ? 'badge-success' : 'badge-error'"
                  >
                    {{ log.success ? '成功' : '失败' }}
                  </span>
                </td>
                <td class="text-xs opacity-70">{{ log.notes || '-' }}</td>
              </tr>
            </tbody>
          </table>
        </div>

        <!-- Pagination -->
        <div v-if="filteredLogs.length > pageSize" class="flex justify-center gap-2 p-4 border-t">
          <button
            class="btn btn-sm"
            :disabled="currentPage === 1"
            @click="currentPage--"
          >
            上一页
          </button>
          <span class="flex items-center px-4">
            第 {{ currentPage }} / {{ totalPages }} 页
          </span>
          <button
            class="btn btn-sm"
            :disabled="currentPage === totalPages"
            @click="currentPage++"
          >
            下一页
          </button>
        </div>
      </div>
    </div>

    <!-- Cleanup Dialog -->
    <div v-if="showCleanupDialog" class="modal modal-open">
      <div class="modal-box">
        <h3 class="font-bold text-lg">清理旧审计日志</h3>
        <div class="py-4 space-y-4">
          <p>选择要保留的日志天数，更早的日志将被删除。</p>
          <div class="form-control">
            <label class="label">
              <span class="label-text">保留天数</span>
            </label>
            <input
              v-model.number="retentionDays"
              type="number"
              min="1"
              max="365"
              class="input input-bordered"
              placeholder="例如：30"
            />
          </div>
        </div>
        <div class="modal-action">
          <button class="btn" @click="showCleanupDialog = false">取消</button>
          <button class="btn btn-warning" @click="handleCleanup">确认清理</button>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue';
import { exportAuditLog, cleanupAuditLogs } from '../api/credential';

interface AuditLogEntry {
  timestamp: number;
  operation: string;
  host: string;
  username: string;
  success: boolean;
  notes?: string;
}

const logs = ref<AuditLogEntry[]>([]);
const loading = ref(false);
const exporting = ref(false);
const error = ref<string | null>(null);

// Filters
const filterOperation = ref('');
const filterSuccess = ref('');
const filterHost = ref('');

// Pagination
const currentPage = ref(1);
const pageSize = 50;

// Cleanup
const showCleanupDialog = ref(false);
const retentionDays = ref(30);

const filteredLogs = computed(() => {
  return logs.value.filter(log => {
    if (filterOperation.value && log.operation !== filterOperation.value) {
      return false;
    }
    if (filterSuccess.value !== '' && log.success.toString() !== filterSuccess.value) {
      return false;
    }
    if (filterHost.value && !log.host.toLowerCase().includes(filterHost.value.toLowerCase())) {
      return false;
    }
    return true;
  });
});

const totalPages = computed(() => {
  return Math.ceil(filteredLogs.value.length / pageSize);
});

const paginatedLogs = computed(() => {
  const start = (currentPage.value - 1) * pageSize;
  const end = start + pageSize;
  return filteredLogs.value.slice(start, end);
});

const formatTimestamp = (timestamp: number) => {
  return new Date(timestamp * 1000).toLocaleString('zh-CN', {
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit',
  });
};

const translateOperation = (operation: string): string => {
  const translations: Record<string, string> = {
    'Add': '添加',
    'Get': '获取',
    'Update': '更新',
    'Delete': '删除',
    'Unlock': '解锁',
  };
  return translations[operation] || operation;
};

const getOperationBadgeClass = (operation: string): string => {
  const classes: Record<string, string> = {
    'Add': 'badge-primary',
    'Get': 'badge-info',
    'Update': 'badge-warning',
    'Delete': 'badge-error',
    'Unlock': 'badge-success',
  };
  return classes[operation] || 'badge-ghost';
};

const refreshLogs = async () => {
  loading.value = true;
  error.value = null;
  
  try {
    const logJson = await exportAuditLog();
    const parsed = JSON.parse(logJson);
    
    if (Array.isArray(parsed)) {
      logs.value = parsed;
    } else {
      throw new Error('Invalid audit log format');
    }
  } catch (err: any) {
    error.value = `加载失败: ${err.message || err}`;
    console.error('Failed to load audit logs:', err);
  } finally {
    loading.value = false;
  }
};

const exportLogs = async () => {
  exporting.value = true;
  error.value = null;
  
  try {
    const logJson = await exportAuditLog();
    
    // Create download
    const blob = new Blob([logJson], { type: 'application/json' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `audit-log-${Date.now()}.json`;
    a.click();
    URL.revokeObjectURL(url);
  } catch (err: any) {
    error.value = `导出失败: ${err.message || err}`;
    console.error('Failed to export audit logs:', err);
  } finally {
    exporting.value = false;
  }
};

const handleCleanup = async () => {
  showCleanupDialog.value = false;
  
  if (!retentionDays.value || retentionDays.value < 1) {
    error.value = '保留天数必须大于 0';
    return;
  }

  loading.value = true;
  error.value = null;
  
  try {
    const removedCount = await cleanupAuditLogs(retentionDays.value);
    
    // Refresh logs after cleanup
    await refreshLogs();
    
    // Show success message (you could use a toast notification here)
    alert(`成功清理 ${removedCount} 条旧日志`);
  } catch (err: any) {
    error.value = `清理失败: ${err.message || err}`;
    console.error('Failed to cleanup audit logs:', err);
  } finally {
    loading.value = false;
  }
};

onMounted(() => {
  refreshLogs();
});
</script>
