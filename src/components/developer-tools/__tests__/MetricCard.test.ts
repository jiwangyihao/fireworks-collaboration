import { describe, it, expect } from "vitest";
import { mount } from "@vue/test-utils";
import MetricCard from "../observability/MetricCard.vue";

describe("MetricCard", () => {
  describe("rendering", () => {
    it("renders the component", () => {
      const wrapper = mount(MetricCard, {
        props: { title: "Test Title", value: "100" },
      });
      expect(wrapper.find(".metric-card").exists()).toBe(true);
    });

    it("displays title", () => {
      const wrapper = mount(MetricCard, {
        props: { title: "Request Count", value: "500" },
      });
      expect(wrapper.find(".metric-card__title").text()).toBe("Request Count");
    });

    it("displays value", () => {
      const wrapper = mount(MetricCard, {
        props: { title: "Title", value: "1,234" },
      });
      expect(wrapper.find(".metric-card__value").text()).toBe("1,234");
    });
  });

  describe("optional props", () => {
    it("does not render description when not provided", () => {
      const wrapper = mount(MetricCard, {
        props: { title: "Title", value: "100" },
      });
      expect(wrapper.find(".metric-card__description").exists()).toBe(false);
    });

    it("renders description when provided", () => {
      const wrapper = mount(MetricCard, {
        props: { title: "Title", value: "100", description: "This is a test" },
      });
      expect(wrapper.find(".metric-card__description").text()).toBe(
        "This is a test"
      );
    });

    it("does not render trend when not provided", () => {
      const wrapper = mount(MetricCard, {
        props: { title: "Title", value: "100" },
      });
      expect(wrapper.find(".metric-card__trend").exists()).toBe(false);
    });

    it("renders trend when both trendLabel and trendValue provided", () => {
      const wrapper = mount(MetricCard, {
        props: {
          title: "Title",
          value: "100",
          trendLabel: "Change",
          trendValue: "+10%",
        },
      });
      expect(wrapper.find(".metric-card__trend").exists()).toBe(true);
      expect(wrapper.find(".metric-card__trend-label").text()).toBe("Change");
      expect(wrapper.find(".metric-card__trend-value").text()).toBe("+10%");
    });

    it("does not render trend when only trendLabel provided", () => {
      const wrapper = mount(MetricCard, {
        props: { title: "Title", value: "100", trendLabel: "Change" },
      });
      expect(wrapper.find(".metric-card__trend").exists()).toBe(false);
    });
  });

  describe("muted state", () => {
    it("does not apply opacity by default", () => {
      const wrapper = mount(MetricCard, {
        props: { title: "Title", value: "100" },
      });
      expect(wrapper.find(".metric-card").classes()).not.toContain(
        "opacity-70"
      );
    });

    it("applies opacity when muted is true", () => {
      const wrapper = mount(MetricCard, {
        props: { title: "Title", value: "0", muted: true },
      });
      expect(wrapper.find(".metric-card").classes()).toContain("opacity-70");
    });
  });

  describe("slot", () => {
    it("renders slot content", () => {
      const wrapper = mount(MetricCard, {
        props: { title: "Title", value: "100" },
        slots: { default: "<div class='custom-content'>Extra</div>" },
      });
      expect(wrapper.find(".custom-content").exists()).toBe(true);
      expect(wrapper.text()).toContain("Extra");
    });
  });
});
