import { describe, it, expect } from "vitest";
import { mount } from "@vue/test-utils";
import BaseBadge from "../BaseBadge.vue";

describe("BaseBadge", () => {
  describe("rendering", () => {
    it("renders slot content", () => {
      const wrapper = mount(BaseBadge, {
        slots: { default: "Badge Text" },
      });

      expect(wrapper.text()).toBe("Badge Text");
    });

    it("renders as span with badge class", () => {
      const wrapper = mount(BaseBadge);
      expect(wrapper.element.tagName).toBe("SPAN");
      expect(wrapper.classes()).toContain("badge");
    });
  });

  describe("variants", () => {
    const variants = [
      "primary",
      "secondary",
      "accent",
      "success",
      "warning",
      "error",
      "info",
      "ghost",
      "outline",
    ] as const;

    variants.forEach((variant) => {
      it(`applies ${variant} variant class`, () => {
        const wrapper = mount(BaseBadge, {
          props: { variant },
        });

        expect(wrapper.classes()).toContain(`badge-${variant}`);
      });
    });

    it("applies ghost variant by default", () => {
      const wrapper = mount(BaseBadge);
      expect(wrapper.classes()).toContain("badge-ghost");
    });
  });

  describe("sizes", () => {
    const sizes = ["xs", "sm", "md", "lg"] as const;

    sizes.forEach((size) => {
      it(`applies ${size} size class`, () => {
        const wrapper = mount(BaseBadge, {
          props: { size },
        });

        expect(wrapper.classes()).toContain(`badge-${size}`);
      });
    });

    it("applies sm size by default", () => {
      const wrapper = mount(BaseBadge);
      expect(wrapper.classes()).toContain("badge-sm");
    });
  });

  describe("combined props", () => {
    it("applies both variant and size classes", () => {
      const wrapper = mount(BaseBadge, {
        props: { variant: "success", size: "lg" },
      });

      expect(wrapper.classes()).toContain("badge");
      expect(wrapper.classes()).toContain("badge-success");
      expect(wrapper.classes()).toContain("badge-lg");
    });
  });
});
