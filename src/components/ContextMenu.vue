<script setup lang="ts">
import { computed } from "vue";
import BasePopover from "./BasePopover.vue";
import BaseMenu from "./BaseMenu.vue";

interface Props {
  modelValue: boolean;
  x: number;
  y: number;
  zIndex?: number;
}

const props = withDefaults(defineProps<Props>(), {
  zIndex: 50,
});

// Create a virtual DOMRect based on x,y coordinates
const triggerRect = computed(() => {
  const { x, y } = props;
  return {
    top: y,
    bottom: y,
    left: x,
    right: x,
    width: 0,
    height: 0,
    x,
    y,
    toJSON: () => {},
  } as DOMRect;
});
</script>

<template>
  <BasePopover
    :is-open="modelValue"
    :trigger-rect="triggerRect"
    placement="bottom-start"
    :offset="2"
    :z-index="zIndex"
  >
    <!-- 
      BaseMenu handles styling (bg, border, shadow).
      @click.stop prevents closing when clicking empty space inside the menu,
      allowing individual menu items to handle closure or keeping it open.
    -->
    <BaseMenu class="min-w-[160px] max-w-xs" @click.stop>
      <slot />
    </BaseMenu>
  </BasePopover>
</template>
