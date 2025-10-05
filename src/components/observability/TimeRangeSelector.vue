<script setup lang="ts">
import type { MetricsRange } from "../../api/metrics";

interface RangeOption {
  label: string;
  value: MetricsRange;
}

const props = defineProps<{
  modelValue: MetricsRange;
  options: RangeOption[];
}>();

const emit = defineEmits<{
  (e: "update:modelValue", value: MetricsRange): void;
}>();

function select(option: RangeOption) {
  if (option.value !== props.modelValue) {
    emit("update:modelValue", option.value);
  }
}
</script>

<template>
  <div class="time-range-selector">
    <button
      v-for="option in options"
      :key="option.value"
      type="button"
      class="time-range-selector__option"
      :class="{ 'time-range-selector__option--active': option.value === modelValue }"
      @click="select(option)"
    >
      {{ option.label }}
    </button>
  </div>
</template>

<style scoped>
.time-range-selector {
  @apply inline-flex items-center gap-2 rounded-lg border border-base-200 bg-base-100/70 p-1;
}

.time-range-selector__option {
  @apply rounded-md px-3 py-1 text-sm font-medium text-base-content/70 transition-colors;
}

.time-range-selector__option:hover {
  @apply bg-base-200/60 text-base-content;
}

.time-range-selector__option--active {
  @apply bg-primary text-primary-content shadow-sm;
}
</style>
