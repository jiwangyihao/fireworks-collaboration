import { describe, it, expect, beforeEach } from "vitest";
import { setActivePinia, createPinia } from "pinia";
import { useTasksStore } from "../tasks";

describe("stores/tasks progress", () => {
  beforeEach(() => setActivePinia(createPinia()));

  it("updateProgress: 新建并夹取 0-100", () => {
    const s = useTasksStore();
    s.updateProgress({ taskId: "a", percent: -5 });
    expect(s.progressById["a"].percent).toBe(0);
    s.updateProgress({ taskId: "a", percent: 250 });
    expect(s.progressById["a"].percent).toBe(100);
  });

  it("updateProgress: 保留未提供字段，覆盖提供字段", () => {
    const s = useTasksStore();
    s.updateProgress({ taskId: "b", percent: 10, objects: 5, bytes: 100 });
    s.updateProgress({ taskId: "b", percent: 20, objects: 8 });
    expect(s.progressById["b"]).toMatchObject({ percent: 20, objects: 8, bytes: 100 });
  });
});
