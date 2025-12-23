<script setup lang="ts">
/**
 * StatusList - 状态列表组件
 *
 * 带动画的状态检查列表，用于显示环境检查、任务进度等
 */

export interface StatusItem {
  id: number;
  type: "success" | "warning" | "error" | "info";
  message: string;
}

defineProps<{
  /** 状态项列表 */
  items: StatusItem[];
}>();
</script>

<template>
  <TransitionGroup name="list" tag="ul" class="max-w-1/2">
    <li
      v-for="status in items"
      class="flex items-center gap-2 font-bold my-2!"
      :key="status.id"
    >
      <span class="inline-grid *:[grid-area:1/1]">
        <span
          v-if="status.type !== 'success'"
          class="status animate-ping"
          :class="`status-${status.type}`"
        ></span>
        <span class="status" :class="`status-${status.type}`"></span>
      </span>
      {{ status.message }}
    </li>
  </TransitionGroup>
</template>

<style scoped>
.list-move,
.list-enter-active,
.list-leave-active {
  transition: all 0.3s ease;
}

.list-enter-from,
.list-leave-to {
  opacity: 0;
  transform: translateX(-30px);
}

.list-leave-active {
  position: absolute;
}
</style>
