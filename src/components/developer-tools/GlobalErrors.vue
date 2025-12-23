<!--
  @deprecated This component is deprecated and not currently in use.
  It was previously used for displaying global error/log notifications.
  Consider removing in future cleanup.
-->
<template>
  <div class="fixed bottom-2 right-2 z-50 space-y-2 w-96 max-w-[95vw]">
    <div v-for="l in logs.items" :key="l.id" class="alert shadow">
      <span
        :class="{
          'text-blue-700': l.level === 'info',
          'text-yellow-700': l.level === 'warn',
          'text-red-700': l.level === 'error',
        }"
        >{{ new Date(l.time).toLocaleTimeString() }} · {{ l.message }}</span
      >
    </div>
    <button v-if="logs.items.length" class="btn btn-xs" @click="logs.clear()">
      清空
    </button>
  </div>
</template>

<script setup lang="ts">
import { useLogsStore } from "../../stores/logs";
const logs = useLogsStore();
</script>

<style scoped></style>
