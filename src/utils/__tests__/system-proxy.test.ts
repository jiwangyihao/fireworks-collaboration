import { describe, it, expect, vi, beforeEach } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

import { invoke } from "@tauri-apps/api/core";
import {
  getSystemProxy,
  formatProxyAddress,
  isLocalProxy,
  getProxyTypeDescription,
  type SystemProxy,
} from "../system-proxy";

describe("utils/system-proxy", () => {
  beforeEach(() => {
    (invoke as any).mockReset();
  });

  it("getSystemProxy: 成功返回后端配置", async () => {
    const cfg: SystemProxy = {
      enabled: true,
      host: "127.0.0.1",
      port: 7890,
      bypass: "localhost",
    };
    (invoke as any).mockResolvedValueOnce(cfg);

    const res = await getSystemProxy();
    expect(invoke as any).toHaveBeenCalledWith("get_system_proxy");
    expect(res).toEqual(cfg);
  });

  it("getSystemProxy: 调用失败应回退到默认禁用配置", async () => {
    (invoke as any).mockRejectedValueOnce(new Error("boom"));
    const res = await getSystemProxy();
    expect(res).toEqual({ enabled: false, host: "", port: 0, bypass: "" });
  });

  it("format/isLocal/getProxyTypeDescription: 覆盖常见分支", () => {
    const disabled: SystemProxy = {
      enabled: false,
      host: "",
      port: 0,
      bypass: "",
    };
    expect(formatProxyAddress(disabled)).toBe("未启用");
    expect(isLocalProxy(disabled)).toBe(false);
    expect(getProxyTypeDescription(disabled)).toBe("未启用代理");

    const local: SystemProxy = {
      enabled: true,
      host: "localhost",
      port: 8080,
      bypass: "",
    };
    expect(formatProxyAddress(local)).toBe("localhost:8080");
    expect(isLocalProxy(local)).toBe(true);
    expect(getProxyTypeDescription(local)).toBe("本地代理服务");

    const remote: SystemProxy = {
      enabled: true,
      host: "10.0.0.2",
      port: 8080,
      bypass: "",
    };
    expect(isLocalProxy(remote)).toBe(false);
    expect(getProxyTypeDescription(remote)).toBe("远程代理服务");
  });
});
