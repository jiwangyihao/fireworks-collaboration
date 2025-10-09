import { describe, it, expect, vi, beforeEach, afterEach, type Mock, type MockInstance } from "vitest";
import { waitForIpPoolWarmup, extractProgress } from "../check-preheat";
import type { IpPoolSnapshot } from "../../api/ip-pool";
import type { IpPoolRuntimeConfig, IpPoolFileConfig } from "../../api/config";
import { getIpPoolSnapshot } from "../../api/ip-pool";

vi.mock("../../api/ip-pool", () => ({
  getIpPoolSnapshot: vi.fn(),
}));

const mockGetSnapshot = getIpPoolSnapshot as unknown as Mock;
let consoleWarnSpy: MockInstance;

describe("check-preheat helpers", () => {
  beforeEach(() => {
    mockGetSnapshot.mockReset();
    consoleWarnSpy = vi.spyOn(console, "warn").mockImplementation(() => {});
  });

  afterEach(() => {
    mockGetSnapshot.mockReset();
    consoleWarnSpy.mockRestore();
  });

  it("clamps completed targets when progress exceeds total", () => {
    const snapshot = buildSnapshot({ preheatTargets: 5, preheatedTargets: 9 });
    const progress = extractProgress(snapshot);
    expect(progress).toEqual({ totalTargets: 5, completedTargets: 5 });
  });

  it("returns disabled state when IP pool is disabled", async () => {
    mockGetSnapshot.mockResolvedValueOnce(buildSnapshot({ enabled: false }));
    await expect(waitForIpPoolWarmup()).resolves.toEqual({ state: "disabled" });
    expect(mockGetSnapshot).toHaveBeenCalledTimes(1);
  });

  it("returns inactive state when preheat is disabled", async () => {
    mockGetSnapshot.mockResolvedValueOnce(
      buildSnapshot({
        enabled: true,
        preheatEnabled: false,
        preheatTargets: 4,
        preheatedTargets: 1,
      }),
    );

    await expect(waitForIpPoolWarmup()).resolves.toEqual({
      state: "inactive",
      totalTargets: 4,
      completedTargets: 1,
    });
  });

  it("returns ready state once all targets complete", async () => {
    mockGetSnapshot.mockResolvedValueOnce(
      buildSnapshot({
        enabled: true,
        preheatEnabled: true,
        preheatTargets: 3,
        preheatedTargets: 3,
      }),
    );

    await expect(waitForIpPoolWarmup()).resolves.toEqual({
      state: "ready",
      totalTargets: 3,
      completedTargets: 3,
    });
  });

  it("returns pending state when attempts exhaust with active preheat", async () => {
    mockGetSnapshot.mockResolvedValue(
      buildSnapshot({
        enabled: true,
        preheatEnabled: true,
        preheatTargets: 5,
        preheatedTargets: 2,
      }),
    );

    await expect(waitForIpPoolWarmup(0, 2, 0)).resolves.toEqual({
      state: "pending",
      totalTargets: 5,
      completedTargets: 2,
    });
    expect(mockGetSnapshot).toHaveBeenCalledTimes(2);
  });

  it("falls back to inactive when snapshots never succeed", async () => {
    mockGetSnapshot.mockRejectedValue(new Error("network down"));

    await expect(waitForIpPoolWarmup(7, 2, 0)).resolves.toEqual({
      state: "inactive",
      totalTargets: 7,
      completedTargets: 0,
    });
    expect(mockGetSnapshot).toHaveBeenCalledTimes(2);
  });
});

function buildSnapshot(overrides: Partial<IpPoolSnapshot>): IpPoolSnapshot {
  const baseRuntime: IpPoolRuntimeConfig = {
    enabled: true,
    sources: {
      builtin: true,
      dns: true,
      history: true,
      userStatic: true,
      fallback: true,
    },
    dns: {
      useSystem: true,
      resolvers: [],
      presetCatalog: {},
      enabledPresets: [],
    },
    maxParallelProbes: 4,
    probeTimeoutMs: 1_000,
    historyPath: null,
    cachePruneIntervalSecs: 60,
    maxCacheEntries: 10,
    singleflightTimeoutMs: 1_000,
    failureThreshold: 3,
    failureRateThreshold: 0.5,
    failureWindowSeconds: 60,
    minSamplesInWindow: 1,
    cooldownSeconds: 30,
    circuitBreakerEnabled: true,
  };

  const baseFile: IpPoolFileConfig = {
    preheatDomains: [],
    scoreTtlSeconds: 300,
    userStatic: [],
    blacklist: [],
    whitelist: [],
    disabledBuiltinPreheat: [],
  };

  return {
    runtime: overrides.runtime ?? baseRuntime,
    file: overrides.file ?? baseFile,
    enabled: true,
    preheatEnabled: true,
    preheaterActive: false,
    preheatTargets: 1,
    preheatedTargets: 0,
    autoDisabledUntil: null,
    cacheEntries: [],
    trippedIps: [],
    timestampMs: 1,
    ...overrides,
  };
}
