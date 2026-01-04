<script setup lang="ts">
import { ref, onMounted, onUnmounted } from "vue";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useToastStore } from "../../stores/toast";
import { startDevServer, stopDevServer } from "../../api/vitepress";
import BaseIcon from "../BaseIcon.vue";

const props = defineProps<{
  projectPath: string;
}>();

const toastStore = useToastStore();

const status = ref<"stopped" | "starting" | "running" | "error">("stopped");
const url = ref<string | null>(null);
const processId = ref<number | null>(null);
const isLoading = ref(false);
const logs = ref<string[]>([]);
const showLogs = ref(false);

let unlistenOutput: UnlistenFn | null = null;

// Start Dev Server
async function handleStart() {
  if (isLoading.value) return;

  isLoading.value = true;
  status.value = "starting";
  logs.value = [];

  try {
    const info = await startDevServer(props.projectPath);
    url.value = info.url;
    processId.value = info.processId;
    status.value = "running";
    toastStore.success(`Dev Server started at ${info.url}`);
  } catch (e) {
    status.value = "error";
    toastStore.error(`Failed to start Dev Server: ${e}`);
    logs.value.push(`Error: ${e}`);
  } finally {
    isLoading.value = false;
  }
}

// Stop Dev Server
async function handleStop() {
  if (!processId.value) return;

  isLoading.value = true;
  try {
    await stopDevServer(processId.value, props.projectPath);
    status.value = "stopped";
    url.value = null;
    processId.value = null;
    toastStore.success("Dev Server stopped");
  } catch (e) {
    toastStore.error(`Failed to stop Dev Server: ${e}`);
  } finally {
    isLoading.value = false;
  }
}

function toggleLogs() {
  showLogs.value = !showLogs.value;
}

function openUrl() {
  if (url.value) {
    import("@tauri-apps/plugin-opener").then((mod: any) => {
      if (mod.open) mod.open(url.value!);
    });
  }
}

onMounted(async () => {
  unlistenOutput = await listen<string>(
    "vitepress://dev-server-output",
    (event) => {
      logs.value.push(event.payload);
      if (logs.value.length > 100) logs.value.shift(); // Keep last 100 lines
    }
  );
});

onUnmounted(() => {
  if (unlistenOutput) unlistenOutput();
  // Optional: Stop server on component destroy?
  // Probably better to keep running if user navigates away, but for E1 simple case maybe fine.
  // Actually, if we navigate away, we lose control of PID unless stored in global store.
  // We didn't implement global store for dev server yet.
  // So for now, maybe we should stop it or just warn.
  // Let's stop it to be safe for E1.
  if (processId.value) {
    stopDevServer(processId.value, props.projectPath);
  }
});
</script>

<template>
  <div class="flex items-center gap-2">
    <!-- Status Indicator -->
    <div
      class="flex items-center gap-2 text-xs px-2 py-1 rounded-full bg-base-200"
      :class="{
        'text-success': status === 'running',
        'text-warning': status === 'starting',
        'text-base-content/50': status === 'stopped',
        'text-error': status === 'error',
      }"
    >
      <span class="w-2 h-2 rounded-full bg-current"></span>
      <span>{{
        status === "running"
          ? "Running"
          : status === "starting"
            ? "Starting"
            : status === "stopped"
              ? "Stopped"
              : "Error"
      }}</span>
    </div>

    <!-- Controls -->
    <div class="join">
      <button
        v-if="status === 'stopped' || status === 'error'"
        class="btn btn-xs btn-primary join-item"
        :disabled="isLoading"
        @click="handleStart"
      >
        <BaseIcon icon="ph--play" size="sm" />
        Start
      </button>

      <button
        v-if="status === 'running' || status === 'starting'"
        class="btn btn-xs btn-error join-item"
        :disabled="isLoading"
        @click="handleStop"
      >
        <BaseIcon icon="ph--stop" size="sm" />
        Stop
      </button>

      <button
        v-if="url"
        class="btn btn-xs btn-ghost join-item"
        @click="openUrl"
        title="Open in Browser"
      >
        <BaseIcon icon="ph--arrow-square-out" size="sm" />
      </button>

      <button
        class="btn btn-xs btn-ghost join-item"
        :class="{ 'text-primary': showLogs }"
        @click="toggleLogs"
        title="View Logs"
      >
        <BaseIcon icon="ph--terminal" size="sm" />
      </button>
    </div>

    <!-- Logs Dropdown/Drawer -->
    <div
      v-if="showLogs"
      class="fixed bottom-12 right-4 w-96 max-h-64 bg-base-300 rounded-box shadow-xl flex flex-col z-50 overflow-hidden text-xs font-mono"
    >
      <div
        class="bg-base-content/10 px-2 py-1 flex justify-between items-center"
      >
        <span>Server Logs</span>
        <button
          @click="showLogs = false"
          class="btn btn-ghost btn-xs btn-square"
        >
          <BaseIcon icon="ph--x" />
        </button>
      </div>
      <div class="p-2 overflow-y-auto flex-1">
        <div v-for="(line, i) in logs" :key="i" class="whitespace-pre-wrap">
          {{ line }}
        </div>
        <div v-if="logs.length === 0" class="text-base-content/30 italic">
          No logs yet
        </div>
      </div>
    </div>
  </div>
</template>
