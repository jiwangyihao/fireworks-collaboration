import { describe, it, expect, beforeEach } from "vitest";
import { mount } from "@vue/test-utils";
import { setActivePinia, createPinia } from "pinia";

import GitPanel from "../GitPanel.vue";
import { useTasksStore } from "../../../stores/tasks";

// 复用现有 mock（git-panel.test.ts 中已经对 api/tasks 做了 mock），本文件不需要额外 mocks

describe("views/GitPanel - error display", () => {
  beforeEach(() => setActivePinia(createPinia()));

  it("当存在 lastError 时，表格应显示分类、重试次数和消息", async () => {
    const w = mount(GitPanel);
    const store = useTasksStore();
    const id = "tid-e1";
    store.upsert({ id, kind: "GitFetch", state: "failed", createdAt: Date.now() });
    store.setLastError(id, { category: "Network", message: "连接超时", retriedTimes: 3 } as any);
    await w.vm.$nextTick();

    const rowText = w.text();
    expect(rowText).toContain("Network");
    expect(rowText).toContain("重试 3 次");
    expect(rowText).toContain("连接超时");
  });

  it("当没有 lastError 时，显示 - 占位", async () => {
    const w = mount(GitPanel);
    const store = useTasksStore();
    const id = "tid-e2";
    store.upsert({ id, kind: "GitClone", state: "running", createdAt: Date.now() });
    await w.vm.$nextTick();

    const row = w.findAll("tbody tr")[0];
    expect(row.text()).toContain("-");
  });
});
