<script setup lang="ts">
/**
 * StatusCard - 状态卡片组件
 *
 * 基于 BaseCard，用于显示带标题、图标、徽章和加载状态的卡片
 */
import BaseCard from "./BaseCard.vue";
import BaseBadge from "./BaseBadge.vue";

withDefaults(
  defineProps<{
    /** 卡片标题 */
    title: string;
    /** 标题前的图标（emoji） */
    icon?: string;
    /** 右侧徽章文本 */
    badge?: string;
    /** 徽章样式变体 */
    badgeVariant?:
      | "primary"
      | "secondary"
      | "accent"
      | "success"
      | "warning"
      | "error"
      | "info";
    /** 是否显示加载状态 */
    loading?: boolean;
    /** 卡片变体 */
    variant?: "default" | "gradient";
    /** 是否为弹性布局 */
    flex?: boolean;
  }>(),
  {
    variant: "default",
    loading: false,
    flex: false,
  }
);
</script>

<template>
  <BaseCard :variant="variant" :flex="flex">
    <!-- 卡片头部 -->
    <div class="flex items-center justify-between">
      <h4 class="font-semibold text-sm flex items-center gap-2">
        <span v-if="icon">{{ icon }}</span>
        {{ title }}
        <span v-if="loading" class="loading loading-spinner loading-xs"></span>
      </h4>
      <div class="flex items-center gap-2">
        <slot name="header-actions"></slot>
        <BaseBadge v-if="badge" :variant="badgeVariant" size="sm">
          {{ badge }}
        </BaseBadge>
      </div>
    </div>

    <!-- 卡片内容 -->
    <slot></slot>
  </BaseCard>
</template>
