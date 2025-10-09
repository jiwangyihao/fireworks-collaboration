import { invoke } from "./tauri";

export interface HttpCfg {
  fakeSniEnabled: boolean;
  // 新增：多候选伪 SNI 列表（若存在将参与随机选择与403轮换）
  fakeSniHosts?: string[];
  // 新增：403 时自动轮换 SNI（仅 InfoRefs GET 阶段）
  sniRotateOn403?: boolean;
  // P3.1：渐进放量百分比（0..=100）
  fakeSniRolloutPercent?: number;
  followRedirects: boolean;
  maxRedirects: number;
  largeBodyWarnBytes: number;
}
export interface TlsCfg {
  sanWhitelist: string[];
  insecureSkipVerify?: boolean;
  skipSanWhitelist?: boolean;
  // P3.2：可观测性
  metricsEnabled?: boolean;
  certFpLogEnabled?: boolean;
  certFpMaxBytes?: number;
  // P3.3：Real-Host 验证
  realHostVerifyEnabled?: boolean;
}
export interface LoggingCfg {
  authHeaderMasked: boolean;
  logLevel: string;
}
export interface IpPoolSourceToggle {
  builtin: boolean;
  dns: boolean;
  history: boolean;
  userStatic: boolean;
  fallback: boolean;
}
export type DnsResolverProtocol = "udp" | "doh" | "dot";

export interface DnsResolverConfig {
  label: string;
  protocol: DnsResolverProtocol;
  endpoint: string;
  port?: number | null;
  bootstrapIps: string[];
  sni?: string | null;
  cacheSize?: number | null;
  desc?: string | null;
  presetKey?: string | null;
}

export interface DnsRuntimeConfig {
  useSystem: boolean;
  resolvers: DnsResolverConfig[];
  presetCatalog: Record<string, DnsResolverPreset>;
  enabledPresets: string[];
}

export interface DnsResolverPreset {
  server: string;
  type?: string;
  sni?: string;
  cacheSize?: number;
  desc?: string;
  forSNI?: boolean;
}
export interface IpPoolRuntimeConfig {
  enabled: boolean;
  sources: IpPoolSourceToggle;
  dns: DnsRuntimeConfig;
  maxParallelProbes: number;
  probeTimeoutMs: number;
  historyPath?: string | null;
  cachePruneIntervalSecs: number;
  maxCacheEntries: number;
  singleflightTimeoutMs: number;
  failureThreshold: number;
  failureRateThreshold: number;
  failureWindowSeconds: number;
  minSamplesInWindow: number;
  cooldownSeconds: number;
  circuitBreakerEnabled: boolean;
}
export interface PreheatDomain {
  host: string;
  ports: number[];
}
export interface UserStaticIp {
  host: string;
  ip: string;
  ports: number[];
}
export interface IpPoolFileConfig {
  preheatDomains: PreheatDomain[];
  scoreTtlSeconds: number;
  userStatic: UserStaticIp[];
  blacklist: string[];
  whitelist: string[];
  disabledBuiltinPreheat: string[];
}
export interface ProxyCfg {
  mode: 'off' | 'http' | 'socks5' | 'system';
  url: string;
  username?: string;
  password?: string;
  disableCustomTransport: boolean;
  timeoutSeconds: number;
  fallbackThreshold: number;
  fallbackWindowSeconds: number;
  recoveryCooldownSeconds: number;
  healthCheckIntervalSeconds: number;
  recoveryStrategy: string;
  probeUrl: string;
  probeTimeoutSeconds: number;
  recoveryConsecutiveThreshold: number;
  debugProxyLogging: boolean;
}
export interface ObservabilityExportConfig {
  authToken?: string | null;
  rateLimitQps: number;
  maxSeriesPerSnapshot: number;
  bindAddress: string;
}
export interface ObservabilityConfig {
  enabled: boolean;
  basicEnabled: boolean;
  aggregateEnabled: boolean;
  exportEnabled: boolean;
  uiEnabled: boolean;
  alertsEnabled: boolean;
  export: ObservabilityExportConfig;
}
export interface AppConfig {
  http: HttpCfg;
  tls: TlsCfg;
  logging: LoggingCfg;
  ipPool: IpPoolRuntimeConfig;
  proxy: ProxyCfg;
  observability?: ObservabilityConfig;
}

export type SectionStrategy = "overwrite" | "keepLocal" | "merge";

export interface TemplateExportOptions {
  includeIpPool?: boolean;
  includeIpPoolFile?: boolean;
  includeProxy?: boolean;
  includeTls?: boolean;
  includeCredential?: boolean;
  metadata?: Record<string, unknown>;
}

export interface ImportStrategyConfig {
  ipPool?: SectionStrategy;
  ipPoolFile?: SectionStrategy;
  proxy?: SectionStrategy;
  tls?: SectionStrategy;
  credential?: SectionStrategy;
}

export interface TemplateImportOptions {
  includeIpPool?: boolean;
  includeIpPoolFile?: boolean;
  includeProxy?: boolean;
  includeTls?: boolean;
  includeCredential?: boolean;
  strategies?: ImportStrategyConfig;
}

export type TemplateSectionKind =
  | "ipPoolRuntime"
  | "ipPoolFile"
  | "proxy"
  | "tls"
  | "credential";

export interface AppliedSection {
  section: TemplateSectionKind;
  strategy: SectionStrategy;
}

export interface SkippedSection {
  section: TemplateSectionKind;
  reason: string;
}

export interface TemplateImportReport {
  schemaVersion: string;
  applied: AppliedSection[];
  skipped: SkippedSection[];
  warnings: string[];
  backupPath?: string;
}

export async function getConfig(): Promise<AppConfig> {
  return invoke<AppConfig>("get_config");
}

export async function setConfig(cfg: AppConfig): Promise<void> {
  return invoke<void>("set_config", { newCfg: cfg });
}

export async function exportTeamConfigTemplate(
  destination?: string,
  options?: TemplateExportOptions,
): Promise<string> {
  const payload: Record<string, unknown> = {};
  if (destination) payload.destination = destination;
  if (options) payload.options = options;
  return invoke<string>("export_team_config_template", payload);
}

export async function importTeamConfigTemplate(
  source?: string,
  options?: TemplateImportOptions,
): Promise<TemplateImportReport> {
  const payload: Record<string, unknown> = {};
  if (source) payload.source = source;
  if (options) payload.options = options;
  return invoke<TemplateImportReport>("import_team_config_template", payload);
}
