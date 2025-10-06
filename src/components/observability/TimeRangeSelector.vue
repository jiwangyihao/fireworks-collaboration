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
  <div class="time-range-selector inline-flex items-center gap-2 rounded-lg border border-base-200 bg-base-100/70 p-1">
    <button
      v-for="option in options"
      :key="option.value"
      type="button"
      @click="select(option)"
      :class="[
        'rounded-md px-3 py-1 text-sm font-medium transition-colors',
        option.value === modelValue
          ? 'bg-primary text-primary-content shadow-sm'
          : 'text-base-content/70 hover:bg-base-200/60 hover:text-base-content'
      ]"
    >
      {{ option.label }}
    </button>
  </div>
</template>
