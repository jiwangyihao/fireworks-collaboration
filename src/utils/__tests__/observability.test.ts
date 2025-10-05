import { describe, expect, it } from "vitest";
import {
  aggregatedCounterSeries,
  aggregatedHistogramSeries,
  computeRate,
  filterSeries,
  formatDurationMs,
  formatNumber,
  formatPercent,
  getQuantile,
  getSeries,
  groupByLabel,
  labelValue,
  sumCounter,
  sumCounters,
  toNormalizedCounterPoints,
  toNormalizedHistogramPoints,
} from "../observability";
import type { MetricsSnapshot, MetricsSnapshotSeries } from "../../api/metrics";

const snapshot: MetricsSnapshot = {
  generatedAtMs: Date.now(),
  series: [
    {
      name: "git_tasks_total",
      type: "counter",
      labels: { state: "completed", kind: "GitClone" },
      points: [
        { offsetSeconds: 0, value: 2 },
        { offsetSeconds: 30, value: 3 },
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
      name: "git_tasks_total",
      type: "counter",
      labels: { state: "completed", kind: "GitFetch" },
      points: [
        { offsetSeconds: 0, value: 5 },
        { offsetSeconds: 30, value: 7 },
      ],
    },
    {
      name: "ip_pool_latency_ms",
      type: "histogram",
      labels: { source: "cloudflare" },
      histogramPoints: [
        { offsetSeconds: 0, count: 2, sum: 40 },
        { offsetSeconds: 30, count: 3, sum: 75 },
      ],
    },
    {
      name: "ip_pool_latency_ms",
      type: "histogram",
      labels: { source: "azure" },
      histogramPoints: [
        { offsetSeconds: 0, count: 1, sum: 35 },
        { offsetSeconds: 30, count: 1, sum: 45 },
      ],
    },
    {
      name: "tls_handshake_ms",
      type: "histogram",
      labels: { outcome: "ok", sni_strategy: "fake" },
      histogramPoints: [{ offsetSeconds: 0, count: 1, sum: 120 }],
      quantiles: { p95: 150 },
    },
  ],
};

function seriesBy(labelValueMatch: string): MetricsSnapshotSeries {
  const series = snapshot.series.find((entry) => entry.labels.state === labelValueMatch);
  if (!series) {
    throw new Error("series not found");
  }
  return series;
}

describe("observability utilities", () => {
  it("filters snapshot series by name and labels", () => {
    const series = getSeries(snapshot, "git_tasks_total");
    expect(series).toHaveLength(3);

    const completed = filterSeries(series, { state: "completed" });
    expect(completed).toHaveLength(2);

    const grouped = groupByLabel(series, "kind");
    expect(Object.keys(grouped)).toEqual(["GitClone", "GitFetch"]);
  });

  it("aggregates counter series and preserves ordering", () => {
    const completedSeries = filterSeries(getSeries(snapshot, "git_tasks_total"), { state: "completed" });
    const aggregated = aggregatedCounterSeries(completedSeries);

    expect(aggregated).toHaveLength(2);
    expect(aggregated[0].x).toBe(0);
    expect(aggregated[0].y).toBe(7); // 2 + 5
    expect(aggregated[1].y).toBe(10); // 3 + 7
  });

  it("normalizes single counter series without aggregation", () => {
    const series = seriesBy("failed");
    const normalized = toNormalizedCounterPoints(series);
    expect(normalized).toHaveLength(2);
    expect(normalized[0]).toEqual({ x: 0, y: 1 });
    expect(normalized[1].x).toBeCloseTo(1);
  });

  it("aggregates histogram series into averages", () => {
    const latency = getSeries(snapshot, "ip_pool_latency_ms");
    const aggregated = aggregatedHistogramSeries(latency);
    expect(aggregated).toHaveLength(2);
    // (40 + 35) / (2 + 1) = 25, (75 + 45) / (3 + 1) = 30
    expect(aggregated[0].y).toBeCloseTo(25);
    expect(aggregated[1].y).toBeCloseTo(30);
  });

  it("normalizes histogram points per series", () => {
    const azure = latencySeries("azure");
    const normalized = toNormalizedHistogramPoints(azure);
    expect(normalized).toHaveLength(2);
    expect(normalized[0].y).toBeCloseTo(35);
    expect(normalized[1].y).toBeCloseTo(45);
  });

  it("summarizes counters and quantiles", () => {
    const series = seriesBy("completed");
    expect(sumCounter(series)).toBe(5);
    const totals = sumCounters(filterSeries(getSeries(snapshot, "git_tasks_total"), { state: "completed" }));
  expect(totals).toBe(17);
    const quantile = getQuantile(snapshot.series[5], "p95");
    expect(quantile).toBe(150);
    expect(labelValue(series, "kind")).toBe("GitClone");
  });

  it("formats metrics and rates", () => {
    expect(formatPercent(0.456)).toBe("45.6%");
    expect(formatPercent(null)).toBe("N/A");
    expect(formatDurationMs(1234, 1)).toBe("1.2 s");
    expect(formatNumber(12345)).toBe("12,345");
    expect(computeRate(3, 0)).toBeNull();
    expect(computeRate(3, 4)).toBeCloseTo(0.75);
  });
});

function latencySeries(source: string): MetricsSnapshotSeries {
  const series = snapshot.series.find((entry) => entry.labels.source === source);
  if (!series) {
    throw new Error(`latency series for ${source} missing`);
  }
  return series;
}
