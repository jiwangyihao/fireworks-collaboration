import { mount } from "@vue/test-utils";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { createPinia, setActivePinia } from "pinia";
import ObservabilityView from "../ObservabilityView.vue";
import { useMetricsStore } from "../../stores/metrics";
import { useConfigStore } from "../../stores/config";
import type { AppConfig, ObservabilityConfig } from "../../api/config";

const MOCK_SNAPSHOT = { generatedAtMs: 0, series: [] };

const STUBS = {
  TimeRangeSelector: true,
  OverviewPanel: true,
  GitPanel: true,
  NetworkPanel: true,
  IpPoolPanel: true,
  TlsPanel: true,
  ProxyPanel: true,
  AlertsPanel: true,
};

function configureObservability(overrides?: Partial<ObservabilityConfig>) {
  const configStore = useConfigStore();
  const base: ObservabilityConfig = {
    enabled: true,
    basicEnabled: true,
    aggregateEnabled: true,
    exportEnabled: true,
    uiEnabled: true,
    alertsEnabled: true,
    export: {
      authToken: null,
      rateLimitQps: 5,
      maxSeriesPerSnapshot: 1_000,
      bindAddress: "127.0.0.1:9688",
    },
  };
  configStore.cfg = {
    observability: { ...base, ...overrides },
  } as unknown as AppConfig;
}

async function flushPromises() {
  await Promise.resolve();
  await Promise.resolve();
}

describe("ObservabilityView", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.clearAllMocks();
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("fetches metrics on mount when observability is enabled", async () => {
    configureObservability();
    const metricsStore = useMetricsStore();
    const ensureSpy = vi.spyOn(metricsStore, "ensure").mockResolvedValue(MOCK_SNAPSHOT);

    mount(ObservabilityView, {
      global: { stubs: STUBS },
    });

    await flushPromises();

    expect(ensureSpy).toHaveBeenCalledTimes(1);
    const [query, options] = ensureSpy.mock.calls[0];
    expect(query.names).toContain("git_tasks_total");
    expect(query.names).toContain("alerts_fired_total");
    expect(query.range).toBe("5m");
    expect(options).toBeUndefined();
  });

  it("omits alert metrics and tab when alerts are disabled", async () => {
    configureObservability({ alertsEnabled: false });
    const metricsStore = useMetricsStore();
    const ensureSpy = vi.spyOn(metricsStore, "ensure").mockResolvedValue(MOCK_SNAPSHOT);

    const wrapper = mount(ObservabilityView, {
      global: { stubs: STUBS },
    });

    await flushPromises();

    expect(ensureSpy).toHaveBeenCalledTimes(1);
    const [query] = ensureSpy.mock.calls[0];
    expect(query.names).not.toContain("alerts_fired_total");
    expect(wrapper.find('[data-testid="observability-tab-alerts"]').exists()).toBe(false);
  });

  it("stops fetching when observability UI is disabled", async () => {
    configureObservability({ enabled: false });
    const metricsStore = useMetricsStore();
    const ensureSpy = vi.spyOn(metricsStore, "ensure").mockResolvedValue(MOCK_SNAPSHOT);

    const wrapper = mount(ObservabilityView, {
      global: { stubs: STUBS },
    });

    await flushPromises();

    expect(ensureSpy).not.toHaveBeenCalled();
    expect(wrapper.find(".observability-view__disabled").exists()).toBe(true);
    expect(wrapper.text()).toContain("可观测性功能已关闭");
  });

  it("forces a refresh when the manual button is clicked", async () => {
    configureObservability();
    const metricsStore = useMetricsStore();
    const ensureSpy = vi.spyOn(metricsStore, "ensure").mockResolvedValue(MOCK_SNAPSHOT);

    const wrapper = mount(ObservabilityView, {
      global: { stubs: STUBS },
    });

    await flushPromises();
    ensureSpy.mockClear();

    await wrapper.get('[data-testid="observability-refresh"]').trigger("click");
    await flushPromises();

    expect(ensureSpy).toHaveBeenCalledTimes(1);
    const [, options] = ensureSpy.mock.calls[0];
    expect(options).toEqual({ force: true });
  });
});
