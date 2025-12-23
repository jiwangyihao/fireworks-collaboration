<script setup lang="ts">
/**
 * LanguageBar - 语言分布进度条组件
 *
 * 显示仓库的语言分布情况
 */
import { computed } from "vue";

const props = defineProps<{
  /** 语言字节数统计 */
  languages: Record<string, number>;
  /** 是否显示图例 */
  showLegend?: boolean;
}>();

// 语言颜色映射
const languageColors: Record<string, string> = {
  TypeScript: "bg-blue-500",
  JavaScript: "bg-yellow-400",
  Vue: "bg-purple-500",
  Rust: "bg-orange-500",
  CSS: "bg-emerald-500",
  HTML: "bg-red-400",
  Python: "bg-blue-400",
  Go: "bg-cyan-500",
  Java: "bg-amber-600",
  default: "bg-primary",
};

// 计算语言百分比
const percentages = computed(() => {
  const total = Object.values(props.languages).reduce((a, b) => a + b, 0);
  if (total === 0) return {};

  const result: Record<string, number> = {};
  for (const [lang, bytes] of Object.entries(props.languages)) {
    result[lang] = Math.round((bytes / total) * 100);
  }
  return result;
});

// 获取语言颜色类
function getColorClass(lang: string): string {
  return languageColors[lang] || languageColors.default;
}
</script>

<template>
  <div v-if="Object.keys(languages).length">
    <!-- 进度条 -->
    <div class="flex h-2 rounded-full overflow-hidden bg-base-300">
      <div
        v-for="(percent, lang) in percentages"
        :key="lang"
        :style="{ width: `${percent}%` }"
        class="h-full"
        :title="`${lang}: ${percent}%`"
        :class="getColorClass(lang as string)"
      ></div>
    </div>

    <!-- 图例 -->
    <div
      v-if="showLegend"
      class="flex flex-wrap gap-2 mt-1 text-[10px] text-base-content/60"
    >
      <span
        v-for="(percent, lang) in percentages"
        :key="lang"
        class="flex items-center gap-1"
      >
        <span
          class="w-1.5 h-1.5 rounded-full"
          :class="getColorClass(lang as string)"
        ></span>
        {{ lang }} {{ percent }}%
      </span>
    </div>
  </div>
</template>
