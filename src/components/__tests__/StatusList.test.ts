import { describe, it, expect } from "vitest";
import { mount } from "@vue/test-utils";
import StatusList from "../StatusList.vue";
import type { StatusItem } from "../StatusList.vue";

describe("StatusList", () => {
  const mockItems: StatusItem[] = [
    { id: 1, type: "success", message: "Check passed" },
    { id: 2, type: "warning", message: "Check warning" },
    { id: 3, type: "error", message: "Check failed" },
    { id: 4, type: "info", message: "Check info" },
  ];

  describe("rendering", () => {
    it("renders list items for each status", () => {
      const wrapper = mount(StatusList, {
        props: { items: mockItems },
      });

      const listItems = wrapper.findAll("li");
      expect(listItems.length).toBe(4);
    });

    it("renders message text for each item", () => {
      const wrapper = mount(StatusList, {
        props: { items: mockItems },
      });

      expect(wrapper.text()).toContain("Check passed");
      expect(wrapper.text()).toContain("Check warning");
      expect(wrapper.text()).toContain("Check failed");
      expect(wrapper.text()).toContain("Check info");
    });

    it("renders empty list when no items", () => {
      const wrapper = mount(StatusList, {
        props: { items: [] },
      });

      expect(wrapper.findAll("li").length).toBe(0);
    });
  });

  describe("status indicators", () => {
    it("renders status dot for each item", () => {
      const wrapper = mount(StatusList, {
        props: { items: mockItems },
      });

      expect(wrapper.findAll(".status").length).toBeGreaterThan(0);
    });

    it("applies correct status class for success", () => {
      const wrapper = mount(StatusList, {
        props: { items: [{ id: 1, type: "success", message: "Success" }] },
      });

      expect(wrapper.find(".status-success").exists()).toBe(true);
    });

    it("applies correct status class for warning", () => {
      const wrapper = mount(StatusList, {
        props: { items: [{ id: 1, type: "warning", message: "Warning" }] },
      });

      expect(wrapper.find(".status-warning").exists()).toBe(true);
    });

    it("applies correct status class for error", () => {
      const wrapper = mount(StatusList, {
        props: { items: [{ id: 1, type: "error", message: "Error" }] },
      });

      expect(wrapper.find(".status-error").exists()).toBe(true);
    });

    it("applies correct status class for info", () => {
      const wrapper = mount(StatusList, {
        props: { items: [{ id: 1, type: "info", message: "Info" }] },
      });

      expect(wrapper.find(".status-info").exists()).toBe(true);
    });
  });

  describe("animations", () => {
    it("does not show ping animation for success items", () => {
      const wrapper = mount(StatusList, {
        props: { items: [{ id: 1, type: "success", message: "Success" }] },
      });

      // Success items should have only one .status element (no ping overlay)
      expect(wrapper.findAll(".status").length).toBe(1);
      expect(wrapper.html()).not.toContain("animate-ping");
    });

    it("shows ping animation for non-success items", () => {
      const wrapper = mount(StatusList, {
        props: { items: [{ id: 1, type: "warning", message: "Warning" }] },
      });

      // Non-success items should have two .status elements (one ping overlay + one solid)
      expect(wrapper.findAll(".status").length).toBe(2);
      expect(wrapper.html()).toContain("animate-ping");
    });

    it("shows ping animation for error items", () => {
      const wrapper = mount(StatusList, {
        props: { items: [{ id: 1, type: "error", message: "Error" }] },
      });

      // Error items should have two .status elements (one ping overlay + one solid)
      expect(wrapper.findAll(".status").length).toBe(2);
      expect(wrapper.html()).toContain("animate-ping");
    });
  });

  describe("TransitionGroup", () => {
    it("wraps items in TransitionGroup", () => {
      const wrapper = mount(StatusList, {
        props: { items: mockItems },
      });

      // TransitionGroup is rendered (as stub in test environment)
      // Check that list items are rendered inside
      expect(wrapper.findAll("li").length).toBe(4);
    });
  });
});
