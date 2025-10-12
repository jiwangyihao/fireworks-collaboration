import { describe, it, expect, vi, beforeEach } from "vitest";
import { setActivePinia, createPinia } from "pinia";

vi.mock("../../api/config", () => ({
  getConfig: vi.fn(),
  setConfig: vi.fn(),
}));

import { useConfigStore } from "../config";
import { getConfig, setConfig } from "../../api/config";

const fakeCfg = {
  http: {
    fakeSniEnabled: true,
    fakeSniHosts: ["baidu.com"],
    sniRotateOn403: true,
    followRedirects: true,
    maxRedirects: 5,
    largeBodyWarnBytes: 1024,
  },
  tls: {
    spkiPins: [],
    metricsEnabled: false,
    certFpLogEnabled: false,
    certFpMaxBytes: 4096,
  },
  logging: { authHeaderMasked: true, logLevel: "info" },
};

describe("stores/config", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    (getConfig as any).mockReset();
    (setConfig as any).mockReset();
  });

  it("refresh: 成功时应写入 cfg 且清空 error/loading", async () => {
    (getConfig as any).mockResolvedValueOnce(fakeCfg);
    const store = useConfigStore();
    await store.refresh();
    expect(store.cfg).toEqual(fakeCfg);
    expect(store.error).toBeNull();
    expect(store.loading).toBe(false);
  });

  it("refresh: 失败时应设置 error 且 cfg 保持 null", async () => {
    (getConfig as any).mockRejectedValueOnce(new Error("boom"));
    const store = useConfigStore();
    await store.refresh();
    expect(store.cfg).toBeNull();
    expect(String(store.error)).toContain("boom");
    expect(store.loading).toBe(false);
  });

  it("save: 成功应调用 API 并更新 cfg", async () => {
    (setConfig as any).mockResolvedValueOnce(undefined);
    const store = useConfigStore();
    await store.save(fakeCfg as any);
    expect(setConfig).toHaveBeenCalledWith(fakeCfg);
    expect(store.cfg).toEqual(fakeCfg);
    expect(store.error).toBeNull();
    expect(store.loading).toBe(false);
  });

  it("save: 失败应设置 error 并抛出", async () => {
    (setConfig as any).mockRejectedValueOnce(new Error("denied"));
    const store = useConfigStore();
    await expect(store.save(fakeCfg as any)).rejects.toThrow("denied");
    expect(String(store.error)).toContain("denied");
    expect(store.loading).toBe(false);
  });
});
