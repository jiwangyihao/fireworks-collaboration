<script setup lang="ts">
import { computed } from "vue";
import type { MetricsSnapshot } from "../../api/metrics";
import MetricCard from "./MetricCard.vue";
import MetricChart, { type ChartSeries } from "./MetricChart.vue";
import LoadingState from "./LoadingState.vue";
import EmptyState from "./EmptyState.vue";
import {
  aggregatedHistogramSeries,
  computeRate,
  filterSeries,
  formatDurationMs,
  formatNumber,
  formatPercent,
  getSeries,
  groupByLabel,
  sumCounters,
} from "../../utils/observability";

const props = defineProps<{
  snapshot: MetricsSnapshot | null;
  loading: boolean;
  error: string | null;
  stale: boolean;
}>();

const refreshSeries = computed(() => getSeries(props.snapshot, "ip_pool_refresh_total"));
const refreshSuccess = computed(() => sumCounters(filterSeries(refreshSeries.value, { success: "true" })));
const refreshFail = computed(() => sumCounters(filterSeries(refreshSeries.value, { success: "false" })));
const refreshTotal = computed(() => refreshSuccess.value + refreshFail.value);
const refreshRate = computed(() => computeRate(refreshSuccess.value, refreshTotal.value));

const failureReasons = computed(() => {
  const failures = filterSeries(refreshSeries.value, { success: "false" });
  const reasons = groupByLabel(failures, "reason");
  return Object.entries(reasons)
    .map(([reason, entries]) => ({
      reason,
      total: sumCounters(entries),
    }))
    .filter((item) => item.total > 0)
    .sort((a, b) => b.total - a.total);
});

const latencySeries = computed(() => getSeries(props.snapshot, "ip_pool_latency_ms"));
const latencyChart = computed<ChartSeries[]>(() => {
  const bySource = groupByLabel(latencySeries.value, "source");
  return Object.entries(bySource).map(([source, entries]) => ({
    id: source,
    label: source,
    points: aggregatedHistogramSeries(entries),
  }));
});

const latencyFormatter = (value: number) => formatDurationMs(value, 1);

const autoDisable = computed(() => sumCounters(getSeries(props.snapshot, "ip_pool_auto_disable_total")));
const selectionAttempts = computed(() => sumCounters(getSeries(props.snapshot, "ip_pool_selection_total")));
const breakerTrips = computed(() => sumCounters(getSeries(props.snapshot, "circuit_breaker_trip_total")));
const breakerRecover = computed(() => sumCounters(getSeries(props.snapshot, "circuit_breaker_recover_total")));

const showEmpty = computed(() => !props.snapshot && !props.loading && !props.error);

const refreshRateDisplay = computed(() => formatPercent(refreshRate.value));
const autoDisableDisplay = computed(() => formatNumber(autoDisable.value));
const breakerTripDisplay = computed(() => formatNumber(breakerTrips.value));
const breakerRecoverDisplay = computed(() => formatNumber(breakerRecover.value));
const selectionDisplay = computed(() => formatNumber(selectionAttempts.value));
</script>

<template>
  <div class="ip-panel">
    <LoadingState v-if="loading && !snapshot" />
    <div v-else-if="error && !snapshot" class="ip-panel__error">加载 IP 池指标失败：{{ error }}</div>
    <EmptyState v-else-if="showEmpty" message="暂无 IP 池指标" />
    <div v-else class="ip-panel__content">
      <div class="ip-panel__meta" v-if="snapshot">
        <span>刷新总次数：{{ formatNumber(refreshTotal) }}</span>
        <span v-if="stale" class="ip-panel__badge">数据为缓存</span>
      </div>
      <div class="ip-panel__cards">
        <MetricCard
          title="刷新成功率"
          :value="refreshRateDisplay"
          :trend-label="'成功/总计'"
          :trend-value="`${formatNumber(refreshSuccess)} / ${formatNumber(refreshTotal)}`"
          :muted="refreshTotal === 0"
          description="最近窗口内 IP 池刷新成功占比"
        />
        <MetricCard
          title="自动禁用"
          :value="autoDisableDisplay"
          description="刷新/探测失败导致的自动禁用次数"
          :muted="autoDisable === 0"
        />
        <MetricCard
          title="熔断触发"
          :value="breakerTripDisplay"
          :trend-label="'恢复'"
          :trend-value="breakerRecoverDisplay"
          :muted="breakerTrips === 0"
          description="Circuit Breaker 触发与恢复次数"
        />
        <MetricCard
          title="IP 选择尝试"
          :value="selectionDisplay"
          description="策略选择 IP 的总尝试次数"
          :muted="selectionAttempts === 0"
        />
      </div>
      <section class="ip-panel__chart">
        <header>
          <h4>各来源延迟趋势</h4>
        </header>
  <MetricChart :series="latencyChart" empty-message="暂无延迟样本" :value-formatter="latencyFormatter" />
      </section>
      <section class="ip-panel__table" v-if="failureReasons.length">
        <header>
          <h4>刷新失败原因</h4>
        </header>
        <table>
          <thead>
            <tr>
              <th>原因</th>
              <th>次数</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="item in failureReasons" :key="item.reason">
              <td>{{ item.reason }}</td>
              <td>{{ formatNumber(item.total) }}</td>
            </tr>
          </tbody>
        </table>
      </section>
    </div>
  </div>
</template>

<style scoped>
.ip-panel {
  @apply flex flex-col gap-4;
}

.ip-panel__error {
  @apply rounded-xl border border-error/40 bg-error/10 px-4 py-6 text-error;
}

.ip-panel__content {
  @apply flex flex-col gap-6;
}

.ip-panel__meta {
  @apply flex items-center gap-3 text-xs text-base-content/60;
}

.ip-panel__badge {
  @apply rounded-full bg-warning/10 px-2 py-0.5 text-warning;
}

.ip-panel__cards {
  @apply grid gap-4 md:grid-cols-2 xl:grid-cols-4;
}

.ip-panel__chart,
.ip-panel__table {
  @apply flex flex-col gap-2;
}

.ip-panel__chart h4,
.ip-panel__table h4 {
  @apply text-sm font-semibold text-base-content/80;
}

.ip-panel__table table {
  @apply w-full table-auto overflow-hidden rounded-xl border border-base-200 text-sm;
}

.ip-panel__table thead {
  @apply bg-base-200/60 text-left text-xs uppercase tracking-wide text-base-content/60;
}

.ip-panel__table th,
.ip-panel__table td {
  @apply px-3 py-2;
}

.ip-panel__table tbody tr:nth-child(even) {
  @apply bg-base-200/20;
}
</style>
