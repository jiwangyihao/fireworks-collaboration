import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";
import MetricChart, { type ChartSeries } from "../observability/MetricChart.vue";

const baseSeries: ChartSeries[] = [
  {
    id: "completed",
    label: "完成",
    points: [
      { x: 0, y: 4 },
      { x: 0.5, y: 6 },
      { x: 1, y: 8 },
    ],
  },
];

describe("MetricChart", () => {
  it("renders polylines for provided series", () => {
    const wrapper = mount(MetricChart, {
      props: {
        series: baseSeries,
      },
    });

    const polyline = wrapper.find("polyline");
    expect(polyline.exists()).toBe(true);
    const pointsAttr = polyline.attributes("points");
    expect(pointsAttr?.split(" ").length).toBe(3);
    const legendItems = wrapper.findAll(".metric-chart__legend-item");
    expect(legendItems).toHaveLength(1);
    expect(legendItems[0].text()).toContain("完成");
  });

  it("extends single-point series so they render a visible line", () => {
    const singlePointSeries: ChartSeries[] = [
      {
        id: "single",
        label: "单点",
        points: [{ x: 0, y: 5 }],
      },
    ];

    const wrapper = mount(MetricChart, {
      props: {
        series: singlePointSeries,
      },
    });

    const polyline = wrapper.find("polyline");
    expect(polyline.exists()).toBe(true);
    const pointsAttr = polyline.attributes("points");
    expect(pointsAttr?.split(" ").length).toBe(2);
  });

  it("shows the fallback message when no data is present", () => {
    const wrapper = mount(MetricChart, {
      props: {
        series: [],
        emptyMessage: "暂无数据",
      },
    });

    const empty = wrapper.find(".metric-chart__empty");
    expect(empty.exists()).toBe(true);
    expect(empty.text()).toBe("暂无数据");
  });
});
