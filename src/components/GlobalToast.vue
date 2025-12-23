<script setup lang="ts">
import { storeToRefs } from "pinia";
import { useToastStore } from "../stores/toast";

const toastStore = useToastStore();
const { toasts } = storeToRefs(toastStore);

function getAlertClass(type: string) {
  switch (type) {
    case "success":
      return "alert-success";
    case "warning":
      return "alert-warning";
    case "error":
      return "alert-error";
    default:
      return "alert-info";
  }
}
</script>

<template>
  <div class="toast toast-end toast-bottom z-50">
    <TransitionGroup name="toast">
      <div
        v-for="toast in toasts"
        :key="toast.id"
        class="alert shadow-lg"
        :class="getAlertClass(toast.type)"
      >
        <span class="text-sm break-words">{{ toast.message }}</span>
        <button
          class="btn btn-sm btn-ghost btn-square"
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
