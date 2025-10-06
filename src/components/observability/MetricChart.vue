<script setup lang="ts">
import { computed } from "vue";
import type { NormalizedPoint } from "../../utils/observability";

const palette = [
  "#2563eb",
  "#16a34a",
  "#dc2626",
  "#9333ea",
  "#f97316",
  "#0ea5e9",
  "#d946ef",
];

export interface ChartSeries {
  id: string;
  label: string;
  points: NormalizedPoint[];
  color?: string;
}

const props = defineProps<{
  series: ChartSeries[];
  emptyMessage?: string;
  valueFormatter?: (value: number) => string;
}>();

const maxY = computed(() => {
  const values = props.series.flatMap((entry) => entry.points.map((point) => point.y));
  const max = Math.max(...values, 0);
  return max > 0 ? max : 1;
});

const lines = computed(() => {
  return props.series.map((entry, idx) => {
    const color = entry.color ?? palette[idx % palette.length];
  const points = entry.points.length <= 1 ? extendSinglePoint(entry.points) : entry.points;
    const pointsAttr = buildPolyline(points, maxY.value);
  const lastPoint = entry.points[entry.points.length - 1];
  const latest = lastPoint ? lastPoint.y : 0;
    return {
      id: entry.id,
      label: entry.label,
      color,
      pointsAttr,
      hasData: entry.points.length > 0,
      latest,
    };
  });
});

const hasAnyData = computed(() => lines.value.some((line) => line.hasData));

const formatter = computed(() => props.valueFormatter ?? defaultFormatter);

function extendSinglePoint(points: NormalizedPoint[]): NormalizedPoint[] {
  if (points.length === 0) {
    return [];
  }
  const [point] = points;
  return [
    { x: 0, y: point.y },
    { x: point.x, y: point.y },
  ];
}

function buildPolyline(points: NormalizedPoint[], max: number): string {
  if (points.length === 0) {
    return "";
  }
  const bottom = 35;
  const scale = 30;
  return points
    .map((point, idx, arr) => {
      const normalizedX = (point.x ?? 0) * 100;
      const normalized = max > 0 ? point.y / max : 0;
      const clamped = Number.isFinite(normalized) ? Math.min(Math.max(normalized, 0), 1) : 0;
      const y = bottom - clamped * scale;
      const x = Number.isFinite(normalizedX) ? normalizedX : (idx / Math.max(1, arr.length - 1)) * 100;
      return `${x.toFixed(2)},${y.toFixed(2)}`;
    })
    .join(" ");
}

function defaultFormatter(value: number): string {
  if (!Number.isFinite(value)) {
    return "N/A";
  }
  if (value >= 1_000) {
    return value.toLocaleString();
  }
  if (value >= 10) {
    return value.toFixed(0);
  }
  return value.toFixed(2);
}
</script>

<template>
  <div class="metric-chart flex flex-col gap-2 rounded-xl border border-base-200 bg-base-100/60 p-3 shadow-sm">
    <div v-if="!hasAnyData" class="metric-chart__empty flex min-h-24 items-center justify-center text-sm text-base-content/50">
      {{ emptyMessage ?? "暂无数据" }}
    </div>
    <div v-else class="metric-chart__canvas flex flex-col gap-2">
      <svg viewBox="0 0 100 40" preserveAspectRatio="none">
        <line x1="0" y1="35" x2="100" y2="35" class="metric-chart__axis" />
        <template v-for="line in lines" :key="line.id">
          <polyline
            v-if="line.hasData"
            :points="line.pointsAttr"
            :stroke="line.color"
            fill="none"
            stroke-width="1.5"
            stroke-linejoin="round"
            stroke-linecap="round"
          />
        </template>
      </svg>
      <div class="metric-chart__legend flex flex-wrap items-center gap-x-4 gap-y-1 text-xs text-base-content/70">
        <div v-for="line in lines" :key="`${line.id}-legend`" class="metric-chart__legend-item flex items-center gap-1">
          <span class="metric-chart__swatch h-2 w-2 rounded-full" :style="{ backgroundColor: line.color }" />
          <span class="metric-chart__label font-medium">{{ line.label }}</span>
          <span class="metric-chart__value font-mono text-xs text-base-content">{{ formatter(line.latest) }}</span>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.metric-chart__axis { stroke: var(--fallback-bc, rgba(148, 163, 184, 0.6)); stroke-width: 0.75; stroke-dasharray: 2 2; }
</style>
