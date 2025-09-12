// P0.2 任务事件测试：验证 state 事件写入/更新、progress 事件当前忽略、dispose 失效
import { describe, it, beforeEach, expect, vi } from "vitest";
import { setActivePinia, createPinia } from "pinia";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

// 监听 mock
const listenMock = vi.fn();
vi.mock("@tauri-apps/api/event", () => ({
  listen: (...args: any[]) => listenMock(...args),
}));

import { initTaskEvents, disposeTaskEvents } from "../tasks";
import { useTasksStore } from "../../stores/tasks";

interface ListenCall {
  evt: string;
  cb: (payload: any) => void;
}

describe("tasks events 基础行为", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    listenMock.mockReset();
    listenMock.mockImplementation((evt: string, cb: any) => {
      const wrapper = { evt, cb } as ListenCall;
      (listenMock as any)._calls = ((listenMock as any)._calls || []).concat(
        wrapper,
      );
      return Promise.resolve(() => {
        wrapper.cb = () => {}; // 解除后 no-op
      });
    });
  });

  it("state 事件应写入并可被后续同 id 更新", async () => {
    const store = useTasksStore();
    await initTaskEvents();
    const calls: ListenCall[] = (listenMock as any)._calls || [];
    const stateListener = calls.find((c) => c.evt === "task://state");
    expect(stateListener).toBeTruthy();

    // 初次 running
    stateListener!.cb({
      payload: {
        taskId: "t1",
        kind: "Sleep",
        state: "running",
        createdAt: 100,
      },
    });
    expect(store.items.length).toBe(1);
    expect(store.items[0]).toMatchObject({ id: "t1", state: "running" });

    // 更新为 completed
    stateListener!.cb({
      payload: {
        taskId: "t1",
        kind: "Sleep",
        state: "completed",
        createdAt: 100,
      },
    });
    expect(store.items.length).toBe(1); // upsert 覆盖
    expect(store.items[0]).toMatchObject({ id: "t1", state: "completed" });
  });

  it("progress 事件当前忽略，不应改变 store 数量", async () => {
    const store = useTasksStore();
    await initTaskEvents();
    const calls: ListenCall[] = (listenMock as any)._calls || [];
    const stateListener = calls.find((c) => c.evt === "task://state")!;
    const progListener = calls.find((c) => c.evt === "task://progress")!;

    stateListener.cb({
      payload: {
        taskId: "t2",
        kind: "Sleep",
        state: "running",
        createdAt: 200,
      },
    });
    const before = store.items.length;
    progListener.cb({
      payload: {
        taskId: "t2",
        kind: "Sleep",
        phase: "Half",
        percent: 50,
      },
    });
    expect(store.items.length).toBe(before); // 未新增/删除
  });

  it("dispose 后 state 事件不再生效", async () => {
    const store = useTasksStore();
    await initTaskEvents();
    const calls: ListenCall[] = (listenMock as any)._calls || [];
    const stateListener = calls.find((c) => c.evt === "task://state")!;
    disposeTaskEvents();
    // 尝试再推送一个新 id
    stateListener.cb({
      payload: {
        taskId: "tX",
        kind: "Sleep",
        state: "running",
        createdAt: 300,
      },
    });
    expect(store.items.find((i) => i.id === "tX")).toBeUndefined();
  });
});
