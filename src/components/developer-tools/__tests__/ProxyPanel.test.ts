import { describe, it, expect } from "vitest";
import { mount } from "@vue/test-utils";
import ProxyPanel from "../observability/ProxyPanel.vue";
import LoadingState from "../observability/LoadingState.vue";
import EmptyState from "../observability/EmptyState.vue";

describe("ProxyPanel", () => {
  describe("loading state", () => {
    it("shows LoadingState when loading without snapshot", () => {
      const wrapper = mount(ProxyPanel, {
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
      const wrapper = mount(ProxyPanel, {
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
      const wrapper = mount(ProxyPanel, {
        props: {
          snapshot: null,
          loading: false,
          error: "Proxy unavailable",
          stale: false,
        },
      });
      expect(wrapper.find(".proxy-panel__error").exists()).toBe(true);
      expect(wrapper.text()).toContain("Proxy unavailable");
      expect(wrapper.text()).toContain("加载代理指标失败");
    });
  });

  describe("empty state", () => {
    it("shows EmptyState when no snapshot, loading, or error", () => {
      const wrapper = mount(ProxyPanel, {
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
      const wrapper = mount(ProxyPanel, {
        props: {
          snapshot: { generatedAtMs: Date.now(), series: [] },
          loading: false,
          error: null,
          stale: false,
        },
      });
      expect(wrapper.find(".proxy-panel__content").exists()).toBe(true);
    });

    it("shows stale badge when data is cached", () => {
      const wrapper = mount(ProxyPanel, {
        props: {
          snapshot: { generatedAtMs: Date.now(), series: [] },
          loading: false,
          error: null,
          stale: true,
        },
      });
      expect(wrapper.find(".proxy-panel__badge").exists()).toBe(true);
      expect(wrapper.text()).toContain("数据为缓存");
    });

    it("renders metric cards section", () => {
      const wrapper = mount(ProxyPanel, {
        props: {
          snapshot: { generatedAtMs: Date.now(), series: [] },
          loading: false,
          error: null,
          stale: false,
        },
      });
      expect(wrapper.find(".proxy-panel__cards").exists()).toBe(true);
    });

    it("renders chart section", () => {
      const wrapper = mount(ProxyPanel, {
        props: {
          snapshot: { generatedAtMs: Date.now(), series: [] },
          loading: false,
          error: null,
          stale: false,
        },
      });
      expect(wrapper.find(".proxy-panel__chart").exists()).toBe(true);
    });
  });
});
