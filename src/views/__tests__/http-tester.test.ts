import { describe, it, expect, vi, beforeEach } from "vitest";
import { mount, flushPromises } from "@vue/test-utils";
import { setActivePinia, createPinia } from "pinia";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

vi.mock("../../api/http", () => ({
  httpFakeRequest: vi.fn().mockResolvedValue({
    ok: true,
    status: 200,
    usedFakeSni: true,
    ip: null,
    timing: { connectMs: 1, tlsMs: 1, firstByteMs: 1, totalMs: 2 },
    headers: {},
    bodyBase64: "",
    redirects: [],
    bodySize: 0,
  }),
}));

vi.mock("../../api/config", () => ({
  getConfig: vi.fn().mockResolvedValue({ http: { fakeSniEnabled: true, fakeSniHost: "baidu.com", followRedirects: true, maxRedirects: 5, largeBodyWarnBytes: 1024 }, tls: { sanWhitelist: ["github.com"], insecureSkipVerify: false }, logging: { authHeaderMasked: true, logLevel: "info" } }),
  setConfig: vi.fn().mockResolvedValue(undefined),
}));

import { httpFakeRequest } from "../../api/http";
import { setConfig } from "../../api/config";
import HttpTester from "../HttpTester.vue";

describe("views/HttpTester", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    (httpFakeRequest as any).mockClear?.();
    (setConfig as any).mockClear?.();
  });

  it("发送请求后应记录到本地历史列表", async () => {
    const w = mount(HttpTester);
    await w.get("button.btn-primary").trigger("click");
    await flushPromises();
    expect(httpFakeRequest).toHaveBeenCalled();
    // 历史区域存在一条记录
    expect(w.text()).toContain("最近请求");
  });

  it("保存 HTTP 策略会调用 setConfig", async () => {
    const w = mount(HttpTester);
    const btns = w.findAll("button.btn");
    const saveBtn = btns.find((b) => /保存 HTTP 策略/.test(b.text()))!;
    await saveBtn.trigger("click");
    await flushPromises();
    expect(setConfig).toHaveBeenCalled();
  });

  it("点击历史条目应回填表单字段", async () => {
    const w = mount(HttpTester);
    // 先发送，产生历史
    await w.get("button.btn-primary").trigger("click");
    await flushPromises();
    // 修改输入
    await w.find("input.input").setValue("https://example.com/");
    // 点击历史第一条
    const firstHistory = w.find("ul.menu li a");
    await firstHistory.trigger("click");
    // 断言被回填为历史中的 github 默认值
    expect((w.find("input.input").element as HTMLInputElement).value).toContain("https://github.com/");
  });
});
