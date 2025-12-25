<script setup lang="ts">
/**
 * BaseIcon - 图标组件
 *
 * 支持 Iconify 图标和自定义 SVG 图标
 * - Iconify 图标通过 icon prop 指定，格式为 "{prefix}:{name}" 或 "{prefix}--{name}"
 * - 自定义 SVG 通过默认插槽传入
 */
import { Icon } from "@iconify/vue";
import { computed } from "vue";

const props = withDefaults(
  defineProps<{
    /** Iconify 图标名称，格式: "{prefix}:{name}" 或 "{prefix}--{name}" */
    icon?: string;
    /** 图标尺寸 */
    size?: "xs" | "sm" | "md" | "lg" | "xl" | "2xl";
    /** 旋转动画 */
    spin?: boolean;
    /** 脉冲动画（步进旋转） */
    pulse?: boolean;
    /** 翻转方向 */
    flip?: "horizontal" | "vertical" | "both";
    /** 旋转角度 */
    rotate?: 90 | 180 | 270;
  }>(),
  {
    size: "md",
    spin: false,
    pulse: false,
  }
);

/** 尺寸映射 (px) */
const sizeMap: Record<string, string> = {
  xs: "12",
  sm: "16",
  md: "20",
  lg: "24",
  xl: "32",
  "2xl": "40",
};

/** 将 "--" 格式转换为 ":" 格式 */
const normalizedIcon = computed(() => {
  if (!props.icon) return "";
  // 将 "mdi--github" 格式转为 "mdi:github"
  return props.icon.replace("--", ":");
});

/** Iconify flip 属性 */
const iconFlip = computed(() => {
  if (props.flip === "both") return "horizontal,vertical";
  return props.flip;
});
</script>

<template>
  <span
    class="inline-flex items-center justify-center shrink-0"
    :class="{
      'animate-spin': spin,
      'animate-pulse': pulse,
    }"
  >
    <!-- Iconify icon -->
    <Icon
      v-if="icon"
      :icon="normalizedIcon"
      :width="sizeMap[size]"
      :height="sizeMap[size]"
      :flip="iconFlip"
      :rotate="rotate ? rotate / 90 : undefined"
    />
    <!-- Custom SVG slot -->
    <slot v-else />
  </span>
</template>
