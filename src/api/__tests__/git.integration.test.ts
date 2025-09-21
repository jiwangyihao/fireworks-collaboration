import { describe, it, expect, vi, beforeEach } from "vitest";
import { setActivePinia, createPinia } from "pinia";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

// 监听 mock
const listenMock = vi.fn();
vi.mock("@tauri-apps/api/event", () => ({
  listen: (...args: any[]) => listenMock(...args),
}));

import { invoke } from "@tauri-apps/api/core";
import { initTaskEvents, disposeTaskEvents, startGitClone, startGitFetch, cancelTask } from "../tasks";
import { useTasksStore } from "../../stores/tasks";

interface ListenCall {
  evt: string;
  cb: (payload: any) => void;
}

describe("api/git clone 集成（mocked tauri）", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    (invoke as any).mockReset();
    listenMock.mockReset();
    // 清空跨用例残留的监听记录，避免找到上一次用例的回调
    (listenMock as any)._calls = [];
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

  it("startGitClone + events 应驱动 store 状态变化", async () => {
    const store = useTasksStore();
    await initTaskEvents();

    (invoke as any).mockResolvedValueOnce("git-t1");
    const id = await startGitClone(
      "https://github.com/rust-lang/log",
      "C:/tmp/log",
    );
    expect(id).toBe("git-t1");
    expect(invoke).toHaveBeenCalledWith("git_clone", {
      repo: "https://github.com/rust-lang/log",
      dest: "C:/tmp/log",
    });

    const calls: ListenCall[] = (listenMock as any)._calls || [];
    const stateListener = calls.find((c) => c.evt === "task://state")!;
    const progListener = calls.find((c) => c.evt === "task://progress")!;

    // running
    stateListener.cb({
      payload: {
        taskId: "git-t1",
        kind: "GitClone",
        state: "running",
        createdAt: 1,
      },
    });
    expect(store.items[0]).toMatchObject({ id: "git-t1", state: "running", kind: "GitClone" });

    // progress（包含可选字段，不影响 store 数量）
    const before = store.items.length;
    // 细粒度阶段：Negotiating -> Receiving -> Checkout -> Fetching（兼容旧起始）
    progListener.cb({ payload: { taskId: "git-t1", kind: "GitClone", phase: "Negotiating", percent: 0 } });
    await Promise.resolve();
    expect(store.progressById["git-t1"]).toMatchObject({ phase: "Negotiating" });

    progListener.cb({ payload: { taskId: "git-t1", kind: "GitClone", phase: "Receiving", percent: 10 } });
    await Promise.resolve();
    expect(store.progressById["git-t1"]).toMatchObject({ phase: "Receiving", percent: 10 });

    progListener.cb({ payload: { taskId: "git-t1", kind: "GitClone", phase: "Checkout", percent: 90 } });
    await Promise.resolve();
    expect(store.progressById["git-t1"]).toMatchObject({ phase: "Checkout", percent: 90 });

    // 继续旧的 Fetching 事件，保持向后兼容
    progListener.cb({ payload: { taskId: "git-t1", kind: "GitClone", phase: "Fetching", percent: 30, objects: 100, bytes: 123456, total_hint: 1000 } });
    expect(store.items.length).toBe(before);

    // completed 覆盖状态
    stateListener.cb({
      payload: {
        taskId: "git-t1",
        kind: "GitClone",
        state: "completed",
        createdAt: 1,
      },
    });
    expect(store.items[0]).toMatchObject({ id: "git-t1", state: "completed" });

    disposeTaskEvents();
  });

  it("取消流程：startGitClone 后可调用 cancelTask", async () => {
    (invoke as any).mockResolvedValueOnce("git-t2"); // start
    (invoke as any).mockResolvedValueOnce(true); // cancel
    const id = await startGitClone("https://example.com/repo.git", "C:/repo");
    const ok = await cancelTask(id);
    expect(invoke).toHaveBeenNthCalledWith(1, "git_clone", {
      repo: "https://example.com/repo.git",
      dest: "C:/repo",
    });
    expect(invoke).toHaveBeenNthCalledWith(2, "task_cancel", { id: "git-t2" });
    expect(ok).toBe(true);
  });

  it("startGitFetch + events 应驱动 store 状态变化（允许空 repo）", async () => {
    const store = useTasksStore();
    await initTaskEvents();

  (invoke as any).mockResolvedValueOnce("git-f1");
  const id = await startGitFetch("", "C:/tmp/repo");
    expect(id).toBe("git-f1");
  expect(invoke).toHaveBeenCalledWith("git_fetch", { repo: "", dest: "C:/tmp/repo" });

    const calls: any[] = (listenMock as any)._calls || [];
    const stateListener = calls.find((c) => c.evt === "task://state")!;
    const progListener = calls.find((c) => c.evt === "task://progress")!;

  stateListener.cb({ payload: { taskId: "git-f1", kind: "GitFetch", state: "running", createdAt: 1 } });
  await Promise.resolve();
  const item1 = store.items.find((x) => x.id === "git-f1");
  expect(item1).toBeTruthy();
  expect(item1).toMatchObject({ id: "git-f1", state: "running", kind: "GitFetch" });

  // 桥接的阶段事件：Negotiating -> Receiving -> Fetching（旧起始）
  progListener.cb({ payload: { taskId: "git-f1", kind: "GitFetch", phase: "Negotiating", percent: 0 } });
  await Promise.resolve();
  expect(store.progressById["git-f1"]).toMatchObject({ phase: "Negotiating" });

  progListener.cb({ payload: { taskId: "git-f1", kind: "GitFetch", phase: "Receiving", percent: 10 } });
  await Promise.resolve();
  expect(store.progressById["git-f1"]).toMatchObject({ phase: "Receiving", percent: 10 });

  // 继续常规进度（包含可选字段）
  progListener.cb({ payload: { taskId: "git-f1", kind: "GitFetch", phase: "Fetching", percent: 55, objects: 20, bytes: 2048 } });
  await Promise.resolve();
    expect(store.progressById["git-f1"]).toMatchObject({ percent: 55, phase: "Fetching", objects: 20, bytes: 2048 });

  stateListener.cb({ payload: { taskId: "git-f1", kind: "GitFetch", state: "completed", createdAt: 1 } });
  await Promise.resolve();
  const item2 = store.items.find((x) => x.id === "git-f1");
  expect(item2).toBeTruthy();
  expect(item2).toMatchObject({ id: "git-f1", state: "completed" });

    disposeTaskEvents();
  });

  it("startGitFetch 透传 preset 给后端", async () => {
    await initTaskEvents();
    (invoke as any).mockResolvedValueOnce("git-f2");
    const id2 = await startGitFetch("", "C:/tmp/repo", { preset: "branches" });
    expect(id2).toBe("git-f2");
    expect(invoke).toHaveBeenCalledWith("git_fetch", { repo: "", dest: "C:/tmp/repo", preset: "branches" });
    disposeTaskEvents();
  });
});
