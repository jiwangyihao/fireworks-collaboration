import { describe, it, expect } from "vitest";
import { mount } from "@vue/test-utils";
import LoadingState from "../observability/LoadingState.vue";

describe("LoadingState", () => {
  describe("rendering", () => {
    it("renders the component", () => {
      const wrapper = mount(LoadingState);
      expect(wrapper.find(".loading-state").exists()).toBe(true);
    });

    it("displays loading spinner", () => {
      const wrapper = mount(LoadingState);
      expect(wrapper.find(".loading.loading-spinner").exists()).toBe(true);
    });

    it("displays loading text in Chinese", () => {
      const wrapper = mount(LoadingState);
      expect(wrapper.text()).toContain("数据加载中...");
    });
  });

  describe("styling", () => {
    it("has flexbox centering classes", () => {
      const wrapper = mount(LoadingState);
      const el = wrapper.find(".loading-state");
      expect(el.classes()).toContain("flex");
      expect(el.classes()).toContain("min-h-24");
      expect(el.classes()).toContain("items-center");
      expect(el.classes()).toContain("justify-center");
    });

    it("has gap between spinner and text", () => {
      const wrapper = mount(LoadingState);
      expect(wrapper.find(".loading-state").classes()).toContain("gap-2");
    });
  });
});
