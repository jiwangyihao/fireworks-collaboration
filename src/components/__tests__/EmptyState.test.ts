import { describe, it, expect } from "vitest";
import { mount } from "@vue/test-utils";
import EmptyState from "../EmptyState.vue";

describe("EmptyState", () => {
  describe("rendering", () => {
    it("renders default icon emoji", () => {
      const wrapper = mount(EmptyState);
      expect(wrapper.text()).toContain("📭");
    });

    it("renders custom icon prop", () => {
      const wrapper = mount(EmptyState, {
        props: { icon: "🔍" },
      });

      expect(wrapper.text()).toContain("🔍");
    });

    it("renders icon slot content instead of prop", () => {
      const wrapper = mount(EmptyState, {
        props: { icon: "📭" },
        slots: { icon: "🎉" },
      });

      expect(wrapper.text()).toContain("🎉");
      expect(wrapper.text()).not.toContain("📭");
    });
  });

  describe("title", () => {
    it("does not render title when not provided", () => {
      const wrapper = mount(EmptyState);
      expect(wrapper.find("p.text-sm").exists()).toBe(false);
    });

    it("renders title prop", () => {
      const wrapper = mount(EmptyState, {
        props: { title: "No items found" },
      });

      expect(wrapper.find("p.text-sm").text()).toBe("No items found");
    });

    it("renders title slot instead of prop", () => {
      const wrapper = mount(EmptyState, {
        slots: { title: "<span>Custom Title</span>" },
      });

      expect(wrapper.text()).toContain("Custom Title");
    });
  });

  describe("description", () => {
    it("does not render description when not provided", () => {
      const wrapper = mount(EmptyState);
      expect(wrapper.find("p.text-xs").exists()).toBe(false);
    });

    it("renders description prop", () => {
      const wrapper = mount(EmptyState, {
        props: { description: "Try adding some items" },
      });

      expect(wrapper.find("p.text-xs").text()).toBe("Try adding some items");
    });

    it("renders description slot instead of prop", () => {
      const wrapper = mount(EmptyState, {
        slots: { description: "<span>Custom Description</span>" },
      });

      expect(wrapper.text()).toContain("Custom Description");
    });
  });

  describe("default slot", () => {
    it("renders default slot content for actions", () => {
      const wrapper = mount(EmptyState, {
        slots: { default: '<button class="btn">Add Item</button>' },
      });

      expect(wrapper.find("button.btn").exists()).toBe(true);
      expect(wrapper.text()).toContain("Add Item");
    });
  });

  describe("styling", () => {
    it("has centered text styling", () => {
      const wrapper = mount(EmptyState);
      expect(wrapper.classes()).toContain("text-center");
      expect(wrapper.classes()).toContain("py-6");
    });
  });
});
