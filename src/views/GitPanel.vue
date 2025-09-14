<template>
  <div class="p-4 pt-16 space-y-4">
  <h2 class="text-xl font-bold">Git 面板（Clone / Fetch / Push）</h2>

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
      <div class="card-body gap-3">
        <div class="flex gap-2 items-center">
          <input v-model="pushDest" class="input input-bordered input-sm flex-1" placeholder="C:/tmp/repo（本地仓库路径）" />
          <input v-model="remote" class="input input-bordered input-sm w-36" placeholder="origin" />
          <input v-model="refspec" class="input input-bordered input-sm flex-1" placeholder="refs/heads/main:refs/heads/main" />
          <button class="btn btn-accent btn-sm" :disabled="!pushDest || working" @click="startPush">Push</button>
        </div>
        <div class="flex gap-2 items-center">
          <input v-model="username" class="input input-bordered input-sm w-56" placeholder="用户名（仅 token 可填 x-access-token）" />
          <input v-model="password" type="password" class="input input-bordered input-sm w-72" placeholder="密码/令牌（可选）" />
        </div>
        <div class="text-xs opacity-70">Push 会使用 HTTPS 基础认证；如仅使用 GitHub Token，请将用户名设为 x-access-token，密码填入 token。</div>
      </div>
    </div>

    <div class="card bg-base-100 shadow-sm">
      <div class="card-body gap-3">
        <h3 class="font-semibold">SNI / TLS 策略</h3>
        <div class="grid grid-cols-2 gap-2 items-center">
          <label class="label cursor-pointer gap-2 col-span-2">
            <span>跳过证书验证（不安全，仅用于联通性验证）</span>
            <input type="checkbox" v-model="insecureSkipVerify" class="checkbox checkbox-sm" @change="applyTlsToggle" />
          </label>
          <label class="label cursor-pointer gap-2 col-span-2">
            <span>跳过 SAN 白名单校验</span>
            <input type="checkbox" v-model="skipSanWhitelist" class="checkbox checkbox-sm" @change="applyTlsToggle" />
          </label>
          <label class="label cursor-pointer gap-2"><span>启用 Fake SNI</span><input type="checkbox" v-model="fakeSniEnabled" class="checkbox checkbox-sm" /></label>
          <label class="label cursor-pointer gap-2"><span>403 时自动轮换 SNI</span><input type="checkbox" v-model="sniRotateOn403" class="checkbox checkbox-sm" /></label>
          <textarea v-model="fakeSniHostsText" class="textarea textarea-bordered w-full col-span-2" rows="3" placeholder="多个候选域名：每行一个或用逗号分隔"></textarea>
        </div>
        <div class="flex items-center gap-2">
          <button class="btn btn-sm" @click="applyHttpStrategy">保存策略</button>
          <div class="text-xs opacity-70">{{ policySummary }}</div>
        </div>
        <div class="text-xs opacity-70">当前任务的 SNI 状态会在下方“最近任务”的进度阶段列显示。</div>
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
import { ref, computed, onMounted } from 'vue';
import { useTasksStore } from '../stores/tasks';
import { startGitClone, startGitFetch, startGitPush, cancelTask, listTasks } from '../api/tasks';
import { getConfig, setConfig, type AppConfig } from '../api/config';
import { useLogsStore } from '../stores/logs';

const repo = ref('https://github.com/rust-lang/log');
const dest = ref('C:/tmp/log');
const preset = ref<'remote'|'branches'|'branches+tags'|'tags'>('remote');
const working = ref(false);
const tasks = useTasksStore();

// Push 相关输入
const pushDest = ref('C:/tmp/log');
const remote = ref('origin');
const refspec = ref('refs/heads/main:refs/heads/main');
const username = ref('');
const password = ref('');

// SNI/TLS 策略
const insecureSkipVerify = ref(false);
const skipSanWhitelist = ref(false);
const fakeSniEnabled = ref(true);
const fakeSniHostsText = ref('');
const sniRotateOn403 = ref(true);
const cfg = ref<AppConfig | null>(null);
const logs = useLogsStore();

onMounted(async () => {
  try {
    cfg.value = await getConfig();
  // TLS
  insecureSkipVerify.value = !!cfg.value.tls.insecureSkipVerify;
  skipSanWhitelist.value = !!(cfg.value.tls as any).skipSanWhitelist;
  // HTTP SNI
  fakeSniEnabled.value = !!cfg.value.http.fakeSniEnabled;
    const hosts = (cfg.value.http as any).fakeSniHosts as string[] | undefined;
    fakeSniHostsText.value = (hosts && hosts.length > 0) ? hosts.join('\n') : '';
    sniRotateOn403.value = (cfg.value.http as any).sniRotateOn403 ?? true;
  } catch (e) {
    // 读取失败忽略
  }
});

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

async function applyTlsToggle() {
  try {
    if (!cfg.value) cfg.value = await getConfig();
  cfg.value!.tls.insecureSkipVerify = !!insecureSkipVerify.value;
  (cfg.value!.tls as any).skipSanWhitelist = !!skipSanWhitelist.value;
    await setConfig(cfg.value!);
  } catch (e) {
    console.error('更新 TLS 配置失败', e);
    logs.push('error', `更新 TLS 配置失败: ${String(e)}`);
  }
}

async function applyHttpStrategy() {
  try {
    if (!cfg.value) cfg.value = await getConfig();
    cfg.value!.http.fakeSniEnabled = !!fakeSniEnabled.value;
    const raw = (fakeSniHostsText.value || '').split(/[\n,]/).map(s => s.trim()).filter(Boolean);
    const uniq = Array.from(new Set(raw));
    (cfg.value!.http as any).fakeSniHosts = uniq;
    (cfg.value!.http as any).sniRotateOn403 = !!sniRotateOn403.value;
    await setConfig(cfg.value!);
  } catch (e) {
    console.error('更新 HTTP 策略失败', e);
    logs.push('error', `更新 HTTP 策略失败: ${String(e)}`);
  }
}

const policySummary = computed(() => {
  const parts: string[] = [];
  parts.push(`insecureSkipVerify=${insecureSkipVerify.value ? 'on' : 'off'}`);
  parts.push(`fakeSni=${fakeSniEnabled.value ? 'on' : 'off'}`);
  const cnt = (fakeSniHostsText.value || '').split(/[\n,]/).map(s => s.trim()).filter(Boolean).length;
  parts.push(`candidates=${cnt}`);
  parts.push(`rotate403=${sniRotateOn403.value ? 'on' : 'off'}`);
  return parts.join(' · ');
});

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

async function startPush() {
  working.value = true;
  try {
    const rs = refspec.value.trim();
    const args: any = {
      dest: pushDest.value.trim(),
      remote: remote.value.trim() || undefined,
      refspecs: rs ? [rs] : undefined,
    };
    if (username.value.trim()) args.username = username.value.trim();
    if (password.value.trim()) args.password = password.value.trim();
    await startGitPush(args);
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
</script>

<style scoped>
</style>
