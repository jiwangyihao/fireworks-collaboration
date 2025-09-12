import { defineStore } from "pinia";

export interface LogItem {
  id: string;
  level: "info" | "warn" | "error";
  message: string;
  time: number;
}

export const useLogsStore = defineStore("logs", {
  state: () => ({ items: [] as LogItem[] }),
  actions: {
    push(level: LogItem["level"], message: string) {
      this.items.unshift({ id: Math.random().toString(36).slice(2), level, message, time: Date.now() });
      if (this.items.length > 50) this.items.pop();
    },
    clear() { this.items = []; },
  },
});
