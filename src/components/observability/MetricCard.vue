<script setup lang="ts">
defineProps<{
  title: string;
  value: string;
  description?: string;
  trendLabel?: string;
  trendValue?: string;
  muted?: boolean;
}>();
</script>

<template>
  <section :class="['metric-card flex flex-col gap-2 rounded-xl border border-base-200 bg-base-100/80 p-4 shadow-sm', muted && 'opacity-70']">
    <header class="metric-card__header flex items-center justify-between text-sm text-base-content/70">
      <h3 class="metric-card__title font-medium">{{ title }}</h3>
      <div v-if="trendLabel && trendValue" class="metric-card__trend flex items-center gap-1 text-xs font-medium">
        <span class="metric-card__trend-label uppercase tracking-wide text-base-content/50">{{ trendLabel }}</span>
        <span class="metric-card__trend-value font-mono text-base-content">{{ trendValue }}</span>
      </div>
    </header>
    <div class="metric-card__value text-2xl font-semibold text-base-content">{{ value }}</div>
    <p v-if="description" class="metric-card__description text-xs leading-relaxed text-base-content/60">{{ description }}</p>
    <slot />
  </section>
</template>

