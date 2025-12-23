<script setup lang="ts">
/**
 * BaseBadge - 基础徽章组件
 *
 * 提供统一的徽章样式，支持多种变体和尺寸
 */

withDefaults(
  defineProps<{
    /** 徽章样式变体 */
    variant?:
      | "primary"
      | "secondary"
      | "accent"
      | "success"
      | "warning"
      | "error"
      | "info"
      | "ghost"
      | "outline";
    /** 徽章尺寸 */
    size?: "xs" | "sm" | "md" | "lg";
  }>(),
  {
    variant: "ghost",
    size: "sm",
  }
);
</script>

<template>
  <span class="badge" :class="[`badge-${variant}`, `badge-${size}`]">
    <slot></slot>
  </span>
</template>
