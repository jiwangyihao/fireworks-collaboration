import { describe, it, expect, beforeEach } from "vitest";
import { setActivePinia, createPinia } from "pinia";
import { useTasksStore, type TaskItem } from "../tasks";

describe("stores/tasks", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
  });

  it("upsert: 新任务应插入到队列头部", () => {
    const store = useTasksStore();
    const a: TaskItem = {
      id: "1",
      kind: "GitClone",
      state: "pending",
      createdAt: 1,
    };
    const b: TaskItem = {
      id: "2",
      kind: "Unknown",
      state: "running",
      createdAt: 2,
    };

    store.upsert(a);
    store.upsert(b);

    expect(store.items.map((t) => t.id)).toEqual(["2", "1"]);
  });

  it("upsert: 已存在的任务应被覆盖且位置不变", () => {
    const store = useTasksStore();
    store.upsert({ id: "x", kind: "Unknown", state: "pending", createdAt: 1 });
    store.upsert({ id: "y", kind: "HttpFake", state: "pending", createdAt: 2 });

    // 覆盖 x，不应改变相对顺序（y 仍在前）
    store.upsert({
      id: "x",
      kind: "Unknown",
      state: "completed",
      createdAt: 3,
    });

    expect(store.items.map((t) => `${t.id}:${t.state}`)).toEqual([
      "y:pending",
      "x:completed",
    ]);
  });

  it("remove: 移除指定 id 任务", () => {
    const store = useTasksStore();
    store.upsert({ id: "1", kind: "Unknown", state: "pending", createdAt: 1 });
    store.upsert({ id: "2", kind: "Unknown", state: "pending", createdAt: 2 });

    store.remove("1");
    expect(store.items.map((t) => t.id)).toEqual(["2"]);

    store.remove("2");
    expect(store.items).toHaveLength(0);
  });

  it("updateProgress: 进度应被限制在 0-100 并保留已有度量", () => {
    const store = useTasksStore();
    store.updateProgress({ taskId: "a", percent: 33, phase: "cloning", objects: 10 });
    expect(store.progressById["a"]).toEqual({ percent: 33, phase: "cloning", objects: 10 });

    // 更新时若未提供 objects，应保留旧值，percent 超过 100 应被截断
    store.updateProgress({ taskId: "a", percent: 150, phase: "finishing" });
    expect(store.progressById["a"]).toEqual({ percent: 100, phase: "finishing", objects: 10 });

    // 负值应提升为 0
    store.updateProgress({ taskId: "b", percent: -5 });
    expect(store.progressById["b"]).toEqual({ percent: 0 });
  });

  it("updateProgress: 应向下取整百分比并保留字节/总量指标", () => {
    const store = useTasksStore();
    store.updateProgress({ taskId: "p", percent: 42.9, bytes: 10_240, total_hint: 20_480 });

    expect(store.progressById["p"]).toEqual({ percent: 42, bytes: 10_240, total_hint: 20_480 });

    // 再次更新仅提供百分比，bytes/total_hint 应沿用旧值
    store.updateProgress({ taskId: "p", percent: 88.4 });

    expect(store.progressById["p"]).toEqual({ percent: 88, bytes: 10_240, total_hint: 20_480 });
  });

  it("setLastError: 应兼容 retriedTimes/retried_times", () => {
    const store = useTasksStore();
    store.setLastError("task1", { category: "Network", message: "boom", retried_times: 2 });
    expect(store.lastErrorById["task1"]).toEqual({ category: "Network", message: "boom", retriedTimes: 2, code: undefined });

    // camelCase 输入应覆盖并保留 code
    store.setLastError("task1", { category: "Network", message: "still boom", retriedTimes: 5, code: "EPIPE" });
    expect(store.lastErrorById["task1"]).toEqual({ category: "Network", message: "still boom", retriedTimes: 5, code: "EPIPE" });
  });
});
