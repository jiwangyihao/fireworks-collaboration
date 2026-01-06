<!-- src/components/editor/EditorToolbar.vue -->
<script setup lang="ts">
/**
 * EditorToolbar - 编辑器工具栏 (E2.5)
 *
 * 提供快速格式化和块插入功能
 * 与 Slash Menu 配合使用
 */
import { computed } from "vue";
import { useDocumentStore } from "@/stores/document";
import BaseIcon from "@/components/BaseIcon.vue";

const docStore = useDocumentStore();

const isDirty = computed(() => docStore.isDirty);
const isSaving = computed(() => docStore.isSaving);

// 发出事件供父组件处理
const emit = defineEmits<{
  save: [];
  insertBlock: [type: string, props?: Record<string, any>];
  formatText: [format: string];
}>();

// 保存文档
async function handleSave() {
  emit("save");
}

// 插入块
function insertBlock(type: string, props?: Record<string, any>) {
  emit("insertBlock", type, props);
}

// 格式化文本
function formatText(format: string) {
  emit("formatText", format);
}
</script>

<template>
  <div
    class="editor-toolbar flex items-center gap-1 px-3 py-2 bg-base-200/50 border-b border-base-300"
  >
    <!-- 文本格式化 -->
    <div class="join">
      <button
        class="btn btn-xs btn-ghost join-item tooltip tooltip-bottom"
        data-tip="粗体 (Ctrl+B)"
        @click="formatText('bold')"
      >
        <BaseIcon icon="ph--text-b-bold" size="sm" />
      </button>
      <button
        class="btn btn-xs btn-ghost join-item tooltip tooltip-bottom"
        data-tip="斜体 (Ctrl+I)"
        @click="formatText('italic')"
      >
        <BaseIcon icon="ph--text-italic-bold" size="sm" />
      </button>
      <button
        class="btn btn-xs btn-ghost join-item tooltip tooltip-bottom"
        data-tip="删除线"
        @click="formatText('strikethrough')"
      >
        <BaseIcon icon="ph--text-strikethrough-bold" size="sm" />
      </button>
      <button
        class="btn btn-xs btn-ghost join-item tooltip tooltip-bottom"
        data-tip="行内代码"
        @click="formatText('code')"
      >
        <BaseIcon icon="ph--code-bold" size="sm" />
      </button>
    </div>

    <div class="divider divider-horizontal mx-0.5 h-5" />

    <!-- 标题下拉 -->
    <div class="dropdown dropdown-bottom">
      <label
        tabindex="0"
        class="btn btn-xs btn-ghost gap-1 tooltip tooltip-bottom"
        data-tip="插入标题"
      >
        <BaseIcon icon="ph--text-h-bold" size="sm" />
        <BaseIcon icon="ph--caret-down" size="xs" />
      </label>
      <ul
        tabindex="0"
        class="dropdown-content z-50 menu p-1 shadow-lg bg-base-100 rounded-box w-32 border border-base-300"
      >
        <li>
          <a class="text-sm py-1" @click="insertBlock('heading', { level: 1 })">
            <span class="text-lg font-bold">H1</span> 标题 1
          </a>
        </li>
        <li>
          <a class="text-sm py-1" @click="insertBlock('heading', { level: 2 })">
            <span class="text-base font-bold">H2</span> 标题 2
          </a>
        </li>
        <li>
          <a class="text-sm py-1" @click="insertBlock('heading', { level: 3 })">
            <span class="text-sm font-bold">H3</span> 标题 3
          </a>
        </li>
      </ul>
    </div>

    <!-- 插入块下拉 -->
    <div class="dropdown dropdown-bottom">
      <label
        tabindex="0"
        class="btn btn-xs btn-ghost gap-1 tooltip tooltip-bottom"
        data-tip="插入块"
      >
        <BaseIcon icon="ph--plus-circle-bold" size="sm" />
        <BaseIcon icon="ph--caret-down" size="xs" />
      </label>
      <ul
        tabindex="0"
        class="dropdown-content z-50 menu p-1 shadow-lg bg-base-100 rounded-box w-44 border border-base-300"
      >
        <!-- 基础块 -->
        <li class="menu-title text-xs py-0.5">基础块</li>
        <li>
          <a class="text-sm py-1" @click="insertBlock('bulletListItem')">
            <BaseIcon icon="ph--list-bullets" size="sm" /> 无序列表
          </a>
        </li>
        <li>
          <a class="text-sm py-1" @click="insertBlock('numberedListItem')">
            <BaseIcon icon="ph--list-numbers" size="sm" /> 有序列表
          </a>
        </li>
        <li>
          <a class="text-sm py-1" @click="insertBlock('checkListItem')">
            <BaseIcon icon="ph--check-square" size="sm" /> 任务列表
          </a>
        </li>
        <li>
          <a class="text-sm py-1" @click="insertBlock('quote')">
            <BaseIcon icon="ph--quotes" size="sm" /> 引用
          </a>
        </li>

        <div class="divider my-0.5" />

        <!-- 高级块 -->
        <li class="menu-title text-xs py-0.5">高级块</li>
        <li>
          <a class="text-sm py-1" @click="insertBlock('codeBlock')">
            <BaseIcon icon="ph--code-block" size="sm" /> 代码块
          </a>
        </li>
        <li>
          <a class="text-sm py-1" @click="insertBlock('table')">
            <BaseIcon icon="ph--table" size="sm" /> 表格
          </a>
        </li>
        <li>
          <a class="text-sm py-1" @click="insertBlock('math')">
            <BaseIcon icon="ph--function" size="sm" /> 公式
          </a>
        </li>
        <li>
          <a class="text-sm py-1" @click="insertBlock('mermaid')">
            <BaseIcon icon="ph--flow-arrow" size="sm" /> Mermaid
          </a>
        </li>

        <div class="divider my-0.5" />

        <!-- VitePress -->
        <li class="menu-title text-xs py-0.5">VitePress</li>
        <li>
          <a class="text-sm py-1" @click="insertBlock('container')">
            <BaseIcon icon="ph--warning-circle" size="sm" /> 容器
          </a>
        </li>
        <li>
          <a class="text-sm py-1" @click="insertBlock('vueComponent')">
            <BaseIcon icon="simple-icons--vuedotjs" size="sm" /> Vue 组件
          </a>
        </li>
        <li>
          <a class="text-sm py-1" @click="insertBlock('include')">
            <BaseIcon icon="ph--link-simple" size="sm" /> @include
          </a>
        </li>
      </ul>
    </div>

    <div class="divider divider-horizontal mx-0.5 h-5" />

    <!-- 链接和图片 -->
    <button
      class="btn btn-xs btn-ghost tooltip tooltip-bottom"
      data-tip="插入链接 (Ctrl+K)"
      @click="formatText('link')"
    >
      <BaseIcon icon="ph--link-bold" size="sm" />
    </button>
    <button
      class="btn btn-xs btn-ghost tooltip tooltip-bottom"
      data-tip="插入图片"
      @click="insertBlock('image')"
    >
      <BaseIcon icon="ph--image-bold" size="sm" />
    </button>
  </div>
</template>

<style scoped>
.editor-toolbar .divider {
  margin: 0;
}

.editor-toolbar .dropdown-content {
  margin-top: 0.25rem;
}

.editor-toolbar .menu-title {
  color: var(--fallback-bc, oklch(var(--bc) / 0.5));
}
</style>
