<!--
  @deprecated This component is deprecated and not currently in use.
  It was a debug/development component for testing task management.
  Consider removing in future cleanup.
-->
<script setup lang="ts">
import { onMounted, ref } from "vue";
import { useTasksStore } from "../../stores/tasks";
import { listTasks, startSleepTask, cancelTask } from "../../api/tasks";

const store = useTasksStore();
const loading = ref(false);
const sleepMs = ref(3000);

async function refresh() {
  loading.value = true;
  try {
    const items = await listTasks();
    if (Array.isArray(items)) {
      for (const s of items) {
        store.upsert({
          id: s.id,
          kind: s.kind ?? "Unknown",
          state: s.state ?? "pending",
          createdAt: s.createdAt ?? Date.now(),
        });
      }
    }
  } finally {
    loading.value = false;
  }
}

async function startSleep() {
  await startSleepTask(sleepMs.value);
  // 刷新列表（事件会更新，但为保证初始可见）
  refresh();
}

async function cancel(id: string) {
  await cancelTask(id);
}

onMounted(refresh);
</script>

<template>
  <div class="task-list">
    <div class="toolbar">
      <input type="number" v-model.number="sleepMs" min="200" step="200" />
      <button @click="startSleep">启动 Sleep 任务</button>
      <button @click="refresh" :disabled="loading">
        {{ loading ? "刷新中" : "刷新" }}
      </button>
    </div>
    <table>
      <thead>
        <tr>
          <th>ID</th>
          <th>类型</th>
          <th>状态</th>
          <th>创建时间</th>
          <th></th>
        </tr>
      </thead>
      <tbody>
        <tr v-for="t in store.items" :key="t.id">
          <td class="mono">{{ t.id.slice(0, 8) }}</td>
          <td>{{ t.kind }}</td>
          <td :class="['state', t.state]">{{ t.state }}</td>
          <td>{{ new Date(t.createdAt).toLocaleTimeString() }}</td>
          <td>
            <button
              v-if="t.state === 'running' || t.state === 'pending'"
              @click="cancel(t.id)"
            >
              取消
            </button>
          </td>
        </tr>
        <tr v-if="!store.items.length">
          <td colspan="5" style="text-align: center; opacity: 0.6">无任务</td>
        </tr>
      </tbody>
    </table>
  </div>
</template>

<style scoped>
.task-list {
  margin-top: 1rem;
}
.toolbar {
  display: flex;
  gap: 0.5rem;
  align-items: center;
  margin-bottom: 0.5rem;
}
.mono {
  font-family: monospace;
}
table {
  width: 100%;
  border-collapse: collapse;
}
th,
td {
  border: 1px solid #ddd;
  padding: 0.35rem 0.5rem;
  font-size: 12px;
}
th {
  background: #f5f5f5;
}
.state.running {
  color: #1a73e8;
}
.state.completed {
  color: #2e7d32;
}
.state.failed {
  color: #d32f2f;
}
.state.canceled {
  color: #757575;
}
</style>
