import { describe, expect, it } from "vitest";
import {
  aggregatedCounterSeries,
  aggregatedHistogramSeries,
  averageHistogram,
  combinedHistogramTotals,
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
  const series = snapshot.series.find(
    (entry) => entry.labels.state === labelValueMatch
  );
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
    const completedSeries = filterSeries(
      getSeries(snapshot, "git_tasks_total"),
      { state: "completed" }
    );
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
    const totals = sumCounters(
      filterSeries(getSeries(snapshot, "git_tasks_total"), {
        state: "completed",
      })
    );
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
  const series = snapshot.series.find(
    (entry) => entry.labels.source === source
  );
  if (!series) {
    throw new Error(`latency series for ${source} missing`);
  }
  return series;
}

describe("observability utilities - extended", () => {
  it("getSeries returns empty array for null snapshot", () => {
    expect(getSeries(null, "any")).toEqual([]);
  });

  it("toNormalizedCounterPoints returns empty array for no points", () => {
    const emptySeries: MetricsSnapshotSeries = {
      name: "test",
      type: "counter",
      labels: {},
    };
    expect(toNormalizedCounterPoints(emptySeries)).toEqual([]);
  });

  it("toNormalizedHistogramPoints returns empty array for no histogramPoints", () => {
    const emptySeries: MetricsSnapshotSeries = {
      name: "test",
      type: "histogram",
      labels: {},
    };
    expect(toNormalizedHistogramPoints(emptySeries)).toEqual([]);
  });

  it("aggregatedCounterSeries returns empty for series without points", () => {
    const noPoints: MetricsSnapshotSeries[] = [
      { name: "test", type: "counter", labels: {} },
    ];
    expect(aggregatedCounterSeries(noPoints)).toEqual([]);
  });

  it("aggregatedHistogramSeries returns empty for series without histogramPoints", () => {
    const noPoints: MetricsSnapshotSeries[] = [
      { name: "test", type: "histogram", labels: {} },
    ];
    expect(aggregatedHistogramSeries(noPoints)).toEqual([]);
  });

  it("averageHistogram computes average from histogramPoints", () => {
    const series = latencySeries("cloudflare");
    // (40+75) / (2+3) = 115/5 = 23
    expect(averageHistogram(series)).toBeCloseTo(23);
  });

  it("averageHistogram uses sum/count fields when histogramPoints missing", () => {
    const series: MetricsSnapshotSeries = {
      name: "test",
      type: "histogram",
      labels: {},
      sum: 100,
      count: 4,
    };
    expect(averageHistogram(series)).toBe(25);
  });

  it("averageHistogram returns null when count is zero or missing", () => {
    const zeroCount: MetricsSnapshotSeries = {
      name: "test",
      type: "histogram",
      labels: {},
      sum: 100,
      count: 0,
    };
    expect(averageHistogram(zeroCount)).toBeNull();

    const noData: MetricsSnapshotSeries = {
      name: "test",
      type: "histogram",
      labels: {},
    };
    expect(averageHistogram(noData)).toBeNull();
  });

  it("combinedHistogramTotals aggregates from histogramPoints", () => {
    const latencySeries = getSeries(snapshot, "ip_pool_latency_ms");
    const totals = combinedHistogramTotals(latencySeries);
    // cloudflare: sum=40+75=115, count=2+3=5
    // azure: sum=35+45=80, count=1+1=2
    expect(totals.sum).toBe(195);
    expect(totals.count).toBe(7);
  });

  it("combinedHistogramTotals uses sum/count fields when histogramPoints missing", () => {
    const series: MetricsSnapshotSeries[] = [
      { name: "test", type: "histogram", labels: {}, sum: 50, count: 2 },
      { name: "test", type: "histogram", labels: {}, sum: 30, count: 3 },
    ];
    const totals = combinedHistogramTotals(series);
    expect(totals.sum).toBe(80);
    expect(totals.count).toBe(5);
  });

  it("getQuantile returns null when quantiles missing", () => {
    const series: MetricsSnapshotSeries = {
      name: "test",
      type: "histogram",
      labels: {},
    };
    expect(getQuantile(series, "p99")).toBeNull();
  });

  it("sumCounter falls back to value field when points missing", () => {
    const series: MetricsSnapshotSeries = {
      name: "test",
      type: "counter",
      labels: {},
      value: 42,
    };
    expect(sumCounter(series)).toBe(42);
  });

  it("sumCounter returns 0 when no points and no value", () => {
    const series: MetricsSnapshotSeries = {
      name: "test",
      type: "counter",
      labels: {},
    };
    expect(sumCounter(series)).toBe(0);
  });

  it("formatDurationMs handles sub-second values", () => {
    expect(formatDurationMs(500, 0)).toBe("500 ms");
    expect(formatDurationMs(null)).toBe("N/A");
    expect(formatDurationMs(NaN)).toBe("N/A");
  });

  it("formatNumber handles null and NaN", () => {
    expect(formatNumber(null)).toBe("N/A");
    expect(formatNumber(NaN)).toBe("N/A");
  });

  it("formatPercent handles NaN", () => {
    expect(formatPercent(NaN)).toBe("N/A");
  });

  it("labelValue returns fallback when label missing", () => {
    const series: MetricsSnapshotSeries = {
      name: "test",
      type: "counter",
      labels: { a: "1" },
    };
    expect(labelValue(series, "missing", "default")).toBe("default");
  });

  it("groupByLabel uses 'unknown' for missing labels", () => {
    const series: MetricsSnapshotSeries[] = [
      { name: "test", type: "counter", labels: {} },
    ];
    const grouped = groupByLabel(series, "missing");
    expect(grouped["unknown"]).toHaveLength(1);
  });
});
