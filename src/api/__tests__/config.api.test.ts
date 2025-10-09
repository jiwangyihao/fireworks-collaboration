import { describe, it, expect, vi, beforeEach } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

import { invoke } from "@tauri-apps/api/core";
import { getConfig, setConfig, type AppConfig } from "../config";

const fakeCfg = {
  http: {
    fakeSniEnabled: true,
    followRedirects: true,
    maxRedirects: 5,
    largeBodyWarnBytes: 1024,
  },
  tls: { sanWhitelist: ["github.com", "*.github.com"] },
  logging: { authHeaderMasked: true, logLevel: "info" },
} as unknown as AppConfig;

describe("api/config", () => {
  beforeEach(() => {
    (invoke as any).mockReset();
  });

  it("getConfig 调用 get_config 命令并返回配置", async () => {
    (invoke as any).mockResolvedValueOnce(fakeCfg);
    const cfg = await getConfig();
    expect(invoke as any).toHaveBeenCalledWith("get_config", undefined);
    expect(cfg).toEqual(fakeCfg);
  });

  it("setConfig 调用 set_config 命令并传递 newCfg", async () => {
    (invoke as any).mockResolvedValueOnce(undefined);
    await setConfig(fakeCfg);
    expect(invoke as any).toHaveBeenCalledWith("set_config", {
      newCfg: fakeCfg,
    });
  });
});
