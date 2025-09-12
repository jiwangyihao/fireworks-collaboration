import { invoke } from "./tauri";
import { listen } from "@tauri-apps/api/event";
import { useTasksStore } from "../stores/tasks";

export interface TaskStateEventPayload {
  taskId: string;
  kind: string;
  state: string;
  createdAt: number;
}
export interface TaskProgressEventPayload {
  taskId: string;
  kind: string;
  phase: string;
  percent: number;
}

let unsubs: (() => void)[] = [];

export async function initTaskEvents() {
  const store = useTasksStore();
  // state events
  const un1 = await listen<TaskStateEventPayload>("task://state", (e) => {
    const p = e.payload;
    store.upsert({
      id: p.taskId,
      kind: (p.kind as any) ?? "Unknown",
      state: (p.state as any) ?? "pending",
      createdAt: p.createdAt ?? Date.now(),
    });
  });
  const un2 = await listen<TaskProgressEventPayload>("task://progress", () => {
    // 可在后续扩展进度条存储
  });
  unsubs.push(un1, un2);
}

export function disposeTaskEvents() {
  unsubs.forEach((u) => u());
  unsubs = [];
}

export async function listTasks() {
  return invoke<any[]>("task_list");
}

export async function startSleepTask(ms: number) {
  return invoke<string>("task_start_sleep", { ms });
}

export async function cancelTask(id: string) {
  return invoke<boolean>("task_cancel", { id });
}
