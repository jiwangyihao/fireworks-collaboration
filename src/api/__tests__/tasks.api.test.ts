import { describe, it, expect, vi, beforeEach } from "vitest";
import { setActivePinia, createPinia } from "pinia";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

// mock event listen
const listenMock = vi.fn();
vi.mock("@tauri-apps/api/event", () => ({
  listen: (...args: any[]) => listenMock(...args),
}));

import { invoke } from "@tauri-apps/api/core";
import {
  initTaskEvents,
  disposeTaskEvents,
  startSleepTask,
  listTasks,
  cancelTask,
} from "../tasks";
import { useTasksStore } from "../../stores/tasks";

interface ListenCall {
  evt: string;
  cb: (payload: any) => void;
}

describe("api/tasks integration (mocked tauri)", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    (invoke as any).mockReset();
    listenMock.mockReset();
    // each listen returns an unsubscribe fn
    listenMock.mockImplementation((evt: string, cb: any) => {
      const wrapper = { evt, cb } as ListenCall;
      (listenMock as any)._calls = ((listenMock as any)._calls || []).concat(
        wrapper,
      );
      return Promise.resolve(() => {
        wrapper.cb = () => {};
      });
    });
  });

  it("startSleepTask 调用 task_start_sleep", async () => {
    (invoke as any).mockResolvedValueOnce("task-id-1");
    const id = await startSleepTask(500);
    expect(invoke).toHaveBeenCalledWith("task_start_sleep", { ms: 500 });
    expect(id).toBe("task-id-1");
  });

  it("listTasks 调用 task_list", async () => {
    (invoke as any).mockResolvedValueOnce([
      { id: "x", kind: "Sleep", state: "completed", createdAt: Date.now() },
    ]);
    const list = await listTasks();
    expect(invoke).toHaveBeenCalledWith("task_list", undefined);
    expect(list).toHaveLength(1);
  });

  it("cancelTask 调用 task_cancel", async () => {
    (invoke as any).mockResolvedValueOnce(true);
    const ok = await cancelTask("abc");
    expect(invoke).toHaveBeenCalledWith("task_cancel", { id: "abc" });
    expect(ok).toBe(true);
  });

  it("initTaskEvents 监听并写入 store", async () => {
    const store = useTasksStore();
    await initTaskEvents();
    // 模拟 state 事件
    const calls: ListenCall[] = (listenMock as any)._calls || [];
    const stateListener = calls.find((c) => c.evt === "task://state");
    expect(stateListener).toBeTruthy();
    stateListener!.cb({
      payload: {
        taskId: "t1",
        kind: "Sleep",
        state: "running",
        createdAt: 111,
      },
    });
    expect(store.items[0]).toMatchObject({ id: "t1", state: "running" });
  });

  it("disposeTaskEvents 调用后 state listener 不再生效", async () => {
    const store = useTasksStore();
    await initTaskEvents();
    disposeTaskEvents();
    const calls: ListenCall[] = (listenMock as any)._calls || [];
    const stateListener = calls.find((c) => c.evt === "task://state");
    // 卸载后我们模拟调用也不会影响（cb 被替换为 no-op via unsubscribe）
    if (stateListener)
      stateListener.cb({
        payload: {
          taskId: "t2",
          kind: "Sleep",
          state: "running",
          createdAt: 222,
        },
      });
    expect(store.items.find((t) => t.id === "t2")).toBeUndefined();
  });
});
