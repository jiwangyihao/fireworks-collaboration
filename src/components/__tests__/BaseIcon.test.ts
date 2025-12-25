import { describe, it, expect } from "vitest";
import { mount } from "@vue/test-utils";
import BaseIcon from "../BaseIcon.vue";

describe("BaseIcon", () => {
  describe("rendering", () => {
    it("renders Icon component when icon prop is provided", () => {
      const wrapper = mount(BaseIcon, {
        props: { icon: "lucide--home" },
      });

      // Should have the wrapper span
      expect(wrapper.find("span").exists()).toBe(true);
      // Should not render slot content
      expect(wrapper.find('[data-testid="custom-svg"]').exists()).toBe(false);
    });

    it("renders custom SVG via slot when no icon prop", () => {
      const wrapper = mount(BaseIcon, {
        slots: {
          default: '<svg data-testid="custom-svg"></svg>',
        },
      });

      expect(wrapper.find('[data-testid="custom-svg"]').exists()).toBe(true);
    });

    it("does not render slot when icon prop is provided", () => {
      const wrapper = mount(BaseIcon, {
        props: { icon: "lucide--home" },
        slots: {
          default: '<svg data-testid="custom-svg"></svg>',
        },
      });

      expect(wrapper.find('[data-testid="custom-svg"]').exists()).toBe(false);
    });
  });

  describe("animations", () => {
    it("applies spin animation class to wrapper", () => {
      const wrapper = mount(BaseIcon, {
        props: { icon: "lucide--loader", spin: true },
      });

      expect(wrapper.find("span").classes()).toContain("animate-spin");
    });

    it("applies pulse animation class to wrapper", () => {
      const wrapper = mount(BaseIcon, {
        props: { icon: "lucide--loader", pulse: true },
      });

      expect(wrapper.find("span").classes()).toContain("animate-pulse");
    });

    it("does not apply animation classes by default", () => {
      const wrapper = mount(BaseIcon, {
        props: { icon: "lucide--home" },
      });

      const classes = wrapper.find("span").classes();
      expect(classes).not.toContain("animate-spin");
      expect(classes).not.toContain("animate-pulse");
    });
  });

  describe("component structure", () => {
    it("has correct wrapper classes", () => {
      const wrapper = mount(BaseIcon, {
        props: { icon: "lucide--home" },
      });

      const span = wrapper.find("span");
      expect(span.classes()).toContain("inline-flex");
      expect(span.classes()).toContain("items-center");
      expect(span.classes()).toContain("justify-center");
      expect(span.classes()).toContain("shrink-0");
    });
  });
});
