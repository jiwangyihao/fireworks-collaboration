<script setup lang="ts">
import { computed } from "vue";
import type { MetricsSnapshot } from "../../api/metrics";
import MetricCard from "./MetricCard.vue";
import LoadingState from "./LoadingState.vue";
import EmptyState from "./EmptyState.vue";
import {
  combinedHistogramTotals,
  formatDurationMs,
  formatNumber,
  formatPercent,
  getQuantile,
  getSeries,
  groupByLabel,
  labelValue,
} from "../../utils/observability";

const props = defineProps<{
  snapshot: MetricsSnapshot | null;
  loading: boolean;
  error: string | null;
  stale: boolean;
}>();

const tlsSeries = computed(() => getSeries(props.snapshot, "tls_handshake_ms"));
const successSeries = computed(() => tlsSeries.value.filter((series) => labelValue(series, "outcome") === "ok"));
const failSeries = computed(() => tlsSeries.value.filter((series) => labelValue(series, "outcome") === "fail"));

const successTotals = computed(() => combinedHistogramTotals(successSeries.value));
const failTotals = computed(() => combinedHistogramTotals(failSeries.value));
const totalCount = computed(() => successTotals.value.count + failTotals.value.count);

const failureRate = computed(() => {
  if (totalCount.value === 0) {
    return null;
  }
  return failTotals.value.count / totalCount.value;
});

const strategyStats = computed(() => {
  const groups = groupByLabel(successSeries.value, "sni_strategy");
  return Object.entries(groups)
    .map(([strategy, entries]) => {
      const totals = combinedHistogramTotals(entries);
      const quantiles = entries
        .map((entry) => ({
          p50: getQuantile(entry, "p50"),
          p95: getQuantile(entry, "p95"),
          p99: getQuantile(entry, "p99"),
        }))
        .find((item) => item.p50 !== null || item.p95 !== null || item.p99 !== null) ?? {
        p50: null,
        p95: null,
        p99: null,
      };
      return {
        strategy,
        count: totals.count,
        p50: quantiles.p50,
        p95: quantiles.p95,
        p99: quantiles.p99,
      };
    })
    .filter((item) => item.count > 0)
    .sort((a, b) => b.count - a.count);
});

const failAverage = computed(() => {
  if (failTotals.value.count === 0) {
    return null;
  }
  return failTotals.value.sum / failTotals.value.count;
});

const showEmpty = computed(() => !props.snapshot && !props.loading && !props.error);

const successCountDisplay = computed(() => formatNumber(successTotals.value.count));
const failCountDisplay = computed(() => formatNumber(failTotals.value.count));
const failureRateDisplay = computed(() => formatPercent(failureRate.value));
const failAverageDisplay = computed(() => formatDurationMs(failAverage.value, 1));
</script>

<template>
  <div class="tls-panel">
    <LoadingState v-if="loading && !snapshot" />
    <div v-else-if="error && !snapshot" class="tls-panel__error">加载 TLS 指标失败：{{ error }}</div>
    <EmptyState v-else-if="showEmpty" message="暂无 TLS 指标" />
    <div v-else class="tls-panel__content">
      <div class="tls-panel__meta" v-if="snapshot">
        <span>总握手：{{ formatNumber(totalCount) }}</span>
        <span v-if="stale" class="tls-panel__badge">数据为缓存</span>
      </div>
      <div class="tls-panel__cards">
        <MetricCard
          title="握手成功"
          :value="successCountDisplay"
          :trend-label="'失败'"
          :trend-value="failCountDisplay"
          description="最近窗口完成的 TLS 握手次数"
        />
        <MetricCard
          title="失败率"
          :value="failureRateDisplay"
          :muted="failureRate === null"
          description="失败占握手总数比例"
        />
        <MetricCard
          title="失败平均耗时"
          :value="failAverageDisplay"
          :muted="failAverage === null"
          description="握手失败样本的平均耗时"
        />
      </div>
      <section class="tls-panel__table" v-if="strategyStats.length">
        <header>
          <h4>SNI 策略分布</h4>
        </header>
        <table>
          <thead>
            <tr>
              <th>策略</th>
              <th>成功次数</th>
              <th>P50</th>
              <th>P95</th>
              <th>P99</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="stat in strategyStats" :key="stat.strategy">
              <td>{{ stat.strategy }}</td>
              <td>{{ formatNumber(stat.count) }}</td>
              <td>{{ formatDurationMs(stat.p50, 1) }}</td>
              <td>{{ formatDurationMs(stat.p95, 1) }}</td>
              <td>{{ formatDurationMs(stat.p99, 1) }}</td>
            </tr>
          </tbody>
        </table>
      </section>
    </div>
  </div>
</template>

<style scoped>
.tls-panel {
  @apply flex flex-col gap-4;
}

.tls-panel__error {
  @apply rounded-xl border border-error/40 bg-error/10 px-4 py-6 text-error;
}

.tls-panel__content {
  @apply flex flex-col gap-6;
}

.tls-panel__meta {
  @apply flex items-center gap-3 text-xs text-base-content/60;
}

.tls-panel__badge {
  @apply rounded-full bg-warning/10 px-2 py-0.5 text-warning;
}

.tls-panel__cards {
  @apply grid gap-4 md:grid-cols-2 xl:grid-cols-3;
}

.tls-panel__table {
  @apply flex flex-col gap-2;
}

.tls-panel__table h4 {
  @apply text-sm font-semibold text-base-content/80;
}

.tls-panel__table table {
  @apply w-full table-auto overflow-hidden rounded-xl border border-base-200 text-sm;
}

.tls-panel__table thead {
  @apply bg-base-200/60 text-left text-xs uppercase tracking-wide text-base-content/60;
}

.tls-panel__table th,
.tls-panel__table td {
  @apply px-3 py-2;
}

.tls-panel__table tbody tr:nth-child(even) {
  @apply bg-base-200/20;
}
</style>
