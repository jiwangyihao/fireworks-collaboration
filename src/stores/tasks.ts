import { defineStore } from "pinia";

export type TaskState =
  | "pending"
  | "running"
  | "completed"
  | "failed"
  | "canceled";
export type TaskKind = "GitClone" | "GitFetch" | "GitPush" | "GitInit" | "GitAdd" | "HttpFake" | "Unknown";

export interface TaskItem {
  id: string;
  kind: TaskKind;
  state: TaskState;
  createdAt: number;
}

export const useTasksStore = defineStore("tasks", {
  state: () => ({
    items: [] as TaskItem[],
    // 进度按任务聚合，percent: 0-100，可选 objects/bytes
    progressById: {} as Record<string, { percent: number; phase?: string; objects?: number; bytes?: number; total_hint?: number }>,
    // MP1.5: 记录最近一次错误（标准化 error 事件）
  lastErrorById: {} as Record<string, { category: string; message: string; retriedTimes?: number }>,
  }),
  actions: {
    upsert(task: TaskItem) {
      const i = this.items.findIndex((t) => t.id === task.id);
      if (i >= 0) this.items[i] = task;
      else this.items.unshift(task);
    },
    remove(id: string) {
      this.items = this.items.filter((t) => t.id !== id);
    },
    updateProgress(payload: { taskId: string; percent: number; phase?: string; objects?: number; bytes?: number; total_hint?: number }) {
      const { taskId, percent, phase, objects, bytes, total_hint } = payload;
      const prev = this.progressById[taskId] ?? { percent: 0 };
      this.progressById[taskId] = {
        ...prev,
        percent: Math.max(0, Math.min(100, Math.floor(percent))),
        phase,
        objects: objects ?? prev.objects,
        bytes: bytes ?? prev.bytes,
        total_hint: total_hint ?? prev.total_hint,
      };
    },
    setLastError(taskId: string, err: { category: string; message: string; retried_times?: number; retriedTimes?: number }) {
      const retriedTimes = (err as any).retriedTimes ?? (err as any).retried_times;
      this.lastErrorById[taskId] = { category: err.category, message: err.message, retriedTimes };
    },
  },
});
