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
export interface AppConfig {
  http: HttpCfg;
  tls: TlsCfg;
  logging: LoggingCfg;
  proxy: ProxyCfg;
}

export async function getConfig(): Promise<AppConfig> {
  return invoke<AppConfig>("get_config");
}

export async function setConfig(cfg: AppConfig): Promise<void> {
  return invoke<void>("set_config", { newCfg: cfg });
}
