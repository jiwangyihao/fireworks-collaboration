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
  formatDurationMs,
  formatNumber,
  formatPercent,
  getSeries,
  groupByLabel,
  labelValue,
  sumCounters,
} from "../../utils/observability";

const props = defineProps<{
  snapshot: MetricsSnapshot | null;
  loading: boolean;
  error: string | null;
  stale: boolean;
}>();

const fallbackSeries = computed(() => getSeries(props.snapshot, "http_strategy_fallback_total"));
const fallbackTotal = computed(() => sumCounters(fallbackSeries.value));
const fallbackByStage = computed(() => groupByLabel(fallbackSeries.value, "stage"));

const networkChart = computed<ChartSeries[]>(() =>
  Object.entries(fallbackByStage.value).map(([stage, entries]) => ({
    id: stage,
    label: stage.toUpperCase(),
    points: aggregatedCounterSeries(entries),
  })),
);

const tlsSeries = computed(() => getSeries(props.snapshot, "tls_handshake_ms"));
const tlsSuccess = computed(() => combinedHistogramTotals(tlsSeries.value.filter((series) => labelValue(series, "outcome") === "ok")));
const tlsFail = computed(() => combinedHistogramTotals(tlsSeries.value.filter((series) => labelValue(series, "outcome") === "fail")));

const tlsP95ByStrategy = computed(() => {
  const byStrategy = groupByLabel(
    tlsSeries.value.filter((series) => labelValue(series, "outcome") === "ok"),
    "sni_strategy",
  );
  return Object.entries(byStrategy)
    .map(([strategy, entries]) => {
      const totals = combinedHistogramTotals(entries);
      const average = totals.count > 0 ? totals.sum / totals.count : null;
      return {
        strategy,
        count: totals.count,
        average,
      };
    })
    .filter((item) => item.count > 0)
    .sort((a, b) => b.count - a.count);
});

const tlsFailRate = computed(() => {
  const total = tlsSuccess.value.count + tlsFail.value.count;
  return total > 0 ? tlsFail.value.count / total : null;
});

const tlsFailDisplay = computed(() => formatNumber(tlsFail.value.count));
const tlsFailRateDisplay = computed(() => formatPercent(tlsFailRate.value));
const fallbackDisplay = computed(() => formatNumber(fallbackTotal.value));
const tlsSuccessCountDisplay = computed(() => formatNumber(tlsSuccess.value.count));

const showEmpty = computed(() => !props.snapshot && !props.loading && !props.error);
</script>

<template>
  <div class="network-panel flex flex-col gap-4">
    <LoadingState v-if="loading && !snapshot" />
    <div v-else-if="error && !snapshot" class="network-panel__error rounded-xl border border-error/40 bg-error/10 px-4 py-6 text-error">
      加载网络指标失败：{{ error }}
    </div>
    <EmptyState v-else-if="showEmpty" message="暂无网络指标" />
    <div v-else class="network-panel__content flex flex-col gap-6">
      <div class="network-panel__meta flex items-center gap-3 text-xs text-base-content/60" v-if="snapshot">
        <span>回退总次数：{{ fallbackDisplay }}</span>
        <span v-if="stale" class="network-panel__badge rounded-full bg-warning/10 px-2 py-0.5 text-warning">数据为缓存</span>
      </div>
      <div class="network-panel__cards grid gap-4 md:grid-cols-2 xl:grid-cols-3">
        <MetricCard
          title="回退触发"
          :value="fallbackDisplay"
          description="最近窗口内 HTTP/TLS 链路触发回退的总次数"
          :muted="fallbackTotal === 0"
        />
        <MetricCard
          title="TLS 失败次数"
          :value="tlsFailDisplay"
          :trend-label="'总握手'"
          :trend-value="tlsSuccessCountDisplay"
          :muted="tlsFail.count === 0"
          description="握手失败次数，包含 Fake/Real 双策略"
        />
        <MetricCard
          title="TLS 失败率"
          :value="tlsFailRateDisplay"
          :muted="tlsFailRate === null"
          description="失败占握手总数比例"
        />
      </div>
      <section class="network-panel__chart flex flex-col gap-2">
        <header>
          <h4 class="text-sm font-semibold text-base-content/80">回退阶段分布</h4>
        </header>
        <MetricChart :series="networkChart" empty-message="暂无回退事件" :value-formatter="formatNumber" />
      </section>
      <section class="network-panel__table flex flex-col gap-2" v-if="tlsP95ByStrategy.length">
        <header>
          <h4 class="text-sm font-semibold text-base-content/80">SNI 策略握手耗时</h4>
        </header>
        <table class="w-full table-auto overflow-hidden rounded-xl border border-base-200 text-sm">
          <thead class="bg-base-200/60 text-left text-xs uppercase tracking-wide text-base-content/60">
            <tr>
              <th class="px-3 py-2">策略</th>
              <th class="px-3 py-2">成功次数</th>
              <th class="px-3 py-2">平均耗时</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="item in tlsP95ByStrategy" :key="item.strategy" class="even:bg-base-200/20">
              <td class="px-3 py-2">{{ item.strategy }}</td>
              <td class="px-3 py-2">{{ formatNumber(item.count) }}</td>
              <td class="px-3 py-2">{{ formatDurationMs(item.average, 1) }}</td>
            </tr>
          </tbody>
        </table>
      </section>
    </div>
  </div>
</template>

