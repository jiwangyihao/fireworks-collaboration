import { describe, it, expect } from "vitest";
import { mount } from "@vue/test-utils";
import AlertsPanel from "../observability/AlertsPanel.vue";
import LoadingState from "../observability/LoadingState.vue";
import EmptyState from "../observability/EmptyState.vue";

describe("AlertsPanel", () => {
  describe("loading state", () => {
    it("shows LoadingState when loading without snapshot", () => {
      const wrapper = mount(AlertsPanel, {
        props: {
          snapshot: null,
          loading: true,
          error: null,
          stale: false,
        },
      });
      expect(wrapper.findComponent(LoadingState).exists()).toBe(true);
    });

    it("does not show LoadingState when snapshot exists", () => {
      const wrapper = mount(AlertsPanel, {
        props: {
          snapshot: { generatedAtMs: Date.now(), series: [] },
          loading: true,
          error: null,
          stale: false,
        },
      });
      expect(wrapper.findComponent(LoadingState).exists()).toBe(false);
    });
  });

  describe("error state", () => {
    it("shows error message when error without snapshot", () => {
      const wrapper = mount(AlertsPanel, {
        props: {
          snapshot: null,
          loading: false,
          error: "Network error",
          stale: false,
        },
      });
      expect(wrapper.find(".alerts-panel__error").exists()).toBe(true);
      expect(wrapper.text()).toContain("Network error");
    });

    it("does not show error when snapshot exists", () => {
      const wrapper = mount(AlertsPanel, {
        props: {
          snapshot: { generatedAtMs: Date.now(), series: [] },
          loading: false,
          error: "Network error",
          stale: false,
        },
      });
      expect(wrapper.find(".alerts-panel__error").exists()).toBe(false);
    });
  });

  describe("empty state", () => {
    it("shows EmptyState when no snapshot, loading, or error", () => {
      const wrapper = mount(AlertsPanel, {
        props: {
          snapshot: null,
          loading: false,
          error: null,
          stale: false,
        },
      });
      expect(wrapper.findComponent(EmptyState).exists()).toBe(true);
    });
  });

  describe("content state", () => {
    it("shows content when snapshot exists", () => {
      const wrapper = mount(AlertsPanel, {
        props: {
          snapshot: { generatedAtMs: Date.now(), series: [] },
          loading: false,
          error: null,
          stale: false,
        },
      });
      expect(wrapper.find(".alerts-panel__content").exists()).toBe(true);
    });

    it("shows stale badge when data is cached", () => {
      const wrapper = mount(AlertsPanel, {
        props: {
          snapshot: { generatedAtMs: Date.now(), series: [] },
          loading: false,
          error: null,
          stale: true,
        },
      });
      expect(wrapper.find(".alerts-panel__badge").exists()).toBe(true);
      expect(wrapper.text()).toContain("数据为缓存");
    });
  });
});
