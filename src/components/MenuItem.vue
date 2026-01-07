<script setup lang="ts">
import BaseIcon from "@/components/BaseIcon.vue";

interface Props {
  icon?: string;
  label?: string;
  active?: boolean;
  disabled?: boolean;
  description?: string;
  shortcut?: string;
}

const props = defineProps<Props>();
const emit = defineEmits<{
  click: [payload: MouseEvent];
}>();

function handleClick(e: MouseEvent) {
  if (props.disabled) return;
  emit("click", e);
}
</script>

<template>
  <li class="w-full overflow-hidden">
    <a
      class="flex flex-col items-start gap-0.5 py-1.5 px-2 rounded-md border border-transparent transition-all w-full overflow-hidden"
      :class="[
        active
          ? '!border-primary bg-primary/5 text-primary font-medium'
          : 'hover:border-base-content/20 hover:bg-base-200 text-base-content',
        disabled ? 'opacity-50 cursor-not-allowed' : 'cursor-pointer',
        description ? '' : 'items-center !flex-row',
      ]"
      @click="handleClick"
    >
      <div class="flex items-center gap-2">
        <BaseIcon
          v-if="icon"
          :icon="icon"
          size="sm"
          :class="{ 'opacity-60': !active, 'text-primary': active }"
        />

        <span class="font-medium flex-1 truncate">
          <slot>{{ label }}</slot>
        </span>

        <!-- Right Slot -->
        <span v-if="shortcut" class="text-[10px] opacity-40 font-mono">{{
          shortcut
        }}</span>
        <slot name="right"></slot>
      </div>

      <span
        v-if="description"
        class="text-[10px] opacity-60 leading-tight truncate w-full pl-0"
        >{{ description }}</span
      >
    </a>
  </li>
</template>
