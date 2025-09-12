import { describe, it, expect, beforeEach, vi } from "vitest";
import { setActivePinia, createPinia } from "pinia";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const listenMock = vi.fn();
vi.mock("@tauri-apps/api/event", () => ({ listen: (...args: any[]) => listenMock(...args) }));

import { initTaskEvents } from "../tasks";
import { useTasksStore } from "../../stores/tasks";

interface ListenCall { evt: string; cb: (payload: any) => void }

describe("tasks events 进度处理", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    listenMock.mockReset();
    listenMock.mockImplementation((evt: string, cb: any) => {
      const wrapper = { evt, cb } as ListenCall;
      (listenMock as any)._calls = ((listenMock as any)._calls || []).concat(wrapper);
      return Promise.resolve(() => { wrapper.cb = () => {}; });
    });
  });

  it("触发 task://progress 应更新 progressById", async () => {
    const store = useTasksStore();
    await initTaskEvents();
    const calls: ListenCall[] = (listenMock as any)._calls || [];
    const prog = calls.find((c) => c.evt === "task://progress")!;
    prog.cb({ payload: { taskId: "p1", kind: "GitClone", phase: "Fetching", percent: 42, objects: 10, bytes: 1024 } });
    expect(store.progressById["p1"]).toMatchObject({ percent: 42, phase: "Fetching", objects: 10, bytes: 1024 });
  });
});
