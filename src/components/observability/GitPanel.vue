<script setup lang="ts">
import { computed } from "vue";
import type { MetricsSnapshot } from "../../api/metrics";
import MetricCard from "./MetricCard.vue";
import MetricChart, { type ChartSeries } from "./MetricChart.vue";
import LoadingState from "./LoadingState.vue";
import EmptyState from "./EmptyState.vue";
import {
  aggregatedCounterSeries,
  computeRate,
  filterSeries,
  formatNumber,
  formatPercent,
  getSeries,
  groupByLabel,
  labelValue,
  sumCounter,
  sumCounters,
} from "../../utils/observability";

const KIND_LABELS: Record<string, string> = {
  GitClone: "Clone",
  GitFetch: "Fetch",
  GitPush: "Push",
  GitInit: "Init",
  GitAdd: "Add",
  GitCommit: "Commit",
  GitBranch: "Branch",
  GitCheckout: "Checkout",
  GitTag: "Tag",
  GitRemoteSet: "Remote Set",
  GitRemoteAdd: "Remote Add",
  GitRemoteRemove: "Remote Remove",
  HttpFake: "HTTP Fake",
  WorkspaceBatch: "Workspace Batch",
};

const props = defineProps<{
  snapshot: MetricsSnapshot | null;
  loading: boolean;
  error: string | null;
  stale: boolean;
}>();

const taskSeries = computed(() => getSeries(props.snapshot, "git_tasks_total"));
const retrySeries = computed(() => getSeries(props.snapshot, "git_retry_total"));

const completedTasks = computed(() => sumCounters(filterSeries(taskSeries.value, { state: "completed" })));
const failedTasks = computed(() =>
  sumCounters(filterSeries(taskSeries.value, { state: "failed" })) +
  sumCounters(filterSeries(taskSeries.value, { state: "canceled" })),
);
const totalTasks = computed(() => completedTasks.value + failedTasks.value);
const failureRate = computed(() => computeRate(failedTasks.value, totalTasks.value));
const retries = computed(() => sumCounters(retrySeries.value));

const groupedByKind = computed(() => groupByLabel(taskSeries.value, "kind"));
const kindStats = computed(() => {
  return Object.entries(groupedByKind.value)
    .map(([kind, entries]) => {
      const completed = entries
        .filter((entry) => labelValue(entry, "state") === "completed")
        .reduce((total, entry) => total + sumCounter(entry), 0);
      const failed = entries
        .filter((entry) => {
          const state = labelValue(entry, "state");
          return state === "failed" || state === "canceled";
        })
        .reduce((total, entry) => total + sumCounter(entry), 0);
      const total = completed + failed;
      return {
        kind,
        label: KIND_LABELS[kind] ?? kind,
        total,
        completed,
        failed,
        failureRate: computeRate(failed, total),
      };
    })
    .filter((stat) => stat.total > 0)
    .sort((a, b) => b.total - a.total);
});

const chartSeries = computed<ChartSeries[]>(() => {
  return kindStats.value.map((stat) => ({
    id: stat.kind,
    label: stat.label,
    points: aggregatedCounterSeries(
      groupedByKind.value[stat.kind]?.filter((entry) => labelValue(entry, "state") === "completed") ?? [],
    ),
    color: undefined,
  }));
});

const retryByCategory = computed(() => {
  const categories = groupByLabel(retrySeries.value, "category");
  return Object.entries(categories)
    .map(([category, entries]) => ({
      category,
      total: entries.reduce((total, entry) => total + sumCounter(entry), 0),
    }))
    .filter((item) => item.total > 0)
    .sort((a, b) => b.total - a.total);
});

const showEmpty = computed(() => !props.snapshot && !props.loading && !props.error);

const failureRateDisplay = computed(() => formatPercent(failureRate.value));
const totalTasksDisplay = computed(() => formatNumber(totalTasks.value));
const retriesDisplay = computed(() => formatNumber(retries.value));
</script>

