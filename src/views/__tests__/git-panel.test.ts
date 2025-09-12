import { describe, it, expect, vi, beforeEach } from "vitest";
import { mount } from "@vue/test-utils";
import { setActivePinia, createPinia } from "pinia";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

vi.mock("../../api/tasks", () => ({
  startGitClone: vi.fn().mockResolvedValue("tid-1"),
  cancelTask: vi.fn(),
  listTasks: vi.fn().mockResolvedValue([]),
}));

import GitPanel from "../GitPanel.vue";
import { startGitClone, cancelTask } from "../../api/tasks";
import { useTasksStore } from "../../stores/tasks";

describe("views/GitPanel", () => {
  beforeEach(() => setActivePinia(createPinia()));

  it("点击开始克隆会调用 startGitClone", async () => {
    const w = mount(GitPanel);
    const btn = w.get("button.btn-primary");
    await btn.trigger("click");
    expect(startGitClone).toHaveBeenCalled();
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
});
