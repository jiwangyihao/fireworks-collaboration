import { defineStore } from "pinia";
import {
  fetchMetricsSnapshot,
  type MetricsRange,
  type MetricsSnapshot,
  type MetricsSnapshotRequest,
} from "../api/metrics";

export interface MetricsQuery {
  names: string[];
  range: MetricsRange;
  quantiles?: number[];
  maxSeries?: number;
}

export interface MetricsCacheEntry {
  key: string;
  snapshot: MetricsSnapshot | null;
  fetchedAt: number;
  error: string | null;
  loading: boolean;
  refreshing: boolean;
  inflight: Promise<MetricsSnapshot> | null;
  range: MetricsRange;
  namesKey: string;
  quantilesKey: string;
}

interface EnsureOptions {
  force?: boolean;
}

const RANGE_TTL_MS: Record<MetricsRange, number> = {
  "1m": 5_000,
  "5m": 10_000,
  "1h": 30_000,
  "24h": 120_000,
};

const DEFAULT_TTL_MS = 30_000;

function keyFor(query: MetricsQuery): string {
  const namesKey = normalizeNames(query.names).join("|");
  const quantilesKey = normalizeQuantiles(query.quantiles).join("|");
  const maxSeriesKey = typeof query.maxSeries === "number" ? `#${query.maxSeries}` : "";
  return `${query.range}::${namesKey}::${quantilesKey}${maxSeriesKey}`;
}

function normalizeNames(names?: string[]): string[] {
  if (!names || names.length === 0) {
    return [];
  }
  return Array.from(new Set(names.map((name) => name.trim()).filter((name) => name.length > 0))).sort();
}

function normalizeQuantiles(values?: number[]): number[] {
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

export const useMetricsStore = defineStore("metrics", {
  state: () => ({
    entries: {} as Record<string, MetricsCacheEntry>,
  }),
  getters: {
    getEntry: (state) => (query: MetricsQuery | string) => {
      const key = typeof query === "string" ? query : keyFor(query);
      return state.entries[key];
    },
  },
  actions: {
    async ensure(query: MetricsQuery, options?: EnsureOptions): Promise<MetricsSnapshot> {
      const key = keyFor(query);
      const now = Date.now();
      let entry = this.entries[key];
      if (!entry) {
        entry = {
          key,
          snapshot: null,
          fetchedAt: 0,
          error: null,
          loading: false,
          refreshing: false,
          inflight: null,
          range: query.range,
          namesKey: normalizeNames(query.names).join("|"),
          quantilesKey: normalizeQuantiles(query.quantiles).join("|"),
        };
        this.entries[key] = entry;
      }

      const ttl = RANGE_TTL_MS[query.range] ?? DEFAULT_TTL_MS;
      const hasSnapshot = entry.snapshot !== null;
      const isFresh = hasSnapshot && now - entry.fetchedAt <= ttl;
      const force = options?.force === true;

      if (hasSnapshot && isFresh && !force) {
        return entry.snapshot as MetricsSnapshot;
      }

      const shouldBackgroundRefresh = hasSnapshot && !isFresh && !force;

      if (entry.inflight) {
        if (shouldBackgroundRefresh) {
          return entry.snapshot as MetricsSnapshot;
        }
        return entry.inflight;
      }

      const payload: MetricsSnapshotRequest = {
        names: query.names,
        range: query.range,
        quantiles: query.quantiles,
        maxSeries: query.maxSeries,
      };

      const promise = fetchMetricsSnapshot(payload)
        .then((snapshot) => {
          entry.snapshot = snapshot;
          entry.fetchedAt = Date.now();
          entry.error = null;
          entry.loading = false;
          entry.refreshing = false;
          return snapshot;
        })
        .catch((err) => {
          const message = err instanceof Error ? err.message : String(err);
          entry.error = message;
          entry.loading = false;
          entry.refreshing = false;
          if (entry.snapshot) {
            return entry.snapshot;
          }
          throw err;
        })
        .finally(() => {
          entry.inflight = null;
        });

      entry.inflight = promise;
      entry.loading = !shouldBackgroundRefresh && !hasSnapshot;
      entry.refreshing = shouldBackgroundRefresh && hasSnapshot;
      entry.error = shouldBackgroundRefresh ? entry.error : null;

      if (shouldBackgroundRefresh && entry.snapshot) {
        return entry.snapshot;
      }

      return promise;
    },
    isStale(query: MetricsQuery | string): boolean {
      const entry = this.getEntry(query);
      if (!entry || !entry.snapshot) {
        return true;
      }
      const ttl = RANGE_TTL_MS[entry.range] ?? DEFAULT_TTL_MS;
      return Date.now() - entry.fetchedAt > ttl;
    },
    clear() {
      this.entries = {};
    },
  },
});
