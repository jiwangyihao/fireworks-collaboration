<script setup lang="ts">
import { computed } from "vue";
import type { MetricsSnapshot } from "../../api/metrics";
import MetricCard from "./MetricCard.vue";
import MetricChart, { type ChartSeries } from "./MetricChart.vue";
import LoadingState from "./LoadingState.vue";
import EmptyState from "./EmptyState.vue";
import {
  aggregatedCounterSeries,
  combinedHistogramTotals,
  computeRate,
  filterSeries,
  formatDurationMs,
  formatNumber,
  formatPercent,
  getQuantile,
  getSeries,
  sumCounters,
} from "../../utils/observability";

const props = defineProps<{
  snapshot: MetricsSnapshot | null;
  loading: boolean;
  error: string | null;
  stale: boolean;
}>();

const taskSeries = computed(() => getSeries(props.snapshot, "git_tasks_total"));
const completedTasks = computed(() => sumCounters(filterSeries(taskSeries.value, { state: "completed" })));
const failedTasks = computed(() =>
  sumCounters(filterSeries(taskSeries.value, { state: "failed" })) +
  sumCounters(filterSeries(taskSeries.value, { state: "canceled" })),
);
const totalTasks = computed(() => completedTasks.value + failedTasks.value);
const successRate = computed(() => computeRate(completedTasks.value, totalTasks.value));

const durationSeries = computed(() => getSeries(props.snapshot, "git_task_duration_ms"));
const durationTotals = computed(() => combinedHistogramTotals(durationSeries.value));
const averageTaskDuration = computed(() => {
  const totals = durationTotals.value;
  return totals.count > 0 ? totals.sum / totals.count : null;
});

const tlsSeries = computed(() => filterSeries(getSeries(props.snapshot, "tls_handshake_ms"), { outcome: "ok" }));
const tlsP95 = computed(() => {
  const values = tlsSeries.value
    .map((series) => getQuantile(series, "p95"))
    .filter((value): value is number => typeof value === "number");
  if (values.length === 0) {
    return null;
  }
  return Math.max(...values);
});
const tlsTotals = computed(() => combinedHistogramTotals(tlsSeries.value));

const ipRefreshSeries = computed(() => getSeries(props.snapshot, "ip_pool_refresh_total"));
const ipRefreshSuccess = computed(() => sumCounters(filterSeries(ipRefreshSeries.value, { success: "true" })));
const ipRefreshTotal = computed(() => ipRefreshSuccess.value + sumCounters(filterSeries(ipRefreshSeries.value, { success: "false" })));
const ipRefreshRate = computed(() => computeRate(ipRefreshSuccess.value, ipRefreshTotal.value));

const alertsTotal = computed(() => sumCounters(getSeries(props.snapshot, "alerts_fired_total")));

const throughputSeries = computed<ChartSeries[]>(() => {
  const completed = filterSeries(taskSeries.value, { state: "completed" });
  const failed = filterSeries(taskSeries.value, { state: "failed" });
  const canceled = filterSeries(taskSeries.value, { state: "canceled" });
  const failedAndCanceled = [...failed, ...canceled];
  return [
    {
      id: "completed",
      label: "完成",
      points: aggregatedCounterSeries(completed),
      color: "#16a34a",
    },
    {
      id: "failed",
      label: "失败/取消",
      points: aggregatedCounterSeries(failedAndCanceled),
      color: "#dc2626",
    },
  ];
});

const showEmpty = computed(() => !props.snapshot && !props.loading && !props.error);

const successRateDisplay = computed(() => formatPercent(successRate.value));
const averageDurationDisplay = computed(() => formatDurationMs(averageTaskDuration.value, 1));
const tlsP95Display = computed(() => formatDurationMs(tlsP95.value, 0));
const ipRefreshDisplay = computed(() => formatPercent(ipRefreshRate.value));
const alertsDisplay = computed(() => formatNumber(alertsTotal.value));
</script>

<template>
  <div class="overview-panel flex flex-col gap-4">
    <LoadingState v-if="loading && !snapshot" />
    <div v-else-if="error && !snapshot" class="overview-panel__error rounded-xl border border-error/40 bg-error/10 px-4 py-6 text-error">
      <span>加载指标失败：{{ error }}</span>
    </div>
    <EmptyState v-else-if="showEmpty" message="暂无指标数据" />
    <div v-else class="overview-panel__content flex flex-col gap-6">
      <div class="overview-panel__meta flex items-center gap-3 text-xs text-base-content/60" v-if="snapshot">
        <span>生成时间：{{ new Date(snapshot.generatedAtMs).toLocaleTimeString() }}</span>
        <span v-if="stale" class="overview-panel__badge rounded-full bg-warning/10 px-2 py-0.5 text-warning">数据为缓存</span>
      </div>
      <div class="overview-panel__cards grid gap-4 md:grid-cols-2 xl:grid-cols-3">
        <MetricCard
          title="任务成功率"
          :value="successRateDisplay"
          :trend-label="'总计'"
          :trend-value="formatNumber(totalTasks)"
          :muted="totalTasks === 0"
          description="最近窗口内 Git 任务成功占比"
        />
        <MetricCard
          title="平均任务耗时"
          :value="averageDurationDisplay"
          :trend-label="'样本数'"
          :trend-value="formatNumber(durationTotals.count)"
          description="基于 Git 任务耗时直方图计算"
        />
        <MetricCard
          title="TLS P95"
          :value="tlsP95Display"
          :trend-label="'采样数'"
          :trend-value="formatNumber(tlsTotals.count)"
          :muted="!tlsP95"
          description="成功握手的 95 分位延迟"
        />
        <MetricCard
          title="IP 池刷新成功率"
          :value="ipRefreshDisplay"
          :trend-label="'总刷新'"
          :trend-value="formatNumber(ipRefreshTotal)"
          :muted="ipRefreshTotal === 0"
          description="窗口内 IP 池刷新成功的比例"
        />
        <MetricCard
          title="触发告警"
          :value="alertsDisplay"
          description="窗口内告警触发次数"
          :muted="alertsTotal === 0"
        />
      </div>
      <section class="overview-panel__chart flex flex-col gap-2">
        <header class="flex items-center justify-between">
          <h4 class="text-sm font-semibold text-base-content/80">任务吞吐趋势</h4>
        </header>
        <MetricChart :series="throughputSeries" :value-formatter="formatNumber" empty-message="暂无任务事件" />
      </section>
    </div>
  </div>
</template>
