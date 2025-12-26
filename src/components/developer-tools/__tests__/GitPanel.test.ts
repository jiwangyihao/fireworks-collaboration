import { describe, it, expect } from "vitest";
import { mount } from "@vue/test-utils";
import GitPanel from "../observability/GitPanel.vue";
import LoadingState from "../observability/LoadingState.vue";
import EmptyState from "../observability/EmptyState.vue";

describe("GitPanel", () => {
  describe("loading state", () => {
    it("shows LoadingState when loading without snapshot", () => {
      const wrapper = mount(GitPanel, {
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
      const wrapper = mount(GitPanel, {
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
      const wrapper = mount(GitPanel, {
        props: {
          snapshot: null,
          loading: false,
          error: "Connection failed",
          stale: false,
        },
      });
      expect(wrapper.find(".git-panel__error").exists()).toBe(true);
      expect(wrapper.text()).toContain("Connection failed");
      expect(wrapper.text()).toContain("加载 Git 指标失败");
    });
  });

  describe("empty state", () => {
    it("shows EmptyState when no snapshot, loading, or error", () => {
      const wrapper = mount(GitPanel, {
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
      const wrapper = mount(GitPanel, {
        props: {
          snapshot: { generatedAtMs: Date.now(), series: [] },
          loading: false,
          error: null,
          stale: false,
        },
      });
      expect(wrapper.find(".git-panel__content").exists()).toBe(true);
    });

    it("shows stale badge when data is cached", () => {
      const wrapper = mount(GitPanel, {
        props: {
          snapshot: { generatedAtMs: Date.now(), series: [] },
          loading: false,
          error: null,
          stale: true,
        },
      });
      expect(wrapper.find(".git-panel__badge").exists()).toBe(true);
      expect(wrapper.text()).toContain("数据为缓存");
    });

    it("renders metric cards section", () => {
      const wrapper = mount(GitPanel, {
        props: {
          snapshot: { generatedAtMs: Date.now(), series: [] },
          loading: false,
          error: null,
          stale: false,
        },
      });
      expect(wrapper.find(".git-panel__cards").exists()).toBe(true);
    });

    it("renders chart section", () => {
      const wrapper = mount(GitPanel, {
        props: {
          snapshot: { generatedAtMs: Date.now(), series: [] },
          loading: false,
          error: null,
          stale: false,
        },
      });
      expect(wrapper.find(".git-panel__chart").exists()).toBe(true);
    });
  });
});
