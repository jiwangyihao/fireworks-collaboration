<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { storeToRefs } from "pinia";
import TimeRangeSelector from "../components/observability/TimeRangeSelector.vue";
import OverviewPanel from "../components/observability/OverviewPanel.vue";
import GitPanel from "../components/observability/GitPanel.vue";
import NetworkPanel from "../components/observability/NetworkPanel.vue";
import IpPoolPanel from "../components/observability/IpPoolPanel.vue";
import TlsPanel from "../components/observability/TlsPanel.vue";
import ProxyPanel from "../components/observability/ProxyPanel.vue";
import AlertsPanel from "../components/observability/AlertsPanel.vue";
import { useMetricsStore, type MetricsQuery } from "../stores/metrics";
import type { MetricsRange } from "../api/metrics";
import { useConfigStore } from "../stores/config";

const BASE_METRIC_NAMES = [
  "git_tasks_total",
  "git_task_duration_ms",
  "git_retry_total",
  "tls_handshake_ms",
  "http_strategy_fallback_total",
  "ip_pool_refresh_total",
  "ip_pool_latency_ms",
  "ip_pool_auto_disable_total",
  "ip_pool_selection_total",
  "circuit_breaker_trip_total",
  "circuit_breaker_recover_total",
  "proxy_fallback_total",
  "alerts_fired_total",
];

const QUANTILES = [0.5, 0.9, 0.95, 0.99];

const rangeOptions = [
  { label: "5 分钟", value: "5m" as MetricsRange },
  { label: "1 小时", value: "1h" as MetricsRange },
  { label: "24 小时", value: "24h" as MetricsRange },
];

const BASE_TABS = [
  { id: "overview", label: "概览", component: OverviewPanel },
  { id: "git", label: "Git 任务", component: GitPanel },
  { id: "network", label: "网络链路", component: NetworkPanel },
  { id: "ip", label: "IP 池", component: IpPoolPanel },
  { id: "tls", label: "TLS", component: TlsPanel },
  { id: "proxy", label: "代理", component: ProxyPanel },
  { id: "alerts", label: "告警", component: AlertsPanel },
];

const selectedRange = ref<MetricsRange>("5m");
const activeTab = ref("overview");
const configStore = useConfigStore();
const { cfg: config } = storeToRefs(configStore);
const metricsStore = useMetricsStore();

const metricsNames = computed(() => {
  if (config.value?.observability?.alertsEnabled === false) {
    return BASE_METRIC_NAMES.filter((name) => name !== "alerts_fired_total");
  }
  return BASE_METRIC_NAMES;
});

const visibleTabs = computed(() => {
  if (config.value?.observability?.alertsEnabled === false) {
    return BASE_TABS.filter((tab) => tab.id !== "alerts");
  }
  return BASE_TABS;
});

watch(
  () => visibleTabs.value.map((tab) => tab.id).join("|"),
  (ids) => {
    if (!ids) {
      activeTab.value = "";
      return;
    }
    const candidates = ids.split("|").filter((id) => id.length > 0);
    if (candidates.length === 0) {
      activeTab.value = "";
      return;
    }
    if (!candidates.includes(activeTab.value)) {
      activeTab.value = candidates[0];
    }
  },
  { immediate: true },
);

const query = computed<MetricsQuery>(() => ({
  names: metricsNames.value,
  range: selectedRange.value,
  quantiles: QUANTILES,
}));

const observabilityDisabled = computed(() => {
  const cfg = config.value;
  if (!cfg) {
    return false;
  }
  const obs = cfg.observability;
  if (!obs) {
    return true;
  }
  if (!obs.enabled) {
    return true;
  }
  if (obs.uiEnabled === false) {
    return true;
  }
  return false;
});

const observabilityDisabledReason = computed(() => {
  if (!config.value) {
    return "正在读取配置，请稍后重试";
  }
  const obs = config.value.observability;
  if (!obs) {
    return "当前配置未启用可观测性，请在配置文件中开启 observability.enabled";
  }
  if (!obs.enabled) {
    return "可观测性功能已关闭，请启用 observability.enabled";
  }
  if (obs.uiEnabled === false) {
    return "可观测性面板已关闭，请启用 observability.uiEnabled";
  }
  return "";
});

