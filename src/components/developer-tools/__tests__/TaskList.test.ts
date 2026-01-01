import { describe, it, expect, beforeEach, vi } from "vitest";
import { mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import TaskList from "../TaskList.vue";
import { useTasksStore } from "../../../stores/tasks";

// Mock the API functions
vi.mock("../../../api/tasks", () => ({
  listTasks: vi.fn().mockResolvedValue([]),
  startSleepTask: vi.fn().mockResolvedValue(undefined),
  cancelTask: vi.fn().mockResolvedValue(undefined),
}));

describe("TaskList (deprecated)", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.clearAllMocks();
  });

  describe("rendering", () => {
    it("renders the component", () => {
      const wrapper = mount(TaskList);
      expect(wrapper.find(".task-list").exists()).toBe(true);
    });

    it("renders toolbar with input and buttons", () => {
      const wrapper = mount(TaskList);
      expect(wrapper.find(".toolbar").exists()).toBe(true);
      expect(wrapper.find('input[type="number"]').exists()).toBe(true);
      expect(wrapper.findAll("button").length).toBeGreaterThanOrEqual(2);
    });

    it("renders table structure", () => {
      const wrapper = mount(TaskList);
      expect(wrapper.find("table").exists()).toBe(true);
      expect(wrapper.find("thead").exists()).toBe(true);
      expect(wrapper.find("tbody").exists()).toBe(true);
    });

    it("renders table headers", () => {
      const wrapper = mount(TaskList);
      const headers = wrapper.findAll("th");
      expect(headers.length).toBe(5);
      expect(headers[0].text()).toBe("ID");
      expect(headers[1].text()).toBe("类型");
      expect(headers[2].text()).toBe("状态");
      expect(headers[3].text()).toBe("创建时间");
    });
  });

  describe("empty state", () => {
    it("shows empty message when no tasks", () => {
      const wrapper = mount(TaskList);
      expect(wrapper.text()).toContain("无任务");
    });
  });

  describe("with tasks", () => {
    it("renders task rows", async () => {
      const store = useTasksStore();
      store.upsert({
        id: "test-task-123456",
        kind: "GitClone",
        state: "running",
        createdAt: Date.now(),
      });

      const wrapper = mount(TaskList);
      // Wait for component to update
      await wrapper.vm.$nextTick();

      const rows = wrapper.findAll("tbody tr");
      expect(rows.length).toBe(1);
      expect(wrapper.text()).toContain("test-tas"); // Shows first 8 chars
      expect(wrapper.text()).toContain("GitClone");
    });

    it("applies correct state class for running tasks", async () => {
      const store = useTasksStore();
      store.upsert({
        id: "task-1",
        kind: "GitFetch",
        state: "running",
        createdAt: Date.now(),
      });

      const wrapper = mount(TaskList);
      await wrapper.vm.$nextTick();

      expect(wrapper.find(".state.running").exists()).toBe(true);
    });

    it("shows cancel button for running tasks", async () => {
      const store = useTasksStore();
      store.upsert({
        id: "task-1",
        kind: "GitFetch",
        state: "running",
        createdAt: Date.now(),
      });

      const wrapper = mount(TaskList);
      await wrapper.vm.$nextTick();

      const cancelBtn = wrapper
        .findAll("tbody button")
        .find((b) => b.text().includes("取消"));
      expect(cancelBtn).toBeDefined();
    });

    it("does not show cancel button for completed tasks", async () => {
      const store = useTasksStore();
      store.upsert({
        id: "task-1",
        kind: "GitFetch",
        state: "completed",
        createdAt: Date.now(),
      });

      const wrapper = mount(TaskList);
      await wrapper.vm.$nextTick();

      const cancelBtn = wrapper.find("tbody button");
      expect(cancelBtn.exists()).toBe(false);
    });
  });

  describe("toolbar", () => {
    it("has sleep duration input with default value", () => {
      const wrapper = mount(TaskList);
      const input = wrapper.find('input[type="number"]');
      expect((input.element as HTMLInputElement).value).toBe("3000");
    });

    it("has refresh button", () => {
      const wrapper = mount(TaskList);
      const buttons = wrapper.findAll(".toolbar button");
      const refreshBtn = buttons.find((b) => b.text().includes("刷新"));
      expect(refreshBtn).toBeDefined();
    });

    it("has start sleep task button", () => {
      const wrapper = mount(TaskList);
      const buttons = wrapper.findAll(".toolbar button");
      const sleepBtn = buttons.find((b) =>
        b.text().includes("启动 Sleep 任务")
      );
      expect(sleepBtn).toBeDefined();
    });
  });
});
