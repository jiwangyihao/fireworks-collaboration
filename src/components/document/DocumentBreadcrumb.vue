<script setup lang="ts">
import { useRouter } from "vue-router";
import { useDocumentStore } from "../../stores/document";
import BaseIcon from "../BaseIcon.vue";

const router = useRouter();
const docStore = useDocumentStore();

function goBack() {
  router.push("/project");
}
</script>

<template>
  <div class="breadcrumbs text-sm py-2 shrink-0 px-4">
    <ul>
      <li>
        <a @click="goBack" class="cursor-pointer">
          <BaseIcon icon="ph--folder" size="sm" />
          项目
        </a>
      </li>
      <li>
        <span>{{ docStore.projectName }}</span>
      </li>
      <!-- Future: Add active document path segments here -->
    </ul>
  </div>
</template>
