<template>
  <div class="h-full flex flex-col">
    <div
      v-if="loading"
      class="flex-1 flex items-center justify-center text-base-content/50"
    >
      <span class="loading loading-spinner loading-sm"></span>
    </div>

    <div
      v-else-if="!tree"
      class="flex-1 flex items-center justify-center text-base-content/50 text-sm"
    >
      暂无文档
    </div>

    <ul v-else class="menu menu-xs w-full p-0 m-0! space-y-1 not-prose">
      <!-- 如果 root 是文件夹，直接展示其子节点 -->
      <template v-if="tree.nodeType === 'folder' && tree.children">
        <DocumentTreeItem
          v-for="child in tree.children"
          :key="child.path"
          :node="child"
          :selected-path="selectedPath"
          @select="handleSelect"
          @contextmenu="handleContextMenu"
        />
      </template>

      <!-- 如果 root 只是单个文件（虽然不常见）或没有子节点 -->
      <DocumentTreeItem
        v-else
        :node="tree"
        :selected-path="selectedPath"
        @select="handleSelect"
        @contextmenu="handleContextMenu"
      />
    </ul>
  </div>
</template>

<script setup lang="ts">
import { type DocTreeNode } from "../../api/vitepress";
import DocumentTreeItem from "./DocumentTreeItem.vue";

defineProps<{
  tree: DocTreeNode | null;
  loading?: boolean;
  selectedPath?: string | null;
}>();

const emit = defineEmits<{
  (e: "select", node: DocTreeNode): void;
  (e: "contextmenu", payload: { event: MouseEvent; node: DocTreeNode }): void;
}>();

function handleSelect(node: DocTreeNode) {
  emit("select", node);
}

function handleContextMenu(payload: { event: MouseEvent; node: DocTreeNode }) {
  emit("contextmenu", payload);
}
</script>
