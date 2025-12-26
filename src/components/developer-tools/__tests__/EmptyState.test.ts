import { describe, it, expect } from "vitest";
import { mount } from "@vue/test-utils";
import EmptyState from "../observability/EmptyState.vue";

describe("EmptyState (observability)", () => {
  describe("rendering", () => {
    it("renders the component", () => {
      const wrapper = mount(EmptyState);
      expect(wrapper.find(".empty-state").exists()).toBe(true);
    });

    it("renders default message when no props provided", () => {
      const wrapper = mount(EmptyState);
      expect(wrapper.text()).toBe("暂无可展示的数据");
    });

    it("renders custom message when provided", () => {
      const wrapper = mount(EmptyState, {
        props: { message: "No data available" },
      });
      expect(wrapper.text()).toBe("No data available");
    });
  });

  describe("styling", () => {
    it("has correct styling classes", () => {
      const wrapper = mount(EmptyState);
      const el = wrapper.find(".empty-state");
      expect(el.classes()).toContain("flex");
      expect(el.classes()).toContain("min-h-24");
      expect(el.classes()).toContain("items-center");
      expect(el.classes()).toContain("justify-center");
    });
  });
});
