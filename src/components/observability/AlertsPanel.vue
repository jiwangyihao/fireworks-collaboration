<script setup lang="ts">
import { computed } from "vue";
import type { MetricsSnapshot } from "../../api/metrics";
import MetricCard from "./MetricCard.vue";
import MetricChart, { type ChartSeries } from "./MetricChart.vue";
import LoadingState from "./LoadingState.vue";
import EmptyState from "./EmptyState.vue";
import {
  aggregatedCounterSeries,
  formatNumber,
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

const alertsSeries = computed(() => getSeries(props.snapshot, "alerts_fired_total"));
const totalAlerts = computed(() => sumCounters(alertsSeries.value));
const alertsBySeverity = computed(() => groupByLabel(alertsSeries.value, "severity"));

const chartSeries = computed<ChartSeries[]>(() =>
  Object.entries(alertsBySeverity.value).map(([severity, entries]) => ({
    id: severity,
    label: severity.toUpperCase(),
    points: aggregatedCounterSeries(entries),
  })),
);

const tableRows = computed(() =>
  Object.entries(alertsBySeverity.value)
    .map(([severity, entries]) => ({
      severity,
      total: sumCounters(entries),
    }))
    .filter((row) => row.total > 0)
    .sort((a, b) => b.total - a.total),
);

const showEmpty = computed(() => !props.snapshot && !props.loading && !props.error);
</script>

<template>
  <div class="alerts-panel">
    <LoadingState v-if="loading && !snapshot" />
    <div v-else-if="error && !snapshot" class="alerts-panel__error">加载告警指标失败：{{ error }}</div>
    <EmptyState v-else-if="showEmpty" message="暂无告警数据" />
    <div v-else class="alerts-panel__content">
      <div class="alerts-panel__meta" v-if="snapshot">
        <span>告警触发总次数：{{ formatNumber(totalAlerts) }}</span>
        <span v-if="stale" class="alerts-panel__badge">数据为缓存</span>
      </div>
      <div class="alerts-panel__cards">
        <MetricCard
          title="告警触发"
          :value="formatNumber(totalAlerts)"
          :muted="totalAlerts === 0"
          description="最近窗口内触发的规则数量"
        />
      </div>
      <section class="alerts-panel__chart">
        <header>
          <h4>按严重度统计</h4>
        </header>
        <MetricChart :series="chartSeries" empty-message="暂无告警事件" :value-formatter="formatNumber" />
      </section>
      <section class="alerts-panel__table" v-if="tableRows.length">
        <header>
          <h4>严重度明细</h4>
        </header>
        <table>
          <thead>
            <tr>
              <th>严重度</th>
              <th>次数</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="row in tableRows" :key="row.severity">
              <td>{{ row.severity }}</td>
              <td>{{ formatNumber(row.total) }}</td>
            </tr>
          </tbody>
        </table>
      </section>
    </div>
  </div>
</template>

<style scoped>
.alerts-panel {
  @apply flex flex-col gap-4;
}

.alerts-panel__error {
  @apply rounded-xl border border-error/40 bg-error/10 px-4 py-6 text-error;
}

.alerts-panel__content {
  @apply flex flex-col gap-6;
}

.alerts-panel__meta {
  @apply flex items-center gap-3 text-xs text-base-content/60;
}

.alerts-panel__badge {
  @apply rounded-full bg-warning/10 px-2 py-0.5 text-warning;
}

.alerts-panel__cards {
  @apply grid gap-4 md:grid-cols-2;
}

.alerts-panel__chart,
.alerts-panel__table {
  @apply flex flex-col gap-2;
}

.alerts-panel__chart h4,
.alerts-panel__table h4 {
  @apply text-sm font-semibold text-base-content/80;
}

.alerts-panel__table table {
  @apply w-full table-auto overflow-hidden rounded-xl border border-base-200 text-sm;
}

.alerts-panel__table thead {
  @apply bg-base-200/60 text-left text-xs uppercase tracking-wide text-base-content/60;
}

.alerts-panel__table th,
.alerts-panel__table td {
  @apply px-3 py-2;
}

.alerts-panel__table tbody tr:nth-child(even) {
  @apply bg-base-200/20;
}
</style>
