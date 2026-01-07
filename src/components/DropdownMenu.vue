<script setup lang="ts">
import { computed } from "vue";
import BaseMenu from "./BaseMenu.vue";
import BasePopover from "./BasePopover.vue";

interface Props {
  isOpen: boolean;
  triggerRect?: DOMRect | null;
  triggerElement?: HTMLElement | null;
  position?: "bottom-left" | "bottom-center" | "bottom-right";
  width?: number | "trigger" | "auto";
  offset?: number;
}

const props = withDefaults(defineProps<Props>(), {
  position: "bottom-left",
  width: "auto",
  offset: 4,
});

// Map legacy 'position' to BasePopover 'placement'
const placement = computed(() => {
  if (props.position === "bottom-center") {
    return "bottom-center";
  }
  return props.position === "bottom-right" ? "bottom-end" : "bottom-start";
});
</script>

<template>
  <BasePopover
    :is-open="isOpen"
    :trigger-element="triggerElement"
    :trigger-rect="triggerRect"
    :placement="placement"
    :width="width"
    :offset="offset"
  >
    <BaseMenu>
      <slot />
    </BaseMenu>
  </BasePopover>
</template>
