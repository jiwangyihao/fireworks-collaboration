<script setup lang="ts">
/**
 * AvatarGroup - 头像组组件
 *
 * 显示多个用户头像的组合，超出部分显示数字
 */
export interface AvatarItem {
  /** 头像 URL */
  avatarUrl: string;
  /** 用户名/标识 */
  name: string;
  /** 可选链接 */
  url?: string;
}

const props = withDefaults(
  defineProps<{
    /** 头像列表 */
    items: AvatarItem[];
    /** 最多显示数量 */
    max?: number;
    /** 头像尺寸 */
    size?: "xs" | "sm" | "md";
  }>(),
  {
    max: 5,
    size: "sm",
  }
);

// 计算尺寸类
const sizeClass = {
  xs: "w-5",
  sm: "w-6",
  md: "w-8",
}[props.size];

const overflowTextSize = {
  xs: "text-[8px]",
  sm: "text-[9px]",
  md: "text-[10px]",
}[props.size];
</script>

<template>
  <div class="avatar-group -space-x-3">
    <div v-for="item in items.slice(0, max)" :key="item.name" class="avatar">
      <a
        v-if="item.url"
        :href="item.url"
        target="_blank"
        :title="item.name"
        :class="[
          sizeClass,
          'rounded-full ring ring-base-100 hover:ring-primary hover:z-10',
        ]"
      >
        <img :src="item.avatarUrl" :alt="item.name" />
      </a>
      <div
        v-else
        :title="item.name"
        :class="[sizeClass, 'rounded-full ring ring-base-100']"
      >
        <img :src="item.avatarUrl" :alt="item.name" />
      </div>
    </div>

    <!-- 溢出计数 -->
    <div v-if="items.length > max" class="avatar placeholder">
      <div
        :class="[
          sizeClass,
          overflowTextSize,
          'bg-neutral text-neutral-content rounded-full',
        ]"
      >
        +{{ items.length - max }}
      </div>
    </div>
  </div>
</template>
