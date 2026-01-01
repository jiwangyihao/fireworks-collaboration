<template>
  <li class="w-full max-w-full m-0 p-0">
    <!-- 文件夹 -->
    <details
      v-if="node.nodeType === 'folder'"
      :open="isExpanded"
      class="w-full max-w-full"
    >
      <summary
        @click.prevent="toggleExpand"
        @contextmenu.prevent="handleContextMenu"
        class="list-none marker:hidden [&::-webkit-details-marker]:hidden group flex items-center gap-2 px-3 py-2 rounded-lg cursor-pointer transition-all duration-200 border border-transparent hover:bg-base-200 hover:border-base-content/10 select-none text-sm text-base-content/80 hover:text-base-content w-full max-w-full"
      >
        <BaseIcon
          :icon="isExpanded ? 'ph--folder-open-fill' : 'ph--folder-fill'"
          :class="[
            'w-5 h-5 flex-shrink-0 transition-transform duration-200',
            isExpanded
              ? 'text-yellow-400 drop-shadow-sm'
              : 'text-yellow-400/90 group-hover:text-yellow-400 drop-shadow-sm',
          ]"
        />
        <span class="truncate font-medium flex-1 min-w-0">{{
          displayName
        }}</span>

        <!-- Git 状态标记 -->
        <span
          v-if="node.gitStatus && node.gitStatus !== 'clean'"
          class="badge badge-xs badge-outline ml-auto mr-0 font-normal opacity-80"
          :class="statusBadgeClass"
        >
          {{ statusLabel }}
        </span>
      </summary>
      <ul
        v-if="node.children && node.children.length > 0"
        class="pl-3 border-l border-base-200 ml-2.5 space-y-0.5"
      >
        <DocumentTreeItem
          v-for="child in node.children"
          :key="child.path"
          :node="child"
          :selected-path="selectedPath"
          @select="$emit('select', $event)"
          @contextmenu="$emit('contextmenu', $event)"
        />
      </ul>
      <div v-else class="pl-8 text-xs text-base-content/40 py-1.5 italic">
        (空文件夹)
      </div>
    </details>

    <!-- 文件 -->
    <a
      v-else
      @click="selectFile"
      @contextmenu.prevent="handleContextMenu"
      :class="[
        'group flex items-center gap-2 px-3 py-2 rounded-lg cursor-pointer transition-all duration-200 border border-transparent text-sm select-none w-full max-w-full',
        isSelected
          ? 'bg-primary/10 text-primary font-medium border-primary/20'
          : 'text-base-content/70 hover:bg-base-200 hover:border-base-content/10 hover:text-base-content',
        statusColorClass,
      ]"
    >
      <BaseIcon
        icon="ph--file-text"
        :class="[
          'w-5 h-5 flex-shrink-0',
          isSelected
            ? 'text-primary'
            : 'text-base-content/40 group-hover:text-base-content/60',
        ]"
      />
      <span class="truncate flex-1 min-w-0">{{ displayName }}</span>

      <!-- Git 状态标记 -->
      <span
        v-if="node.gitStatus && node.gitStatus !== 'clean'"
        class="w-1.5 h-1.5 rounded-full ring-2 ring-base-100"
        :class="statusDotClass"
        :title="statusLabel"
      >
      </span>
    </a>
  </li>
</template>

<script setup lang="ts">
import { computed, ref } from "vue";
import { type DocTreeNode } from "../../api/vitepress";
import BaseIcon from "../BaseIcon.vue";

const props = defineProps<{
  node: DocTreeNode;
  selectedPath?: string | null;
}>();

const emit = defineEmits<{
  (e: "select", node: DocTreeNode): void;
  (e: "contextmenu", payload: { event: MouseEvent; node: DocTreeNode }): void;
}>();

const isExpanded = ref(false);

const isSelected = computed(() => {
  return props.selectedPath === props.node.path;
});

const displayName = computed(() => {
  return props.node.title || props.node.name.replace(/\.md$/i, "");
});

// 状态颜色映射
const statusColorClass = computed(() => {
  switch (props.node.gitStatus) {
    case "modified":
      return "text-warning";
    case "staged":
      return "text-success";
    case "untracked":
      return "text-info"; // 使用 info 蓝色表示 untracked
    case "conflict":
      return "text-error";
    default:
      return "";
  }
});

// 状态 Badge 样式 (文件夹)
const statusBadgeClass = computed(() => {
  switch (props.node.gitStatus) {
    case "modified":
      return "badge-warning";
    case "staged":
      return "badge-success";
    case "untracked":
      return "badge-info";
      return "";
    case "conflict":
      return "badge-error";
    default:
      return "";
  }
});

// 状态圆点样式 (文件)
const statusDotClass = computed(() => {
  switch (props.node.gitStatus) {
    case "modified":
      return "bg-warning";
    case "staged":
      return "bg-success";
    case "untracked":
      return "bg-info";
    case "conflict":
      return "bg-error";
    default:
      return "";
  }
});

const statusLabel = computed(() => {
  switch (props.node.gitStatus) {
    case "modified":
      return "M";
    case "staged":
      return "S";
    case "untracked":
      return "U";
    case "conflict":
      return "C";
    default:
      return "";
  }
});

function toggleExpand() {
  isExpanded.value = !isExpanded.value;
}

function selectFile() {
  emit("select", props.node);
}

function handleContextMenu(event: MouseEvent) {
  emit("contextmenu", { event, node: props.node });
}
</script>
