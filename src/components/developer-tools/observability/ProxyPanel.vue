<script setup lang="ts">
import { computed } from "vue";
import type { MetricsSnapshot } from "../../../api/metrics";
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
} from "../../../utils/observability";

const props = defineProps<{
  snapshot: MetricsSnapshot | null;
  loading: boolean;
  error: string | null;
  stale: boolean;
}>();

const fallbackSeries = computed(() =>
  getSeries(props.snapshot, "proxy_fallback_total")
);
const fallbackTotal = computed(() => sumCounters(fallbackSeries.value));
const fallbackByMode = computed(() =>
  groupByLabel(fallbackSeries.value, "mode")
);

const chartSeries = computed<ChartSeries[]>(() =>
  Object.entries(fallbackByMode.value).map(([mode, entries]) => ({
    id: mode,
    label: mode.toUpperCase(),
    points: aggregatedCounterSeries(entries),
  }))
);

const showEmpty = computed(
  () => !props.snapshot && !props.loading && !props.error
);

const tableRows = computed(() =>
  Object.entries(fallbackByMode.value)
    .map(([mode, entries]) => ({
      mode,
      total: sumCounters(entries),
    }))
    .filter((row) => row.total > 0)
    .sort((a, b) => b.total - a.total)
);
</script>

<template>
  <div class="proxy-panel flex flex-col gap-4">
    <LoadingState v-if="loading && !snapshot" />
    <div
      v-else-if="error && !snapshot"
      class="proxy-panel__error rounded-xl border border-error/40 bg-error/10 px-4 py-6 text-error"
    >
      加载代理指标失败：{{ error }}
    </div>
    <EmptyState v-else-if="showEmpty" message="暂无代理指标" />
    <div v-else class="proxy-panel__content flex flex-col gap-6">
      <div
        class="proxy-panel__meta flex items-center gap-3 text-xs text-base-content/60"
        v-if="snapshot"
      >
        <span>代理降级总次数：{{ formatNumber(fallbackTotal) }}</span>
        <span
          v-if="stale"
          class="proxy-panel__badge rounded-full bg-warning/10 px-2 py-0.5 text-warning"
          >数据为缓存</span
        >
      </div>
      <div class="proxy-panel__cards grid gap-4 md:grid-cols-2">
        <MetricCard
          title="代理降级"
          :value="formatNumber(fallbackTotal)"
          description="最近窗口触发的代理模式降级次数"
          :muted="fallbackTotal === 0"
        />
      </div>
      <section class="proxy-panel__chart flex flex-col gap-2">
        <header>
          <h4 class="text-sm font-semibold text-base-content/80">
            按模式统计降级
          </h4>
        </header>
        <MetricChart
          :series="chartSeries"
          empty-message="暂无降级事件"
          :value-formatter="formatNumber"
        />
      </section>
      <section
        class="proxy-panel__table flex flex-col gap-2"
        v-if="tableRows.length"
      >
        <header>
          <h4 class="text-sm font-semibold text-base-content/80">模式明细</h4>
        </header>
        <table
          class="w-full table-auto overflow-hidden rounded-xl border border-base-200 text-sm"
        >
          <thead
            class="bg-base-200/60 text-left text-xs uppercase tracking-wide text-base-content/60"
          >
            <tr>
              <th class="px-3 py-2">模式</th>
              <th class="px-3 py-2">降级次数</th>
            </tr>
          </thead>
          <tbody>
            <tr
              v-for="row in tableRows"
              :key="row.mode"
              class="even:bg-base-200/20"
            >
              <td class="px-3 py-2">{{ row.mode }}</td>
              <td class="px-3 py-2">{{ formatNumber(row.total) }}</td>
            </tr>
          </tbody>
        </table>
      </section>
    </div>
  </div>
</template>
