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
  <section
    class="metric-card"
    :class="{
      'metric-card--muted': muted,
    }"
  >
    <header class="metric-card__header">
      <h3 class="metric-card__title">{{ title }}</h3>
      <div v-if="trendLabel && trendValue" class="metric-card__trend">
        <span class="metric-card__trend-label">{{ trendLabel }}</span>
        <span class="metric-card__trend-value">{{ trendValue }}</span>
      </div>
    </header>
    <div class="metric-card__value">
      {{ value }}
    </div>
    <p v-if="description" class="metric-card__description">
      {{ description }}
    </p>
    <slot />
  </section>
</template>

<style scoped>
.metric-card {
  @apply flex flex-col gap-2 rounded-xl border border-base-200 bg-base-100/80 p-4 shadow-sm;
}

.metric-card--muted {
  @apply opacity-70;
}

.metric-card__header {
  @apply flex items-center justify-between text-sm text-base-content/70;
}

.metric-card__title {
  @apply font-medium;
}

.metric-card__trend {
  @apply flex items-center gap-1 text-xs font-medium;
}

.metric-card__trend-label {
  @apply uppercase tracking-wide text-base-content/50;
}

.metric-card__trend-value {
  @apply font-mono text-base-content;
}

.metric-card__value {
  @apply text-2xl font-semibold text-base-content;
}

.metric-card__description {
  @apply text-xs leading-relaxed text-base-content/60;
}
</style>
