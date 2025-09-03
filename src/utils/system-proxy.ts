import { invoke } from "@tauri-apps/api/core";

export interface SystemProxy {
  enabled: boolean;
  host: string;
  port: number;
  bypass: string;
}

/**
 * 获取系统代理设置
 * @returns Promise<SystemProxy> 系统代理配置信息
 */
export async function getSystemProxy(): Promise<SystemProxy> {
  try {
    return await invoke<SystemProxy>("get_system_proxy");
  } catch (error) {
    console.error("获取系统代理设置失败:", error);
    return {
      enabled: false,
      host: "",
      port: 0,
      bypass: "",
    };
  }
}

/**
 * 格式化代理服务器地址
 * @param proxy 系统代理配置
 * @returns 格式化的代理地址字符串
 */
export function formatProxyAddress(proxy: SystemProxy): string {
  if (!proxy.enabled || !proxy.host) {
    return "未启用";
  }
  return `${proxy.host}:${proxy.port}`;
}

/**
 * 检查代理是否为常见的本地代理
 * @param proxy 系统代理配置
 * @returns 是否为本地代理
 */
export function isLocalProxy(proxy: SystemProxy): boolean {
  if (!proxy.enabled || !proxy.host) {
    return false;
  }

  const localHosts = ["127.0.0.1", "localhost", "::1"];
  return localHosts.includes(proxy.host.toLowerCase());
}

/**
 * 获取代理类型描述
 * @param proxy 系统代理配置
 * @returns 代理类型描述
 */
export function getProxyTypeDescription(proxy: SystemProxy): string {
  if (!proxy.enabled) {
    return "未启用代理";
  }

  if (isLocalProxy(proxy)) {
    return "本地代理服务";
  }

  return "远程代理服务";
}
