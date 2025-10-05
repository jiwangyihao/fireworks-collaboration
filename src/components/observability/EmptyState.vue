<script setup lang="ts">
const props = defineProps<{ message?: string }>();
</script>

<template>
  <div class="empty-state">
    {{ props.message ?? "暂无可展示的数据" }}
  </div>
</template>

<style scoped>
.empty-state {
  @apply flex min-h-24 items-center justify-center rounded-xl border border-dashed border-base-300/60 bg-base-100/40 text-sm text-base-content/50;
}
</style>
