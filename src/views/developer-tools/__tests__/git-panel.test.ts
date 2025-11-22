import { describe, it, expect, vi, beforeEach } from "vitest";
import { mount } from "@vue/test-utils";
import { setActivePinia, createPinia } from "pinia";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

vi.mock("../../../api/tasks", () => ({
  startGitClone: vi.fn().mockResolvedValue("tid-1"),
  startGitFetch: vi.fn().mockResolvedValue("tid-2"),
  startGitPush: vi.fn().mockResolvedValue("tid-3"),
  startGitInit: vi.fn().mockResolvedValue("tid-4"),
  startGitAdd: vi.fn().mockResolvedValue("tid-5"),
  startGitCommit: vi.fn().mockResolvedValue("tid-6"),
  cancelTask: vi.fn(),
  listTasks: vi.fn().mockResolvedValue([]),
}));

import GitPanel from "../GitPanel.vue";
import { startGitClone, startGitFetch, startGitPush, startGitInit, startGitAdd, startGitCommit, cancelTask } from "../../../api/tasks";
import { useTasksStore } from "../../../stores/tasks";

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

  it("点击 Push 会调用 startGitPush 并传入凭证和 refspec", async () => {
    const w = mount(GitPanel);
    // 设置 Push 输入
    const inputs = w.findAll("input.input.input-bordered.input-sm");
    // 根据模板顺序：repo, dest, pushDest, remote, refspec, username, password
    await inputs[2].setValue("C:/tmp/log"); // pushDest
    await inputs[3].setValue("origin"); // remote
    await inputs[4].setValue("refs/heads/main:refs/heads/main"); // refspec
    await inputs[5].setValue("x-access-token"); // username
    await inputs[6].setValue("token-123"); // password
    const pushBtn = w.get("button.btn-accent.btn-sm");
    await pushBtn.trigger("click");
    expect(startGitPush).toHaveBeenCalledWith({
      dest: "C:/tmp/log",
      remote: "origin",
      refspecs: ["refs/heads/main:refs/heads/main"],
      username: "x-access-token",
      password: "token-123",
    });
  });

  it("点击 Init 会调用 startGitInit", async () => {
    const w = mount(GitPanel);
    const buttons = w.findAll('button');
    const initBtn = buttons.find(b => /Init/i.test(b.text()));
    expect(initBtn).toBeTruthy();
    await initBtn!.trigger('click');
    expect(startGitInit).toHaveBeenCalled();
  });

  it("点击 Add 会调用 startGitAdd 并拆分路径", async () => {
    const w = mount(GitPanel);
    // 定位 Add 输入：textarea + 按钮
    const textarea = w.find('textarea.textarea');
    await textarea.setValue('README.md, src/main.ts');
    const addBtn = w.findAll('button.btn-sm').filter(b => b.text() === 'Add')[0];
    await addBtn.trigger('click');
    expect(startGitAdd).toHaveBeenCalledWith(expect.any(String), ['README.md', 'src/main.ts']);
  });

  it("点击 Commit 会调用 startGitCommit", async () => {
    const w = mount(GitPanel);
    const commitBtn = w.findAll('button.btn-sm').find(b => b.text() === 'Commit');
    expect(commitBtn).toBeTruthy();
    await commitBtn!.trigger('click');
    expect(startGitCommit).toHaveBeenCalled();
  });
});
