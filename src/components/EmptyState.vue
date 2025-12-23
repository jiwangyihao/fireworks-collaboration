<script setup lang="ts">
/**
 * EmptyState - 空状态组件
 *
 * 用于显示列表为空、功能未启用等状态
 */
withDefaults(
  defineProps<{
    /** 图标（支持 emoji 或 slot） */
    icon?: string;
    /** 主标题 */
    title?: string;
    /** 描述文字 */
    description?: string;
  }>(),
  {
    icon: "📭",
  }
);
</script>

<template>
  <div class="text-center py-6 text-base-content/50">
    <!-- 图标 -->
    <div class="text-3xl mb-2">
      <slot name="icon">{{ icon }}</slot>
    </div>

    <!-- 标题 -->
    <p v-if="title" class="text-sm">{{ title }}</p>
    <slot v-else name="title"></slot>

    <!-- 描述 -->
    <p v-if="description" class="text-xs mt-1">{{ description }}</p>
    <slot v-else name="description"></slot>

    <!-- 操作按钮等额外内容 -->
    <slot></slot>
  </div>
</template>
