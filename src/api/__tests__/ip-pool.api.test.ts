import { describe, it, expect, vi, beforeEach, type Mock } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

import { invoke } from "@tauri-apps/api/core";
import {
  clearIpPoolAutoDisabled,
  getIpPoolSnapshot,
  pickIpPoolBest,
  requestIpPoolRefresh,
  startIpPoolPreheater,
  updateIpPoolConfig,
  type IpPoolSnapshot,
  type IpSelectionResult,
  type IpPoolPreheatActivation,
} from "../ip-pool";
import type { IpPoolRuntimeConfig, IpPoolFileConfig } from "../config";

const mockInvoke = invoke as unknown as Mock;

const runtimeConfig: IpPoolRuntimeConfig = {
  enabled: true,
  sources: {
    builtin: true,
    dns: true,
    history: true,
    userStatic: false,
    fallback: false,
  },
  dns: {
    useSystem: true,
    resolvers: [],
    presetCatalog: {},
    enabledPresets: [],
  },
  maxParallelProbes: 8,
  probeTimeoutMs: 1200,
  historyPath: null,
  cachePruneIntervalSecs: 45,
  maxCacheEntries: 500,
  singleflightTimeoutMs: 8_000,
  failureThreshold: 4,
  failureRateThreshold: 0.4,
  failureWindowSeconds: 90,
  minSamplesInWindow: 3,
  cooldownSeconds: 120,
  circuitBreakerEnabled: true,
};

const fileConfig: IpPoolFileConfig = {
  preheatDomains: [],
  scoreTtlSeconds: 600,
  userStatic: [],
  blacklist: [],
  whitelist: [],
  disabledBuiltinPreheat: [],
};

const snapshot: IpPoolSnapshot = {
  runtime: runtimeConfig,
  file: fileConfig,
  enabled: true,
  preheatEnabled: false,
  preheaterActive: false,
  preheatTargets: 0,
  preheatedTargets: 0,
  autoDisabledUntil: null,
  cacheEntries: [],
  trippedIps: [],
  timestampMs: 1_700_000_000_000,
};

const activation: IpPoolPreheatActivation = {
  enabled: true,
  preheatEnabled: true,
  preheaterActive: true,
  activationChanged: true,
  preheatTargets: 2,
};

const selection: IpSelectionResult = {
  host: "github.com",
  port: 443,
  strategy: "cached",
  cacheHit: true,
  selected: undefined,
  alternatives: [],
  outcome: {
    success: 10,
    failure: 2,
    lastOutcomeMs: 1_700_000_000_500,
  },
};

describe("api/ip-pool", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
  });

  it("getIpPoolSnapshot 调用 ip_pool_get_snapshot", async () => {
    mockInvoke.mockResolvedValueOnce(snapshot);

    const result = await getIpPoolSnapshot();

    expect(mockInvoke).toHaveBeenCalledWith("ip_pool_get_snapshot", undefined);
    expect(result).toEqual(snapshot);
  });

  it("updateIpPoolConfig 传递 runtime 与 file", async () => {
    mockInvoke.mockResolvedValueOnce(snapshot);

    const result = await updateIpPoolConfig(runtimeConfig, fileConfig);

    expect(mockInvoke).toHaveBeenCalledWith("ip_pool_update_config", {
      runtime: runtimeConfig,
      file: fileConfig,
    });
    expect(result).toEqual(snapshot);
  });

  it("requestIpPoolRefresh 返回布尔值", async () => {
    mockInvoke.mockResolvedValueOnce(true);

    const accepted = await requestIpPoolRefresh();

    expect(mockInvoke).toHaveBeenCalledWith("ip_pool_request_refresh", undefined);
    expect(accepted).toBe(true);
  });

  it("clearIpPoolAutoDisabled 返回布尔值", async () => {
    mockInvoke.mockResolvedValueOnce(false);

    const cleared = await clearIpPoolAutoDisabled();

    expect(mockInvoke).toHaveBeenCalledWith("ip_pool_clear_auto_disabled", undefined);
    expect(cleared).toBe(false);
  });

  it("startIpPoolPreheater 返回激活状态", async () => {
    mockInvoke.mockResolvedValueOnce(activation);

    const result = await startIpPoolPreheater();

    expect(mockInvoke).toHaveBeenCalledWith("ip_pool_start_preheater", undefined);
    expect(result).toEqual(activation);
  });

  it("pickIpPoolBest 传递 host 与 port", async () => {
    mockInvoke.mockResolvedValueOnce(selection);

    const result = await pickIpPoolBest("github.com", 443);

    expect(mockInvoke).toHaveBeenCalledWith("ip_pool_pick_best", {
      host: "github.com",
      port: 443,
    });
    expect(result).toEqual(selection);
  });
});
