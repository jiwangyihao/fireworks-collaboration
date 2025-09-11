import { defineStore } from "pinia";

export type TaskState =
  | "pending"
  | "running"
  | "completed"
  | "failed"
  | "canceled";
export type TaskKind = "GitClone" | "HttpFake" | "Unknown";

export interface TaskItem {
  id: string;
  kind: TaskKind;
  state: TaskState;
  createdAt: number;
}

export const useTasksStore = defineStore("tasks", {
  state: () => ({ items: [] as TaskItem[] }),
  actions: {
    upsert(task: TaskItem) {
      const i = this.items.findIndex((t) => t.id === task.id);
      if (i >= 0) this.items[i] = task;
      else this.items.unshift(task);
    },
    remove(id: string) {
      this.items = this.items.filter((t) => t.id !== id);
    },
  },
});
