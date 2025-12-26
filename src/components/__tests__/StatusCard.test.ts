import { describe, it, expect } from "vitest";
import { mount } from "@vue/test-utils";
import StatusCard from "../StatusCard.vue";
import BaseCard from "../BaseCard.vue";
import BaseBadge from "../BaseBadge.vue";

describe("StatusCard", () => {
  describe("rendering", () => {
    it("renders title", () => {
      const wrapper = mount(StatusCard, {
        props: { title: "Card Title" },
      });

      expect(wrapper.find("h4").text()).toContain("Card Title");
    });

    it("renders slot content", () => {
      const wrapper = mount(StatusCard, {
        props: { title: "Title" },
        slots: { default: "<p>Content</p>" },
      });

      expect(wrapper.text()).toContain("Content");
    });

    it("uses BaseCard component", () => {
      const wrapper = mount(StatusCard, {
        props: { title: "Title" },
      });

      expect(wrapper.findComponent(BaseCard).exists()).toBe(true);
    });
  });

  describe("icon", () => {
    it("renders icon emoji when provided", () => {
      const wrapper = mount(StatusCard, {
        props: { title: "Title", icon: "🔧" },
      });

      expect(wrapper.find("h4").text()).toContain("🔧");
    });

    it("renders icon slot instead of prop", () => {
      const wrapper = mount(StatusCard, {
        props: { title: "Title", icon: "🔧" },
        slots: { icon: "<span>🎉</span>" },
      });

      expect(wrapper.text()).toContain("🎉");
    });
  });

  describe("badge", () => {
    it("does not render badge when not provided", () => {
      const wrapper = mount(StatusCard, {
        props: { title: "Title" },
      });

      expect(wrapper.findComponent(BaseBadge).exists()).toBe(false);
    });

    it("renders badge when provided", () => {
      const wrapper = mount(StatusCard, {
        props: { title: "Title", badge: "New" },
      });

      expect(wrapper.findComponent(BaseBadge).exists()).toBe(true);
      expect(wrapper.findComponent(BaseBadge).text()).toBe("New");
    });

    it("applies badge variant", () => {
      const wrapper = mount(StatusCard, {
        props: { title: "Title", badge: "Active", badgeVariant: "success" },
      });

      expect(wrapper.findComponent(BaseBadge).props("variant")).toBe("success");
    });
  });

  describe("loading state", () => {
    it("does not show loading spinner by default", () => {
      const wrapper = mount(StatusCard, {
        props: { title: "Title" },
      });

      expect(wrapper.find(".loading").exists()).toBe(false);
    });

    it("shows loading spinner when loading is true", () => {
      const wrapper = mount(StatusCard, {
        props: { title: "Title", loading: true },
      });

      expect(wrapper.find(".loading").exists()).toBe(true);
      expect(wrapper.find(".loading-spinner").exists()).toBe(true);
    });
  });

  describe("variant and flex props", () => {
    it("passes variant to BaseCard", () => {
      const wrapper = mount(StatusCard, {
        props: { title: "Title", variant: "gradient" },
      });

      expect(wrapper.findComponent(BaseCard).props("variant")).toBe("gradient");
    });

    it("passes flex to BaseCard", () => {
      const wrapper = mount(StatusCard, {
        props: { title: "Title", flex: true },
      });

      expect(wrapper.findComponent(BaseCard).props("flex")).toBe(true);
    });
  });

  describe("header-actions slot", () => {
    it("renders header-actions slot content", () => {
      const wrapper = mount(StatusCard, {
        props: { title: "Title" },
        slots: { "header-actions": '<button class="btn-test">Action</button>' },
      });

      expect(wrapper.find(".btn-test").exists()).toBe(true);
    });
  });
});
