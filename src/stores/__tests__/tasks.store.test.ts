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
});
