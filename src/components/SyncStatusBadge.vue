<script setup lang="ts">
/**
 * SyncStatusBadge - 同步状态徽章组件
 *
 * 显示仓库/分支的同步状态（已同步/领先/落后）
 */
import BaseBadge from "./BaseBadge.vue";
import BaseIcon from "./BaseIcon.vue";

const props = withDefaults(
  defineProps<{
    /** 领先提交数 */
    ahead?: number;
    /** 落后提交数 */
    behind?: number;
    /** 跟踪分支名（为空时不显示已同步状态） */
    trackingBranch?: string | null;
    /** 是否显示在一行（true时合并显示） */
    inline?: boolean;
  }>(),
  {
    ahead: 0,
    behind: 0,
    inline: false,
  }
);

// 判断是否完全同步
const isSynced = () => {
  return props.trackingBranch && props.ahead === 0 && props.behind === 0;
};
</script>

<template>
  <!-- 已同步状态 -->
  <BaseBadge v-if="isSynced()" variant="success" size="sm" class="gap-1">
    <BaseIcon icon="lucide--check" size="xs" /> 已同步
  </BaseBadge>

  <!-- 领先状态 -->
  <BaseBadge v-if="ahead > 0" variant="info" size="sm" class="gap-1">
    <BaseIcon icon="lucide--arrow-up" size="xs" />{{ ahead }} ahead
  </BaseBadge>

  <!-- 落后状态 -->
  <BaseBadge v-if="behind > 0" variant="warning" size="sm" class="gap-1">
    <BaseIcon icon="lucide--arrow-down" size="xs" />{{ behind }} behind
  </BaseBadge>
</template>
