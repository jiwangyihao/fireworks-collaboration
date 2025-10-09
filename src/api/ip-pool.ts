import { invoke } from "./tauri";
import type { IpPoolFileConfig, IpPoolRuntimeConfig } from "./config";

export type IpSource = "builtin" | "dns" | "history" | "userStatic" | "fallback";

export interface IpCandidate {
  address: string;
  port: number;
  source: IpSource;
}

export interface IpStat {
  candidate: IpCandidate;
  sources: IpSource[];
  latencyMs?: number;
  measuredAtEpochMs?: number;
  expiresAtEpochMs?: number;
  resolverMetadata?: string[];
}

export interface OutcomeMetrics {
  success: number;
  failure: number;
  lastOutcomeMs?: number | null;
}

export interface IpPoolCacheEntry {
  host: string;
  port: number;
  best?: IpStat;
  alternatives: IpStat[];
  outcome?: OutcomeMetrics;
}

export interface IpPoolSnapshot {
  runtime: IpPoolRuntimeConfig;
  file: IpPoolFileConfig;
  enabled: boolean;
  preheatEnabled: boolean;
  preheaterActive: boolean;
  preheatTargets: number;
  preheatedTargets: number;
  autoDisabledUntil?: number | null;
  cacheEntries: IpPoolCacheEntry[];
  trippedIps: string[];
  timestampMs: number;
}

export interface IpPoolPreheatActivation {
  enabled: boolean;
  preheatEnabled: boolean;
  preheaterActive: boolean;
  activationChanged: boolean;
  preheatTargets: number;
}

export interface IpSelectionResult {
  host: string;
  port: number;
  strategy: "system" | "cached";
  cacheHit: boolean;
  selected?: IpStat;
  alternatives: IpStat[];
  outcome?: OutcomeMetrics;
}

export function getIpPoolSnapshot(): Promise<IpPoolSnapshot> {
  return invoke<IpPoolSnapshot>("ip_pool_get_snapshot");
}

export function updateIpPoolConfig(
  runtime: IpPoolRuntimeConfig,
  file: IpPoolFileConfig,
): Promise<IpPoolSnapshot> {
  return invoke<IpPoolSnapshot>("ip_pool_update_config", { runtime, file });
}

export function requestIpPoolRefresh(): Promise<boolean> {
  return invoke<boolean>("ip_pool_request_refresh");
}

export function clearIpPoolAutoDisabled(): Promise<boolean> {
  return invoke<boolean>("ip_pool_clear_auto_disabled");
}

export function pickIpPoolBest(host: string, port: number): Promise<IpSelectionResult> {
  return invoke<IpSelectionResult>("ip_pool_pick_best", { host, port });
}

export function startIpPoolPreheater(): Promise<IpPoolPreheatActivation> {
  return invoke<IpPoolPreheatActivation>("ip_pool_start_preheater");
}