<template>
  <div class="git-panel">
    <LoadingState v-if="loading && !snapshot" />
    <div v-else-if="error && !snapshot" class="git-panel__error">
      加载 Git 指标失败：{{ error }}
    </div>
    <EmptyState v-else-if="showEmpty" message="暂无 Git 指标" />
    <div v-else class="git-panel__content">
      <div class="git-panel__meta" v-if="snapshot">
        <span>总任务：{{ totalTasksDisplay }}</span>
        <span v-if="stale" class="git-panel__badge">数据为缓存</span>
      </div>
      <div class="git-panel__cards">
        <MetricCard
          title="任务失败率"
          :value="failureRateDisplay"
          :muted="totalTasks === 0"
          :trend-label="'失败'"
          :trend-value="formatNumber(failedTasks)"
          description="最近窗口内失败与取消占比"
        />
        <MetricCard
          title="任务完成"
          :value="formatNumber(completedTasks)"
          :trend-label="'总数'"
          :trend-value="totalTasksDisplay"
          description="最近窗口成功执行的 Git 任务"
        />
        <MetricCard
          title="重试次数"
          :value="retriesDisplay"
          :muted="retries === 0"
          description="Retryable 错误触发的重试总次数"
        >
          <ul v-if="retryByCategory.length" class="git-panel__retry-list">
            <li v-for="item in retryByCategory" :key="item.category">
              <span class="git-panel__retry-category">{{ item.category }}</span>
              <span class="git-panel__retry-count">{{ formatNumber(item.total) }}</span>
            </li>
          </ul>
        </MetricCard>
      </div>
      <section class="git-panel__chart">
        <header>
          <h4>各任务类型吞吐</h4>
        </header>
        <MetricChart :series="chartSeries" empty-message="暂无任务事件" :value-formatter="formatNumber" />
      </section>
      <section class="git-panel__table" v-if="kindStats.length">
        <header>
          <h4>任务类型明细</h4>
        </header>
        <table>
          <thead>
            <tr>
              <th>类型</th>
              <th>完成</th>
              <th>失败/取消</th>
              <th>总计</th>
              <th>失败率</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="stat in kindStats" :key="stat.kind">
              <td>{{ stat.label }}</td>
              <td>{{ formatNumber(stat.completed) }}</td>
              <td>{{ formatNumber(stat.failed) }}</td>
              <td>{{ formatNumber(stat.total) }}</td>
              <td>{{ formatPercent(stat.failureRate) }}</td>
            </tr>
          </tbody>
        </table>
      </section>
    </div>
  </div>
</template>

<style scoped>
.git-panel {
  @apply flex flex-col gap-4;
}

.git-panel__error {
  @apply rounded-xl border border-error/40 bg-error/10 px-4 py-6 text-error;
}

.git-panel__content {
  @apply flex flex-col gap-6;
}

.git-panel__meta {
  @apply flex items-center gap-3 text-xs text-base-content/60;
}

.git-panel__badge {
  @apply rounded-full bg-warning/10 px-2 py-0.5 text-warning;
}

.git-panel__cards {
  @apply grid gap-4 md:grid-cols-2 xl:grid-cols-3;
}

.git-panel__chart,
.git-panel__table {
  @apply flex flex-col gap-2;
}

.git-panel__chart h4,
.git-panel__table h4 {
  @apply text-sm font-semibold text-base-content/80;
}

.git-panel__table table {
  @apply w-full table-auto overflow-hidden rounded-xl border border-base-200 text-sm;
}

.git-panel__table thead {
  @apply bg-base-200/60 text-left text-xs uppercase tracking-wide text-base-content/60;
}

.git-panel__table th,
.git-panel__table td {
  @apply px-3 py-2;
}

.git-panel__table tbody tr:nth-child(even) {
  @apply bg-base-200/20;
}

.git-panel__retry-list {
  @apply mt-2 divide-y divide-base-200/60 text-xs;
}

.git-panel__retry-list li {
  @apply flex items-center justify-between py-1;
}

.git-panel__retry-category {
  @apply truncate font-medium text-base-content/70;
}

.git-panel__retry-count {
  @apply font-mono text-base-content;
}
</style>
