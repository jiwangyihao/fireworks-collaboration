<script setup lang="ts">
import { ref } from "vue";
import BaseIcon from "@/components/BaseIcon.vue";
import BaseMenu from "./BaseMenu.vue";
import BasePopover from "./BasePopover.vue";

interface Props {
  label: string;
  icon?: string;
}

defineProps<Props>();

const isOpen = ref(false);
const triggerRef = ref<HTMLElement | null>(null);
let closeTimer: ReturnType<typeof setTimeout> | null = null;
const OPEN_DELAY = 50; //ms
const CLOSE_DELAY = 200; //ms

function handleMouseEnter() {
  if (closeTimer) {
    clearTimeout(closeTimer);
    closeTimer = null;
  }
  // Optional open delay
  setTimeout(() => {
    isOpen.value = true;
  }, OPEN_DELAY);
}

function handleMouseLeave() {
  closeTimer = setTimeout(() => {
    isOpen.value = false;
  }, CLOSE_DELAY);
}
</script>

<template>
  <li
    class="relative w-full"
    @mouseenter="handleMouseEnter"
    @mouseleave="handleMouseLeave"
    ref="triggerRef"
  >
    <div
      role="button"
      class="flex items-center gap-2 w-full py-1.5 px-2 rounded-md border border-transparent hover:border-base-content/20 hover:bg-base-200 transition-all font-medium"
      :class="{ 'bg-base-200': isOpen }"
    >
      <BaseIcon v-if="icon" :icon="icon" size="sm" class="opacity-60" />
      <span class="flex-1 text-left">{{ label }}</span>
      <BaseIcon icon="ph:caret-right" size="xs" class="opacity-50" />
    </div>

    <!-- Submenu Popover -->
    <BasePopover
      :is-open="isOpen"
      :trigger-element="triggerRef"
      placement="right-center"
      :offset="-4"
      :z-index="100005"
    >
      <!-- Increase Z-index to be above parent menu -->
      <div
        @mouseenter="handleMouseEnter"
        @mouseleave="handleMouseLeave"
        class="py-1 pl-2"
      >
        <!-- Wrapper div to capture hover events on the popover content itself -->
        <BaseMenu class="w-56 shadow-xl border border-base-content/10 m-0">
          <slot />
        </BaseMenu>
      </div>
    </BasePopover>
  </li>
</template>
