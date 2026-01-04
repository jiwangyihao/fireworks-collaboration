<!--
  BlockEditor.vue - BlockNote 编辑器的 Vue 包装组件
  
  使用 veaury 将 React 的 BlockNoteEditor 组件包装为 Vue 组件
-->
<script setup lang="ts">
import { ref } from "vue";
import { applyPureReactInVue } from "veaury";
// Import React component
// Put React components in 'react_app' directory to let veaury identify them as React JSX
import { BlockNoteEditor } from "./react_app/BlockNoteEditor";
import type { Block } from "@blocknote/core";

// 将 React 组件包装为 Vue 组件
const BlockNoteEditorVue = applyPureReactInVue(BlockNoteEditor);

// Props
interface Props {
  /** 初始内容（BlockNote Block 格式） */
  initialContent?: Block[];
  /** 是否可编辑 */
  editable?: boolean;
  /** 当前文件路径 */
  filePath?: string;
  /** 项目根目录 */
  projectRoot?: string;
  /** Dev Server 端口 */
  devServerPort?: number;
  /** Dev Server URL */
  devServerUrl?: string;
}

// eslint-disable-next-line @typescript-eslint/no-unused-vars
const props = withDefaults(defineProps<Props>(), {
  editable: true,
  initialContent: undefined,
});

// Emits
const emit = defineEmits<{
  /** 编辑器就绪事件 */
  ready: [editor: unknown];
  /** 内容变更事件 */
  change: [blocks: Block[]];
}>();

// Ensure we pass plain JS objects to React, NOT Vue Proxies!
// This is critical for libraries like BlockNote that perform strict checks
import { toRaw, computed, watch } from "vue";
const rawInitialContent = computed(() => {
  // Deep clone to be absolutely safe and break all reactivity links that might confuse BlockNote
  const val = props.initialContent
    ? JSON.parse(JSON.stringify(props.initialContent))
    : undefined;
  return val;
});

// 编辑器实例引用
const editorInstance = ref<unknown>(null);

/**
 * 处理编辑器就绪
 */
function handleReady(editor: unknown) {
  editorInstance.value = editor;
  emit("ready", editor);
}

/**
 * 处理内容变更
 */
function handleChange(blocks: Block[]) {
  emit("change", blocks);
}

// 暴露编辑器实例供父组件访问
defineExpose({
  editor: editorInstance,
});
</script>

<template>
  <div class="block-editor-container h-full overflow-auto">
    <!-- Use camelCase for Veaury/React props to ensure correct mapping -->
    <BlockNoteEditorVue
      :initialContent="rawInitialContent"
      :editable="editable"
      :filePath="filePath"
      :projectRoot="projectRoot"
      :devServerPort="devServerPort"
      :devServerUrl="devServerUrl"
      :onReady="handleReady"
      :onChange="handleChange"
    />
  </div>
</template>

<style scoped>
.block-editor-container {
  /* 编辑器容器样式 */
  min-height: 300px;
}

/* 覆盖 BlockNote 默认样式以适配 DaisyUI 主题 */
.block-editor-container :deep(.bn-container) {
  font-family: inherit;
}
</style>
