import { describe, it, expect, beforeEach, vi } from "vitest";
import { mount, flushPromises } from "@vue/test-utils";
import AuditLogView from "../AuditLogView.vue";

// Mock the credential API
vi.mock("../../../api/credential", () => ({
  exportAuditLog: vi.fn().mockResolvedValue("[]"),
  cleanupAuditLogs: vi.fn().mockResolvedValue(0),
}));

describe("AuditLogView", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe("rendering", () => {
    it("renders the page title", async () => {
      const wrapper = mount(AuditLogView);
      await flushPromises();
      expect(wrapper.find("h2").text()).toBe("审计日志");
    });

    it("renders refresh button", async () => {
      const wrapper = mount(AuditLogView);
      await flushPromises();
      const buttons = wrapper.findAll("button");
      const refreshButton = buttons.find((b) => b.text().includes("刷新"));
      expect(refreshButton).toBeDefined();
    });

    it("renders export button", async () => {
      const wrapper = mount(AuditLogView);
      await flushPromises();
      const buttons = wrapper.findAll("button");
      const exportButton = buttons.find((b) => b.text().includes("导出"));
      expect(exportButton).toBeDefined();
    });

    it("renders cleanup button", async () => {
      const wrapper = mount(AuditLogView);
      await flushPromises();
      const buttons = wrapper.findAll("button");
      const cleanupButton = buttons.find((b) =>
        b.text().includes("清理旧日志")
      );
      expect(cleanupButton).toBeDefined();
    });
  });

  describe("filters", () => {
    it("renders operation type filter", async () => {
      const wrapper = mount(AuditLogView);
      await flushPromises();
      const operationSelect = wrapper.find("select");
      expect(operationSelect.exists()).toBe(true);
    });

    it("renders host filter input", async () => {
      const wrapper = mount(AuditLogView);
      await flushPromises();
      const hostInput = wrapper.find('input[placeholder="输入主机名..."]');
      expect(hostInput.exists()).toBe(true);
    });
  });

  describe("empty state", () => {
    it("shows empty message when no logs", async () => {
      const wrapper = mount(AuditLogView);
      await flushPromises();
      expect(wrapper.text()).toContain("暂无审计日志");
    });
  });
});
