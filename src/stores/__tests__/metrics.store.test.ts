import { beforeEach, describe, expect, it, vi, type Mock } from "vitest";
import { createPinia, setActivePinia } from "pinia";
import { useMetricsStore, type MetricsQuery } from "../metrics";
import type { MetricsSnapshot } from "../../api/metrics";
import { fetchMetricsSnapshot } from "../../api/metrics";

vi.mock("../../api/metrics", () => ({
  fetchMetricsSnapshot: vi.fn(),
}));

const mockedFetch = fetchMetricsSnapshot as unknown as Mock;

function buildSnapshot(seed: number): MetricsSnapshot {
  return {
    generatedAtMs: seed,
    series: [],
  };
}

describe("metrics store", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2025-01-01T00:00:00Z"));
    mockedFetch.mockReset();
    setActivePinia(createPinia());
  });

  it("fetches and caches a snapshot", async () => {
    const store = useMetricsStore();
    const query: MetricsQuery = { names: ["git_tasks_total"], range: "5m", quantiles: [0.95] };
    const snapshot = buildSnapshot(1);
    mockedFetch.mockResolvedValueOnce(snapshot);

    const result = await store.ensure(query);

    expect(result).toStrictEqual(snapshot);
    const entry = store.getEntry(query);
    expect(entry?.snapshot).toStrictEqual(snapshot);
    expect(entry?.loading).toBe(false);
    expect(mockedFetch).toHaveBeenCalledTimes(1);
  });

  it("reuses cached data while still fresh", async () => {
    const store = useMetricsStore();
    const query: MetricsQuery = { names: ["git_tasks_total"], range: "5m" };
    const snapshot = buildSnapshot(2);
    mockedFetch.mockResolvedValueOnce(snapshot);

    await store.ensure(query);
    mockedFetch.mockClear();

    const second = await store.ensure(query);
    expect(second).toStrictEqual(snapshot);
    expect(mockedFetch).not.toHaveBeenCalled();
  });

  it("returns stale data while refreshing in the background", async () => {
    const store = useMetricsStore();
    const query: MetricsQuery = { names: ["git_tasks_total"], range: "5m" };
    const initial = buildSnapshot(3);
    mockedFetch.mockResolvedValueOnce(initial);
    await store.ensure(query);

  mockedFetch.mockClear();

    vi.setSystemTime(new Date(Date.now() + 11_000));

    let resolveFetch: ((value: MetricsSnapshot) => void) | undefined;
    const updated = buildSnapshot(4);
    mockedFetch.mockImplementationOnce(
      () =>
        new Promise<MetricsSnapshot>((resolve) => {
          resolveFetch = (value) => resolve(value);
        }),
    );

    const staleResult = await store.ensure(query);
    expect(staleResult).toStrictEqual(initial);
    const entryDuringRefresh = store.getEntry(query);
    expect(entryDuringRefresh?.refreshing).toBe(true);
    expect(mockedFetch).toHaveBeenCalledTimes(1);

    resolveFetch?.(updated);
    await flushAllPromises();

    const entryAfter = store.getEntry(query);
    expect(entryAfter?.snapshot).toStrictEqual(updated);
    expect(entryAfter?.refreshing).toBe(false);
  });
});

async function flushAllPromises() {
  await Promise.resolve();
  await Promise.resolve();
}
