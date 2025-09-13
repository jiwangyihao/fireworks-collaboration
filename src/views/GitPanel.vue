<template>
  <div class="p-4 pt-16 space-y-4">
  <h2 class="text-xl font-bold">Git 面板（Clone / Fetch）</h2>

    <div class="card bg-base-100 shadow-sm">
      <div class="card-body gap-3">
        <div class="flex gap-2 items-center">
          <input v-model="repo" class="input input-bordered input-sm flex-1" placeholder="https://github.com/rust-lang/log" />
          <input v-model="dest" class="input input-bordered input-sm flex-1" placeholder="C:/tmp/log" />
          <select v-model="preset" class="select select-bordered select-sm">
            <option value="remote">按远程配置</option>
            <option value="branches">分支（refs/heads/*）</option>
            <option value="branches+tags">分支+标签</option>
            <option value="tags">仅标签</option>
          </select>
          <button class="btn btn-primary btn-sm" :disabled="!repo || !dest || working" @click="startClone">Clone</button>
          <!-- Fetch 允许 repo 留空 -> 使用默认远程 -->
          <button class="btn btn-secondary btn-sm" :disabled="!dest || working" @click="startFetch">Fetch</button>
        </div>
        <div class="text-xs opacity-70">建议使用绝对路径，例如 C:/tmp/project；Fetch 时 repo 可留空表示默认远程；如远程缺少 refspec，可用上方预设快速选择</div>
      </div>
    </div>

    <div class="card bg-base-100 shadow-sm">
      <div class="card-body">
        <h3 class="font-semibold mb-2">最近任务</h3>
        <table class="table table-zebra text-sm">
          <thead>
            <tr><th>ID</th><th>类型</th><th>状态</th><th>创建时间</th><th>进度</th><th>操作</th></tr>
          </thead>
          <tbody>
            <tr v-for="t in tasks.items" :key="t.id">
              <td class="font-mono">{{ t.id.slice(0,8) }}</td>
              <td>{{ t.kind }}</td>
              <td :class="stateClass(t.state)">{{ t.state }}</td>
              <td>{{ new Date(t.createdAt).toLocaleTimeString() }}</td>
              <td class="w-64">
                <progress class="progress progress-primary w-56" :value="progressOf(t.id).percent" max="100" />
                <div class="text-xs opacity-70">
                  {{ progressOf(t.id).phase || '-' }} {{ progressOf(t.id).percent }}%
                  <template v-if="progressOf(t.id).objects || progressOf(t.id).bytes">
                    · objs: {{ progressOf(t.id).objects ?? '-' }}
                    · bytes: {{ prettyBytes(progressOf(t.id).bytes) }}
                  </template>
                </div>
              </td>
              <td>
                <button class="btn btn-xs" v-if="t.state==='running' || t.state==='pending'" @click="cancel(t.id)">取消</button>
              </td>
            </tr>
            <tr v-if="tasks.items.length===0"><td colspan="6" class="text-center opacity-60">暂无任务</td></tr>
          </tbody>
        </table>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue';
import { useTasksStore } from '../stores/tasks';
import { startGitClone, startGitFetch, cancelTask, listTasks } from '../api/tasks';

const repo = ref('https://github.com/rust-lang/log');
const dest = ref('C:/tmp/log');
const preset = ref<'remote'|'branches'|'branches+tags'|'tags'>('remote');
const working = ref(false);
const tasks = useTasksStore();

function stateClass(s: string) {
  return {
    'text-blue-600': s==='running',
    'text-green-700': s==='completed',
    'text-red-600': s==='failed',
    'text-gray-500': s==='canceled'
  };
}

function progressOf(id: string) {
  return tasks.progressById[id] || { percent: 0 };
}

function prettyBytes(n?: number) {
  if (!n || n <= 0) return '-';
  const kb = 1024, mb = kb * 1024, gb = mb * 1024;
  if (n >= gb) return (n / gb).toFixed(2) + ' GiB';
  if (n >= mb) return (n / mb).toFixed(2) + ' MiB';
  if (n >= kb) return (n / kb).toFixed(2) + ' KiB';
  return n + ' B';
}

async function startClone() {
  working.value = true;
  try {
    await startGitClone(repo.value.trim(), dest.value.trim());
    // 刷新历史（事件也会推送）
    await listTasks().then((arr:any[])=>{
      if (Array.isArray(arr)) {
        for (const s of arr) {
          tasks.upsert({ id: s.id, kind: s.kind ?? 'Unknown', state: s.state ?? 'pending', createdAt: s.createdAt ?? Date.now() });
        }
      }
    });
  } catch (e) {
    console.error(e);
  } finally {
    working.value = false;
  }
}

async function startFetch() {
  working.value = true;
  try {
    const selected = preset.value;
    const extra = selected === 'remote' ? undefined : selected;
    await startGitFetch(repo.value.trim(), dest.value.trim(), extra);
    await listTasks().then((arr:any[])=>{
      if (Array.isArray(arr)) {
        for (const s of arr) {
          tasks.upsert({ id: s.id, kind: s.kind ?? 'Unknown', state: s.state ?? 'pending', createdAt: s.createdAt ?? Date.now() });
        }
      }
    });
  } catch (e) {
    console.error(e);
  } finally {
    working.value = false;
  }
}

async function cancel(id: string){
  try { await cancelTask(id); } catch(e){ console.error(e); }
}
</script>

<style scoped>
</style>
