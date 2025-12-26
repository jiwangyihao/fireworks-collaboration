import { describe, it, expect } from "vitest";
import { mount } from "@vue/test-utils";
import BaseCard from "../BaseCard.vue";

describe("BaseCard", () => {
  describe("rendering", () => {
    it("renders slot content", () => {
      const wrapper = mount(BaseCard, {
        slots: { default: "<div>Card Content</div>" },
      });

      expect(wrapper.text()).toBe("Card Content");
    });

    it("has card and border classes", () => {
      const wrapper = mount(BaseCard);
      expect(wrapper.classes()).toContain("card");
      expect(wrapper.classes()).toContain("border-2");
      expect(wrapper.classes()).toContain("border-base-content/15");
    });

    it("has card-body wrapper for content", () => {
      const wrapper = mount(BaseCard);
      expect(wrapper.find(".card-body").exists()).toBe(true);
    });
  });

  describe("variants", () => {
    it("applies default variant bg class", () => {
      const wrapper = mount(BaseCard, {
        props: { variant: "default" },
      });

      expect(wrapper.classes()).toContain("bg-base-100");
    });

    it("applies gradient variant class", () => {
      const wrapper = mount(BaseCard, {
        props: { variant: "gradient" },
      });

      expect(wrapper.classes()).toContain("bg-gradient-to-r");
      expect(wrapper.classes()).toContain("from-secondary/5");
      expect(wrapper.classes()).toContain("to-primary/5");
    });

    it("applies default variant by default", () => {
      const wrapper = mount(BaseCard);
      expect(wrapper.classes()).toContain("bg-base-100");
    });
  });

  describe("padding", () => {
    it("applies no padding when padding is none", () => {
      const wrapper = mount(BaseCard, {
        props: { padding: "none" },
      });

      expect(wrapper.find(".card-body").classes()).toContain("p-0");
    });

    it("applies sm padding", () => {
      const wrapper = mount(BaseCard, {
        props: { padding: "sm" },
      });

      expect(wrapper.find(".card-body").classes()).toContain("p-2");
    });

    it("applies md padding by default", () => {
      const wrapper = mount(BaseCard);
      expect(wrapper.find(".card-body").classes()).toContain("p-4");
    });

    it("applies lg padding", () => {
      const wrapper = mount(BaseCard, {
        props: { padding: "lg" },
      });

      expect(wrapper.find(".card-body").classes()).toContain("p-6");
    });
  });

  describe("flex prop", () => {
    it("does not apply flex-1 by default", () => {
      const wrapper = mount(BaseCard);
      expect(wrapper.classes()).not.toContain("flex-1");
    });

    it("applies flex-1 when flex is true", () => {
      const wrapper = mount(BaseCard, {
        props: { flex: true },
      });

      expect(wrapper.classes()).toContain("flex-1");
    });
  });
});
