import { invoke } from "./tauri";

export interface HttpCfg {
  fakeSniEnabled: boolean;
  fakeSniHost: string;
  followRedirects: boolean;
  maxRedirects: number;
  largeBodyWarnBytes: number;
}
export interface TlsCfg {
  sanWhitelist: string[];
  insecureSkipVerify?: boolean;
}
export interface LoggingCfg {
  authHeaderMasked: boolean;
  logLevel: string;
}
export interface AppConfig {
  http: HttpCfg;
  tls: TlsCfg;
  logging: LoggingCfg;
}

export async function getConfig(): Promise<AppConfig> {
  return invoke<AppConfig>("get_config");
}

export async function setConfig(cfg: AppConfig): Promise<void> {
  return invoke<void>("set_config", { newCfg: cfg });
}
