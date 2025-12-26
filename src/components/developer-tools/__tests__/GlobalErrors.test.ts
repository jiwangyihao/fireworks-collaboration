import { describe, it, expect, beforeEach } from "vitest";
import { mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import GlobalErrors from "../GlobalErrors.vue";
import { useLogsStore } from "../../../stores/logs";

describe("GlobalErrors (deprecated)", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
  });

  describe("rendering", () => {
    it("renders the container", () => {
      const wrapper = mount(GlobalErrors);
      expect(wrapper.find(".fixed").exists()).toBe(true);
    });

    it("renders no alerts when logs are empty", () => {
      const wrapper = mount(GlobalErrors);
      expect(wrapper.findAll(".alert").length).toBe(0);
    });

    it("does not render clear button when no logs", () => {
      const wrapper = mount(GlobalErrors);
      expect(wrapper.find("button").exists()).toBe(false);
    });
  });

  describe("with logs", () => {
    it("renders alerts for each log item", () => {
      const logs = useLogsStore();
      logs.push("info", "Test info message");
      logs.push("warn", "Test warning");

      const wrapper = mount(GlobalErrors);
      expect(wrapper.findAll(".alert").length).toBe(2);
    });

    it("applies correct color class for info logs", () => {
      const logs = useLogsStore();
      logs.push("info", "Info message");

      const wrapper = mount(GlobalErrors);
      expect(wrapper.find(".text-blue-700").exists()).toBe(true);
    });

    it("applies correct color class for warning logs", () => {
      const logs = useLogsStore();
      logs.push("warn", "Warning message");

      const wrapper = mount(GlobalErrors);
      expect(wrapper.find(".text-yellow-700").exists()).toBe(true);
    });

    it("applies correct color class for error logs", () => {
      const logs = useLogsStore();
      logs.push("error", "Error message");

      const wrapper = mount(GlobalErrors);
      expect(wrapper.find(".text-red-700").exists()).toBe(true);
    });

    it("shows clear button when logs exist", () => {
      const logs = useLogsStore();
      logs.push("info", "Test message");

      const wrapper = mount(GlobalErrors);
      expect(wrapper.find("button").exists()).toBe(true);
      expect(wrapper.find("button").text()).toBe("清空");
    });

    it("clears logs when clear button clicked", async () => {
      const logs = useLogsStore();
      logs.push("info", "Test message");

      const wrapper = mount(GlobalErrors);
      expect(wrapper.findAll(".alert").length).toBe(1);

      await wrapper.find("button").trigger("click");
      expect(logs.items.length).toBe(0);
    });
  });
});
