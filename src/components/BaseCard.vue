<script setup lang="ts">
/**
 * BaseCard - 基础卡片容器组件
 *
 * 提供统一的卡片样式：边框、背景、内边距
 */

withDefaults(
  defineProps<{
    /** 卡片样式变体 */
    variant?: "default" | "gradient";
    /** 内边距大小 */
    padding?: "none" | "sm" | "md" | "lg";
    /** 是否为弹性布局 */
    flex?: boolean;
  }>(),
  {
    variant: "default",
    padding: "md",
    flex: false,
  }
);
</script>

<template>
  <div
    class="card border-2 border-base-content/15"
    :class="[
      {
        'bg-base-100': variant === 'default',
        'bg-gradient-to-r from-secondary/5 to-primary/5':
          variant === 'gradient',
        'flex-1': flex,
      },
    ]"
  >
    <div
      class="card-body gap-2"
      :class="{
        'p-0': padding === 'none',
        'p-2': padding === 'sm',
        'p-4': padding === 'md',
        'p-6': padding === 'lg',
      }"
    >
      <slot></slot>
    </div>
  </div>
</template>
