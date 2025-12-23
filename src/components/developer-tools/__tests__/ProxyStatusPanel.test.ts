import { describe, it, expect, beforeEach, vi } from "vitest";
import { mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import ProxyStatusPanel from "../ProxyStatusPanel.vue";
import { useConfigStore } from "../../../stores/config";

// Mock Tauri APIs
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));

// Helper to create a wrapper with event handler access
async function createWrapperWithEventHandler() {
  const { listen } = await import("@tauri-apps/api/event");
  const mockListen = listen as ReturnType<typeof vi.fn>;

  let eventHandler: any;
  mockListen.mockImplementation((eventName, handler) => {
    if (eventName === "proxy://state") {
      eventHandler = handler;
    }
    return Promise.resolve(() => {});
  });

  const wrapper = mount(ProxyStatusPanel);
  await wrapper.vm.$nextTick();

  return { wrapper, eventHandler };
}

describe("ProxyStatusPanel.vue", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.clearAllMocks();
  });

  it("renders status panel", () => {
    const wrapper = mount(ProxyStatusPanel);
    expect(wrapper.find("h3").text()).toBe("代理状态");
    expect(wrapper.find(".status-grid").exists()).toBe(true);
  });

  it("displays proxy mode from config", () => {
    const configStore = useConfigStore();
    configStore.cfg = {
      proxy: {
        mode: "http",
        url: "http://proxy.example.com:8080",
        disableCustomTransport: false,
        healthCheckUrl: "https://www.google.com",
        healthCheckIntervalSec: 60,
        healthCheckTimeoutSec: 10,
        fallbackAfterFailures: 3,
        recoverAfterSuccesses: 2,
        fallbackCooldownSec: 300,
        debugProxyLogging: false,
      },
    } as any;

    const wrapper = mount(ProxyStatusPanel);
    expect(wrapper.text()).toContain("HTTP/HTTPS");
  });

  it("displays disabled state when mode is off", () => {
    const configStore = useConfigStore();
    configStore.cfg = {
      proxy: {
        mode: "off",
      },
    } as any;

    const wrapper = mount(ProxyStatusPanel);
    expect(wrapper.text()).toContain("已禁用");
  });

  it("shows proxy URL when configured", () => {
    const configStore = useConfigStore();
    configStore.cfg = {
      proxy: {
        mode: "http",
        url: "http://proxy.example.com:8080",
      },
    } as any;

    const wrapper = mount(ProxyStatusPanel);
    expect(wrapper.text()).toContain("proxy.example.com");
  });

  it("sanitizes proxy URL to hide credentials", () => {
    const configStore = useConfigStore();
    configStore.cfg = {
      proxy: {
        mode: "http",
        url: "http://user:pass@proxy.example.com:8080",
      },
    } as any;

    const wrapper = mount(ProxyStatusPanel);
    // Should show host but not credentials
    expect(wrapper.text()).toContain("proxy.example.com");
    expect(wrapper.text()).not.toContain("user");
    expect(wrapper.text()).not.toContain("pass");
  });

  it("shows fallback button when state is enabled", async () => {
    const { wrapper, eventHandler } = await createWrapperWithEventHandler();

    // Trigger event to set state to enabled
    eventHandler({
      payload: {
        proxy_state: "Enabled",
      },
    });
    await wrapper.vm.$nextTick();

    const fallbackBtn = wrapper.find(".fallback-btn");
    expect(fallbackBtn.exists()).toBe(true);
    expect(fallbackBtn.text()).toContain("强制降级");
  });

  it("shows recovery button when state is fallback", async () => {
    const { wrapper, eventHandler } = await createWrapperWithEventHandler();

    // Trigger event to set state to fallback
    eventHandler({
      payload: {
        proxy_state: "Fallback",
      },
    });
    await wrapper.vm.$nextTick();

    const recoveryBtn = wrapper.find(".recovery-btn");
    expect(recoveryBtn.exists()).toBe(true);
    expect(recoveryBtn.text()).toContain("强制恢复");
  });

  it("calls force_proxy_fallback on fallback button click", async () => {
    const { invoke } = await import("@tauri-apps/api/core");
    const mockInvoke = invoke as ReturnType<typeof vi.fn>;
    mockInvoke.mockResolvedValue(undefined);

    const { wrapper, eventHandler } = await createWrapperWithEventHandler();

    // Set state to enabled
    eventHandler({
      payload: {
        proxy_state: "Enabled",
      },
    });
    await wrapper.vm.$nextTick();

    const fallbackBtn = wrapper.find(".fallback-btn");
    await fallbackBtn.trigger("click");

    expect(mockInvoke).toHaveBeenCalledWith("force_proxy_fallback", {
      reason: "用户手动触发降级",
    });
  });

  it("calls force_proxy_recovery on recovery button click", async () => {
    const { invoke } = await import("@tauri-apps/api/core");
    const mockInvoke = invoke as ReturnType<typeof vi.fn>;
    mockInvoke.mockResolvedValue(undefined);

    const { wrapper, eventHandler } = await createWrapperWithEventHandler();

    // Set state to fallback
    eventHandler({
      payload: {
        proxy_state: "Fallback",
      },
    });
    await wrapper.vm.$nextTick();

    const recoveryBtn = wrapper.find(".recovery-btn");
    await recoveryBtn.trigger("click");

    expect(mockInvoke).toHaveBeenCalledWith("force_proxy_recovery");
  });

  it("displays fallback reason when in fallback state", async () => {
    const { wrapper, eventHandler } = await createWrapperWithEventHandler();

    // Set state to fallback with reason
    eventHandler({
      payload: {
        proxy_state: "Fallback",
        fallback_reason: "连接超时",
      },
    });
    await wrapper.vm.$nextTick();

    expect(wrapper.text()).toContain("连接超时");
  });

  it("displays failure count in fallback state", async () => {
    const { wrapper, eventHandler } = await createWrapperWithEventHandler();

    // Set state to fallback with failure count
    eventHandler({
      payload: {
        proxy_state: "Fallback",
        failure_count: 5,
      },
    });
    await wrapper.vm.$nextTick();

    expect(wrapper.text()).toContain("失败次数");
    expect(wrapper.text()).toContain("5");
  });

  it("shows health check stats when available", async () => {
    const { wrapper, eventHandler } = await createWrapperWithEventHandler();

    // Set health check success rate
    eventHandler({
      payload: {
        health_check_success_rate: 0.85,
      },
    });
    await wrapper.vm.$nextTick();

    expect(wrapper.text()).toContain("健康检查成功率");
    expect(wrapper.text()).toContain("85.0%");
  });

  it("shows next health check countdown in recovering state", async () => {
    const { wrapper, eventHandler } = await createWrapperWithEventHandler();

    // Set state to recovering with countdown
    eventHandler({
      payload: {
        proxy_state: "Recovering",
        next_health_check_in: 30,
      },
    });
    await wrapper.vm.$nextTick();

    expect(wrapper.text()).toContain("下次健康检查");
    expect(wrapper.text()).toContain("30秒后");
  });

  it("displays custom transport status", () => {
    const configStore = useConfigStore();
    configStore.cfg = {
      proxy: {
        mode: "http",
        disableCustomTransport: true,
      },
    } as any;

    const wrapper = mount(ProxyStatusPanel);
    expect(wrapper.text()).toContain("已禁用");
  });

  it("listens for proxy state events on mount", async () => {
    const { listen } = await import("@tauri-apps/api/event");
    const mockListen = listen as ReturnType<typeof vi.fn>;

    mount(ProxyStatusPanel);

    expect(mockListen).toHaveBeenCalledWith(
      "proxy://state",
      expect.any(Function)
    );
  });

  it("updates state from proxy event", async () => {
    const { listen } = await import("@tauri-apps/api/event");
    const mockListen = listen as ReturnType<typeof vi.fn>;

    let eventHandler: any;
    mockListen.mockImplementation((eventName, handler) => {
      if (eventName === "proxy://state") {
        eventHandler = handler;
      }
      return Promise.resolve(() => {});
    });

    const wrapper = mount(ProxyStatusPanel);
    await wrapper.vm.$nextTick();

    // Simulate event
    eventHandler({
      payload: {
        proxy_state: "Fallback",
        fallback_reason: "Health check failed",
        failure_count: 3,
      },
    });

    await wrapper.vm.$nextTick();

    // Check that the UI reflects the updated state
    expect(wrapper.text()).toContain("已降级");
    expect(wrapper.text()).toContain("Health check failed");
    expect(wrapper.text()).toContain("3");
  });

  it("disables control buttons while controlling", async () => {
    const { invoke } = await import("@tauri-apps/api/core");
    const mockInvoke = invoke as ReturnType<typeof vi.fn>;
    mockInvoke.mockImplementation(
      () => new Promise((resolve) => setTimeout(resolve, 100))
    );

    const { wrapper, eventHandler } = await createWrapperWithEventHandler();

    // Set state to enabled
    eventHandler({
      payload: {
        proxy_state: "Enabled",
      },
    });
    await wrapper.vm.$nextTick();

    const fallbackBtn = wrapper.find(".fallback-btn");
    await fallbackBtn.trigger("click");

    // Button should be disabled during operation
    expect(fallbackBtn.attributes("disabled")).toBeDefined();
  });

  it("applies correct health check status class", async () => {
    const { wrapper, eventHandler } = await createWrapperWithEventHandler();

    // High success rate - success class
    eventHandler({
      payload: {
        health_check_success_rate: 0.9,
      },
    });
    await wrapper.vm.$nextTick();
    let progressFill = wrapper.find(".progress-fill");
    expect(progressFill.classes()).toContain("success");

    // Medium success rate - warning class
    eventHandler({
      payload: {
        health_check_success_rate: 0.6,
      },
    });
    await wrapper.vm.$nextTick();
    progressFill = wrapper.find(".progress-fill");
    expect(progressFill.classes()).toContain("warning");

    // Low success rate - error class
    eventHandler({
      payload: {
        health_check_success_rate: 0.3,
      },
    });
    await wrapper.vm.$nextTick();
    progressFill = wrapper.find(".progress-fill");
    expect(progressFill.classes()).toContain("error");
  });

  it("handles null health check rate", () => {
    const wrapper = mount(ProxyStatusPanel);

    // When no health check data, progress bar should not exist
    const healthCheckStats = wrapper.find(".health-check-stats");
    expect(healthCheckStats.exists()).toBe(false);
  });
});
