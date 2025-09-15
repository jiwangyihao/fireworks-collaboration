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
import { useLogsStore } from "../../stores/logs";

interface ListenCall { evt: string; cb: (payload: any) => void }

describe("task://error 事件", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    listenMock.mockReset();
    listenMock.mockImplementation((evt: string, cb: any) => {
      const wrapper = { evt, cb } as ListenCall;
      (listenMock as any)._calls = ((listenMock as any)._calls || []).concat(wrapper);
      return Promise.resolve(() => { wrapper.cb = () => {}; });
    });
  });

  it("应记录最近错误并写入日志", async () => {
    const tasks = useTasksStore();
    const logs = useLogsStore();
    await initTaskEvents();
    const calls: ListenCall[] = (listenMock as any)._calls || [];
    const errListener = calls.find((c) => c.evt === "task://error")!;
    expect(errListener).toBeTruthy();

  errListener.cb({ payload: { taskId: "e1", kind: "GitClone", category: "Network", message: "timeout", retried_times: 2 } });
  expect(tasks.lastErrorById["e1"]).toMatchObject({ category: "Network", message: "timeout", retriedTimes: 2 });
    expect(logs.items[0].message).toContain("GitClone Network: timeout");

    disposeTaskEvents();
  });
});
