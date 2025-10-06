import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";
import OverviewPanel from "../observability/OverviewPanel.vue";
import type { MetricsSnapshot } from "../../api/metrics";

const snapshot: MetricsSnapshot = {
  generatedAtMs: Date.parse("2025-01-01T00:00:00Z"),
  series: [
    {
      name: "git_tasks_total",
      type: "counter",
      labels: { state: "completed", kind: "GitClone" },
      points: [
        { offsetSeconds: 0, value: 2 },
        { offsetSeconds: 30, value: 1 },
      ],
    },
    {
      name: "git_tasks_total",
      type: "counter",
      labels: { state: "failed", kind: "GitClone" },
      points: [
        { offsetSeconds: 0, value: 1 },
        { offsetSeconds: 30, value: 0 },
      ],
    },
    {
      name: "git_task_duration_ms",
      type: "histogram",
      labels: { kind: "GitClone" },
      histogramPoints: [
        { offsetSeconds: 0, count: 1, sum: 120 },
        { offsetSeconds: 30, count: 1, sum: 180 },
      ],
    },
    {
      name: "tls_handshake_ms",
      type: "histogram",
      labels: { outcome: "ok", sni_strategy: "fake" },
      histogramPoints: [
        { offsetSeconds: 0, count: 1, sum: 150 },
      ],
      quantiles: { p95: 180 },
    },
    {
      name: "tls_handshake_ms",
      type: "histogram",
      labels: { outcome: "ok", sni_strategy: "real" },
      histogramPoints: [
        { offsetSeconds: 0, count: 1, sum: 200 },
      ],
      quantiles: { p95: 160 },
    },
    {
      name: "ip_pool_refresh_total",
      type: "counter",
      labels: { success: "true" },
      points: [{ offsetSeconds: 0, value: 4 }],
    },
    {
      name: "ip_pool_refresh_total",
      type: "counter",
      labels: { success: "false" },
      points: [{ offsetSeconds: 0, value: 1 }],
    },
    {
      name: "alerts_fired_total",
      type: "counter",
      labels: {},
      points: [{ offsetSeconds: 0, value: 2 }],
    },
  ],
};

describe("OverviewPanel", () => {
  it("renders aggregated metrics from the snapshot", () => {
    const wrapper = mount(OverviewPanel, {
      props: {
        snapshot,
        loading: false,
        error: null,
        stale: true,
      },
    });

    const cards = wrapper.findAll(".metric-card");
    expect(cards).toHaveLength(5);
    expect(cards[0].text()).toContain("75.0%");
    expect(cards[1].text()).toContain("150.0 ms");
    expect(cards[2].text()).toContain("180 ms");
    expect(cards[3].text()).toContain("80.0%");
    expect(cards[4].text()).toContain("2");

    expect(wrapper.find(".overview-panel__badge").exists()).toBe(true);
    expect(wrapper.findAll(".metric-chart__legend-item").length).toBeGreaterThan(0);
  });

  it("shows a loading indicator before the first snapshot", () => {
    const wrapper = mount(OverviewPanel, {
      props: {
        snapshot: null,
        loading: true,
        error: null,
        stale: false,
      },
    });

    expect(wrapper.find(".loading-state").exists()).toBe(true);
    expect(wrapper.find(".metric-card").exists()).toBe(false);
  });

  it("renders an error message when loading fails", () => {
    const wrapper = mount(OverviewPanel, {
      props: {
        snapshot: null,
        loading: false,
        error: "加载失败",
        stale: false,
      },
    });

    const error = wrapper.find(".overview-panel__error");
    expect(error.exists()).toBe(true);
    expect(error.text()).toContain("加载失败");
  });

  it("shows the empty state when no data is available", () => {
    const wrapper = mount(OverviewPanel, {
      props: {
        snapshot: null,
        loading: false,
        error: null,
        stale: false,
      },
    });

    const empty = wrapper.find(".empty-state");
    expect(empty.exists()).toBe(true);
    expect(empty.text()).toContain("暂无指标数据");
  });
});
