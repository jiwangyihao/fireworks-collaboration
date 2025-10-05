import { fetch as tauriFetch } from "@tauri-apps/plugin-http";
import { invoke } from "./tauri";

export type MetricsRange = "1m" | "5m" | "1h" | "24h";

export interface MetricsSnapshot {
  generatedAtMs: number;
  series: MetricsSnapshotSeries[];
}

export interface MetricsSnapshotSeries {
  name: string;
  type: "counter" | "histogram" | "gauge";
  labels: Record<string, string>;
  value?: number;
  sum?: number;
  count?: number;
  buckets?: HistogramBucket[];
  quantiles?: Record<string, number>;
  points?: CounterPoint[];
  histogramPoints?: HistogramPoint[];
  range?: string;
  rawSamples?: HistogramSample[];
}

export interface HistogramBucket {
  le: string;
  c: number;
}

export interface CounterPoint {
  offsetSeconds: number;
  value: number;
}

export interface HistogramPoint {
  offsetSeconds: number;
  count: number;
  sum: number;
}

export interface HistogramSample {
  offsetSeconds: number;
  value: number;
}

export interface MetricsSnapshotRequest {
  names?: string[];
  range?: MetricsRange;
  quantiles?: number[];
  maxSeries?: number;
}

const DEFAULT_BASE_URL = "http://127.0.0.1:9688";

export async function fetchMetricsSnapshot(
  request: MetricsSnapshotRequest,
): Promise<MetricsSnapshot> {
  try {
    return await fetchViaHttp(request);
  } catch (networkErr) {
    console.debug("metrics snapshot via HTTP failed, fallback to invoke", networkErr);
    return fetchViaCommand(request);
  }
}

async function fetchViaHttp(request: MetricsSnapshotRequest): Promise<MetricsSnapshot> {
  const url = new URL("/metrics/snapshot", DEFAULT_BASE_URL);
  const names = sanitizeNames(request.names);
  if (names.length > 0) {
    url.searchParams.set("names", names.join(","));
  }
  const range = request.range;
  if (range) {
    url.searchParams.set("range", range);
  }
  const quantiles = sanitizeQuantiles(request.quantiles);
  if (quantiles.length > 0) {
    url.searchParams.set("quantiles", quantiles.join(","));
  }
  const response = await tauriFetch(url.toString(), { method: "GET" });
  if (!response.ok) {
    const reason = await safeText(response);
    throw new Error(
      `metrics exporter returned ${response.status} ${response.statusText}: ${reason}`,
    );
  }
  const payload = await safeText(response);
  try {
    return JSON.parse(payload) as MetricsSnapshot;
  } catch (err) {
    throw new Error(`failed to parse metrics snapshot payload: ${err}`);
  }
}

async function fetchViaCommand(request: MetricsSnapshotRequest): Promise<MetricsSnapshot> {
  const payload: Record<string, unknown> = {};
  const names = sanitizeNames(request.names);
  if (names.length > 0) {
    payload.names = names;
  }
  if (request.range) {
    payload.range = request.range;
  }
  const quantiles = sanitizeQuantiles(request.quantiles);
  if (quantiles.length > 0) {
    payload.quantiles = quantiles;
  }
  if (typeof request.maxSeries === "number") {
    payload.maxSeries = request.maxSeries;
  }
  return invoke<MetricsSnapshot>("metrics_snapshot", { options: payload });
}

function sanitizeNames(names?: string[]): string[] {
  if (!names || names.length === 0) {
    return [];
  }
  return Array.from(new Set(names.map((name) => name.trim()).filter((name) => name.length > 0))).sort();
}

function sanitizeQuantiles(values?: number[]): number[] {
  if (!values || values.length === 0) {
    return [];
  }
  const filtered = values
    .filter((value) => Number.isFinite(value) && value > 0 && value < 1)
    .sort((a, b) => a - b);
  const deduped: number[] = [];
  for (const value of filtered) {
    if (deduped.length === 0 || Math.abs(value - deduped[deduped.length - 1]) > Number.EPSILON) {
      deduped.push(value);
    }
  }
  return deduped;
}

type TauriResponse = Awaited<ReturnType<typeof tauriFetch>>;

async function safeText(response: TauriResponse): Promise<string> {
  try {
    return await response.text();
  } catch (err) {
    return `failed to read body: ${err}`;
  }
}
