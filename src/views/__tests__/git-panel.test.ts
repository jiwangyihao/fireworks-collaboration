import { describe, it, expect, vi, beforeEach } from "vitest";
import { mount } from "@vue/test-utils";
import { setActivePinia, createPinia } from "pinia";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

vi.mock("../../api/tasks", () => ({
  startGitClone: vi.fn().mockResolvedValue("tid-1"),
  startGitFetch: vi.fn().mockResolvedValue("tid-2"),
  cancelTask: vi.fn(),
  listTasks: vi.fn().mockResolvedValue([]),
}));

import GitPanel from "../GitPanel.vue";
import { startGitClone, startGitFetch, cancelTask } from "../../api/tasks";
import { useTasksStore } from "../../stores/tasks";

describe("views/GitPanel", () => {
  beforeEach(() => setActivePinia(createPinia()));

  it("点击开始克隆会调用 startGitClone", async () => {
    const w = mount(GitPanel);
    const btn = w.get("button.btn-primary");
    await btn.trigger("click");
    expect(startGitClone).toHaveBeenCalled();
  });

  it("点击 Fetch 会调用 startGitFetch（允许 repo 为空）", async () => {
    const w = mount(GitPanel);
    // 将 repo 清空，仅保留 dest
    await w.find("input.input.input-bordered.input-sm.flex-1").setValue("");
    const fetchBtn = w.get("button.btn-secondary");
    await fetchBtn.trigger("click");
    expect(startGitFetch).toHaveBeenCalled();
  });

  it("选择预设会将 preset 传入 startGitFetch", async () => {
    const w = mount(GitPanel);
    // 选择 分支+标签 预设
    await w.find("select.select.select-bordered.select-sm").setValue("branches+tags");
    const fetchBtn = w.get("button.btn-secondary");
    await fetchBtn.trigger("click");
    expect(startGitFetch).toHaveBeenCalledWith(expect.any(String), expect.any(String), "branches+tags");
  });

  it("存在 running 任务时点击取消应调用 cancelTask", async () => {
    const w = mount(GitPanel);
    const store = useTasksStore();
    store.upsert({ id: "tid-x", kind: "GitClone", state: "running", createdAt: Date.now() });
    await w.vm.$nextTick();
    const cancelBtn = w.find("button.btn-xs");
    await cancelBtn.trigger("click");
    expect(cancelTask).toHaveBeenCalledWith("tid-x");
  });

  it("进度栏可渲染 objects/bytes 字段（格式化）", async () => {
    const w = mount(GitPanel);
    const store = useTasksStore();
    store.upsert({ id: "tid-y", kind: "GitFetch", state: "running", createdAt: Date.now() });
    store.updateProgress({ taskId: "tid-y", percent: 12, phase: "Receiving", objects: 123, bytes: 5 * 1024 * 1024 });
    await w.vm.$nextTick();
    const text = w.text();
    expect(text).toContain("Receiving");
    expect(text).toContain("objs: 123");
    expect(text).toMatch(/5\.00 MiB|5 MiB/);
  });
});
