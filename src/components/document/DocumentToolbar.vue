<script setup lang="ts">
import { computed } from "vue";
import { useRouter } from "vue-router";
import { useDocumentStore } from "../../stores/document";
import BaseIcon from "../BaseIcon.vue";
import DevServerStatus from "./DevServerStatus.vue";

const router = useRouter();
const docStore = useDocumentStore();

const projectName = computed(() => docStore.projectName);
const worktreePath = computed(() => docStore.worktreePath);
const installed = computed(() => docStore.dependencyStatus?.installed);
const isLoading = computed(() => docStore.isLoading);

function goBack() {
  router.push("/project");
}

function handleRefresh() {
  docStore.detectProject();
}
</script>

<template>
  <div
    class="flex items-center gap-4 h-14 shrink-0 px-4 border-b border-base-200"
  >
    <button class="btn btn-sm btn-ghost" @click="goBack">
      <BaseIcon icon="ph--arrow-left" size="sm" />
      返回
    </button>
    <div class="flex flex-col">
      <h2 class="m-0! text-lg font-semibold leading-tight">
        {{ projectName }}
      </h2>
      <span class="text-xs text-base-content/50 leading-tight">{{
        worktreePath
      }}</span>
    </div>

    <div class="ml-auto flex items-center gap-2">
      <!-- Dev Server Status -->
      <DevServerStatus
        v-if="installed && worktreePath"
        :projectPath="worktreePath"
      />

      <div class="divider divider-horizontal mx-0"></div>

      <button
        class="btn btn-sm btn-ghost"
        :disabled="isLoading"
        @click="handleRefresh"
        title="刷新"
      >
        <BaseIcon
          icon="ph--arrows-clockwise"
          size="sm"
          :class="{ 'animate-spin': isLoading }"
        />
      </button>
    </div>
  </div>
</template>
