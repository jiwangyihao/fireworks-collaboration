import type {
  CounterPoint,
  HistogramPoint,
  MetricsSnapshot,
  MetricsSnapshotSeries,
} from "../api/metrics";

export interface NormalizedPoint {
  x: number;
  y: number;
}

export function getSeries(snapshot: MetricsSnapshot | null, name: string): MetricsSnapshotSeries[] {
  if (!snapshot) {
    return [];
  }
  return snapshot.series.filter((series) => series.name === name);
}

export function filterSeries(
  series: MetricsSnapshotSeries[],
  labels: Record<string, string>,
): MetricsSnapshotSeries[] {
  return series.filter((entry) =>
    Object.entries(labels).every(([key, expected]) => entry.labels[key] === expected),
  );
}

export function groupByLabel(
  series: MetricsSnapshotSeries[],
  label: string,
): Record<string, MetricsSnapshotSeries[]> {
  return series.reduce<Record<string, MetricsSnapshotSeries[]>>((acc, entry) => {
    const key = entry.labels[label] ?? "unknown";
    if (!acc[key]) {
      acc[key] = [];
    }
    acc[key].push(entry);
    return acc;
  }, {});
}

export function labelValue(series: MetricsSnapshotSeries, label: string, fallback = "unknown"): string {
  return series.labels[label] ?? fallback;
}

export function sumCounter(series: MetricsSnapshotSeries): number {
  if (series.points && series.points.length > 0) {
    return series.points.reduce((total, point) => total + point.value, 0);
  }
  if (typeof series.value === "number") {
    return series.value;
  }
  return 0;
}

export function sumCounters(series: MetricsSnapshotSeries[]): number {
  return series.reduce((total, entry) => total + sumCounter(entry), 0);
}

export function toNormalizedCounterPoints(series: MetricsSnapshotSeries): NormalizedPoint[] {
  if (!series.points || series.points.length === 0) {
    return [];
  }
  return normalizePoints(series.points, (point) => point.value);
}

export function aggregatedCounterSeries(series: MetricsSnapshotSeries[]): NormalizedPoint[] {
  const merged = new Map<number, number>();
  for (const entry of series) {
    if (!entry.points) {
      continue;
    }
    for (const point of entry.points) {
      const existing = merged.get(point.offsetSeconds) ?? 0;
      merged.set(point.offsetSeconds, existing + point.value);
    }
  }
  const combined = Array.from(merged.entries())
    .map(([offsetSeconds, value]) => ({ offsetSeconds, value }))
    .sort((a, b) => a.offsetSeconds - b.offsetSeconds);
  if (combined.length === 0) {
    return [];
  }
  return normalizePoints(combined, (point) => point.value);
}

export function aggregatedHistogramSeries(seriesList: MetricsSnapshotSeries[]): NormalizedPoint[] {
  const merged = new Map<number, { sum: number; count: number }>();
  for (const series of seriesList) {
    if (!series.histogramPoints) {
      continue;
    }
    for (const point of series.histogramPoints) {
      const entry = merged.get(point.offsetSeconds) ?? { sum: 0, count: 0 };
      entry.sum += point.sum;
      entry.count += point.count;
      merged.set(point.offsetSeconds, entry);
    }
  }
  if (merged.size === 0) {
    return [];
  }
  const combined: HistogramPoint[] = Array.from(merged.entries())
    .map(([offsetSeconds, value]) => ({ offsetSeconds, sum: value.sum, count: value.count }))
    .sort((a, b) => a.offsetSeconds - b.offsetSeconds);
  return normalizeHistogramPoints(combined);
}

export function toNormalizedHistogramPoints(series: MetricsSnapshotSeries): NormalizedPoint[] {
  if (!series.histogramPoints || series.histogramPoints.length === 0) {
    return [];
  }
  return normalizeHistogramPoints(series.histogramPoints);
}

function normalizePoints(points: CounterPoint[], selectY: (point: CounterPoint) => number): NormalizedPoint[] {
  const sorted = [...points].sort((a, b) => a.offsetSeconds - b.offsetSeconds);
  const first = sorted[0]?.offsetSeconds ?? 0;
  const last = sorted[sorted.length - 1]?.offsetSeconds ?? 1;
  const span = Math.max(1, last - first);
  const denom = sorted.length > 1 ? sorted.length - 1 : 1;
  return sorted.map((point, idx) => {
    const normalizedX = span === 0 ? idx / denom : (point.offsetSeconds - first) / span;
    const normalizedY = selectY(point);
    return { x: normalizedX, y: normalizedY };
  });
}

function normalizeHistogramPoints(points: HistogramPoint[]): NormalizedPoint[] {
  const sorted = [...points].sort((a, b) => a.offsetSeconds - b.offsetSeconds);
  const first = sorted[0]?.offsetSeconds ?? 0;
  const last = sorted[sorted.length - 1]?.offsetSeconds ?? 1;
  const span = Math.max(1, last - first);
  const denom = sorted.length > 1 ? sorted.length - 1 : 1;
  return sorted.map((point, idx) => {
    const normalizedX = span === 0 ? idx / denom : (point.offsetSeconds - first) / span;
    const average = point.count > 0 ? point.sum / point.count : 0;
    return { x: normalizedX, y: average };
  });
}

export function averageHistogram(series: MetricsSnapshotSeries): number | null {
  if (series.histogramPoints && series.histogramPoints.length > 0) {
    const totals = series.histogramPoints.reduce(
      (acc, point) => {
        acc.count += point.count;
        acc.sum += point.sum;
        return acc;
      },
      { count: 0, sum: 0 },
    );
    return totals.count > 0 ? totals.sum / totals.count : null;
  }
  if (typeof series.count === "number" && typeof series.sum === "number" && series.count > 0) {
    return series.sum / series.count;
  }
  return null;
}

export function combinedHistogramTotals(seriesList: MetricsSnapshotSeries[]): { sum: number; count: number } {
  return seriesList.reduce(
    (acc, series) => {
      if (series.histogramPoints && series.histogramPoints.length > 0) {
        for (const point of series.histogramPoints) {
          acc.sum += point.sum;
          acc.count += point.count;
        }
      } else if (typeof series.sum === "number" && typeof series.count === "number") {
        acc.sum += series.sum;
        acc.count += series.count;
      }
      return acc;
    },
    { sum: 0, count: 0 },
  );
}

export function getQuantile(series: MetricsSnapshotSeries, key: string): number | null {
  if (!series.quantiles) {
    return null;
  }
  const value = series.quantiles[key];
  return typeof value === "number" ? value : null;
}

export function formatPercent(value: number | null, fractionDigits = 1): string {
  if (value === null || Number.isNaN(value)) {
    return "N/A";
  }
  return `${(value * 100).toFixed(fractionDigits)}%`;
}

export function formatDurationMs(value: number | null, fractionDigits = 0): string {
  if (value === null || Number.isNaN(value)) {
    return "N/A";
  }
  if (value >= 1_000) {
    return `${(value / 1_000).toFixed(fractionDigits)} s`;
  }
  return `${value.toFixed(fractionDigits)} ms`;
}

export function formatNumber(value: number | null, fractionDigits = 0): string {
  if (value === null || Number.isNaN(value)) {
    return "N/A";
  }
  return value.toLocaleString(undefined, {
    maximumFractionDigits: fractionDigits,
    minimumFractionDigits: fractionDigits,
  });
}

export function computeRate(numerator: number, denominator: number): number | null {
  if (denominator <= 0) {
    return null;
  }
  return numerator / denominator;
}
