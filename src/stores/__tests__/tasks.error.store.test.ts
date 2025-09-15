import { describe, it, beforeEach, expect } from "vitest";
import { setActivePinia, createPinia } from "pinia";
import { useTasksStore } from "../tasks";

describe("stores/tasks lastError", () => {
  beforeEach(() => setActivePinia(createPinia()));

  it("setLastError 接受 retriedTimes（camelCase）", () => {
    const s = useTasksStore();
    s.setLastError("t1", { category: "Network", message: "timeout", retriedTimes: 2 });
    expect(s.lastErrorById["t1"]).toMatchObject({ category: "Network", message: "timeout", retriedTimes: 2 });
  });

  it("setLastError 接受 retried_times（snake_case）", () => {
    const s = useTasksStore();
    s.setLastError("t2", { category: "Protocol", message: "502", retried_times: 1 } as any);
    expect(s.lastErrorById["t2"]).toMatchObject({ category: "Protocol", message: "502", retriedTimes: 1 });
  });
});