watch(
  [() => ({ ...query.value }), () => observabilityDisabled.value],
  ([next, disabled]) => {
    if (disabled) {
      return;
    }
    metricsStore.ensure(next).catch((err) => {
      console.error("failed to refresh metrics", err);
    });
  },
  { immediate: true },
);

const entry = computed(() => metricsStore.getEntry(query.value));
const snapshot = computed(() => entry.value?.snapshot ?? null);
const loading = computed(() => entry.value?.loading ?? (!entry.value || !entry.value.snapshot));
const error = computed(() => entry.value?.error ?? null);
const stale = computed(() => {
  if (!entry.value || !entry.value.snapshot) {
    return false;
  }
  return metricsStore.isStale(query.value);
});

const activeComponent = computed(() => {
  const collection = visibleTabs.value;
  const record = collection.find((tab) => tab.id === activeTab.value) ?? collection[0];
  return record?.component ?? OverviewPanel;
});

async function refresh() {
  if (observabilityDisabled.value) {
    return;
  }
  try {
    await metricsStore.ensure(query.value, { force: true });
  } catch (err) {
    console.error("failed to force refresh metrics", err);
  }
}
</script>

<template>
  <div class="observability-view page">
    <header class="observability-view__header">
      <div>
        <h1>可观测性面板</h1>
        <p class="observability-view__subtitle">汇总 Git 任务、网络、IP 池、TLS、代理与告警的关键指标</p>
      </div>
      <div class="observability-view__controls">
        <TimeRangeSelector v-model="selectedRange" :options="rangeOptions" />
        <button class="btn btn-sm" type="button" data-testid="observability-refresh" @click="refresh">
          手动刷新
        </button>
      </div>
    </header>
    <section class="observability-view__status" v-if="!observabilityDisabled && error">
      <span class="observability-view__error">{{ error }}</span>
    </section>
    <section v-if="observabilityDisabled" class="observability-view__disabled">
      <h2>可观测性面板未启用</h2>
      <p>{{ observabilityDisabledReason }}</p>
    </section>
    <template v-else>
      <nav class="observability-view__tabs">
        <button
          v-for="tab in visibleTabs"
          :key="tab.id"
          type="button"
          class="observability-view__tab"
          :class="{ 'observability-view__tab--active': tab.id === activeTab }"
          :data-testid="`observability-tab-${tab.id}`"
          @click="activeTab = tab.id"
        >
          {{ tab.label }}
        </button>
      </nav>
      <component
        :is="activeComponent"
        :snapshot="snapshot"
        :loading="loading"
        :error="error"
        :stale="stale"
        :key="`${activeTab}-${selectedRange}`"
      />
    </template>
  </div>
</template>

<style scoped>
@reference "../style.css";
.observability-view__header {
  @apply flex flex-col gap-4 border-b border-base-200 pb-4 md:flex-row md:items-center md:justify-between;
}

.observability-view__subtitle {
  @apply text-sm text-base-content/60;
}

.observability-view__controls {
  @apply flex items-center gap-2;
}

.observability-view__status {
  @apply mt-3 rounded-lg border border-warning/40 bg-warning/10 px-3 py-2 text-warning;
}

.observability-view__tabs {
  @apply mt-4 flex flex-wrap gap-2 border-b border-base-200 pb-2;
}

.observability-view__tab {
  @apply rounded-lg px-3 py-1.5 text-sm font-medium text-base-content/60 transition-colors;
}

.observability-view__tab:hover {
  @apply bg-base-200/60 text-base-content;
}

.observability-view__tab--active {
  @apply bg-primary text-primary-content shadow-sm;
}

.observability-view__error {
  @apply text-sm;
}

.observability-view__disabled {
  @apply mt-6 flex flex-col gap-2 rounded-xl border border-base-200 bg-base-200/20 px-4 py-6;
}

.observability-view__disabled h2 {
  @apply text-base font-semibold text-base-content/80;
}

.observability-view__disabled p {
  @apply text-sm text-base-content/70;
}
</style>
