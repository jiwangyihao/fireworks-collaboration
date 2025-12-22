<script setup lang="ts">
import { storeToRefs } from "pinia";
import { useToastStore } from "../stores/toast";

const toastStore = useToastStore();
const { toasts } = storeToRefs(toastStore);

function getAlertClass(type: string) {
  switch (type) {
    case "success":
      return "bg-success text-success-content";
    case "warning":
      return "bg-warning text-warning-content";
    case "error":
      return "bg-error text-error-content";
    default:
      return "bg-info text-info-content";
  }
}

function getIcon(type: string) {
  switch (type) {
    case "success":
      return "✓";
    case "warning":
      return "⚠";
    case "error":
      return "!";
    default:
      return "i";
  }
}
</script>

<template>
  <div class="toast toast-end toast-bottom z-50 gap-2">
    <TransitionGroup name="toast">
      <div
        v-for="toast in toasts"
        :key="toast.id"
        class="flex items-center gap-3 px-4 py-3 rounded-lg shadow-xl max-w-md"
        :class="getAlertClass(toast.type)"
      >
        <!-- 图标 -->
        <span
          class="w-6 h-6 rounded-full bg-white/20 flex items-center justify-center text-sm font-bold shrink-0"
        >
          {{ getIcon(toast.type) }}
        </span>
        <!-- 消息 -->
        <span class="text-sm flex-1">{{ toast.message }}</span>
        <!-- 关闭按钮 -->
        <button
          class="w-6 h-6 rounded-full hover:bg-white/20 flex items-center justify-center transition-colors shrink-0"
          @click="toastStore.remove(toast.id)"
        >
          ✕
        </button>
      </div>
    </TransitionGroup>
  </div>
</template>

<style scoped>
.toast-enter-active,
.toast-leave-active {
  transition: all 0.3s ease;
}
.toast-enter-from {
  opacity: 0;
  transform: translateX(100%);
}
.toast-leave-to {
  opacity: 0;
  transform: translateX(100%);
}
</style>
