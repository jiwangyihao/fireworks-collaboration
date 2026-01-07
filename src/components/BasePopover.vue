<script setup lang="ts">
import { ref, watch, nextTick, onMounted, onUnmounted } from "vue";

interface Props {
  isOpen: boolean;
  triggerElement?: HTMLElement | null;
  triggerRect?: DOMRect | null;
  /**
   * Placement strategy
   * Syntax: [side]-[alignment]
   * - Side: top, bottom, left, right
   * - Alignment: start, center, end
   */
  placement?:
    | "top-start"
    | "top-center"
    | "top-end"
    | "bottom-start"
    | "bottom-center"
    | "bottom-end"
    | "left-start"
    | "left-center"
    | "left-end"
    | "right-start"
    | "right-center"
    | "right-end";
  offset?: number;
  width?: number | "trigger" | "auto";
  zIndex?: number;
}

const props = withDefaults(defineProps<Props>(), {
  placement: "bottom-start",
  offset: 4,
  width: "auto",
  zIndex: 99999,
});

const style = ref<Record<string, string>>({
  top: "0px",
  left: "0px",
  width: "auto",
});
const contentRef = ref<HTMLElement | null>(null);

function updatePosition() {
  if (!props.isOpen) return;

  const rect =
    props.triggerRect || props.triggerElement?.getBoundingClientRect();
  if (!rect) return;

  const popoverRect = contentRef.value?.getBoundingClientRect();
  const popWidth = popoverRect?.width || 0;
  const popHeight = popoverRect?.height || 0;

  const scrollTop = window.scrollY || document.documentElement.scrollTop;
  const scrollLeft = window.scrollX || document.documentElement.scrollLeft;

  let top = 0;
  let left = 0;

  const [side, align] = props.placement.split("-");

  // Vertical Position (Top/Bottom)
  if (side === "top") {
    top = rect.top - popHeight - props.offset + scrollTop;
  } else if (side === "bottom") {
    top = rect.bottom + props.offset + scrollTop;
  } else {
    // Left/Right: vertically aligned based on 'align'
    if (align === "start") {
      top = rect.top + scrollTop;
    } else if (align === "center") {
      top = rect.top + rect.height / 2 - popHeight / 2 + scrollTop;
    } else if (align === "end") {
      top = rect.bottom - popHeight + scrollTop;
    }
  }

  // Horizontal Position (Left/Right)
  if (side === "left") {
    left = rect.left - popWidth - props.offset + scrollLeft;
  } else if (side === "right") {
    left = rect.right + props.offset + scrollLeft;
  } else {
    // Top/Bottom: horizontally aligned based on 'align'
    if (align === "start") {
      left = rect.left + scrollLeft;
    } else if (align === "center") {
      left = rect.left + rect.width / 2 - popWidth / 2 + scrollLeft;
    } else if (align === "end") {
      left = rect.right - popWidth + scrollLeft;
    }
  }

  // Width Handling
  let w = "auto";
  if (props.width === "trigger") {
    w = `${rect.width}px`;
    // Re-adjust 'center' if width changed to match trigger?
    // Usually width='trigger' implies alignment matches width, so center is implicitly covered if size matches.
  } else if (typeof props.width === "number") {
    w = `${props.width}px`;
  }

  style.value = {
    top: `${top}px`,
    left: `${left}px`,
    width: w,
  };
}

watch(
  () => [
    props.isOpen,
    props.triggerElement,
    props.triggerRect,
    props.placement,
  ],
  () => {
    if (props.isOpen) {
      // Logic requires popover dimensions for some alignments (center/end).
      // We must render first, then measure.
      // So on open, render at 0,0 invisible? Or just wait 1 tick.
      nextTick(() => {
        updatePosition();
      });
    }
  },
  { immediate: true, deep: true }
);

// Optional: Global resize listener
function onResize() {
  if (props.isOpen) updatePosition();
}
onMounted(() => window.addEventListener("resize", onResize));
onUnmounted(() => window.removeEventListener("resize", onResize));
</script>

<template>
  <Teleport to="body">
    <Transition name="popover-fade">
      <div
        v-if="isOpen"
        ref="contentRef"
        class="fixed"
        :style="{ ...style, zIndex }"
      >
        <slot />
      </div>
    </Transition>
  </Teleport>
</template>

<style scoped>
.popover-fade-enter-active,
.popover-fade-leave-active {
  transition:
    opacity 0.1s ease,
    transform 0.1s ease;
}

.popover-fade-enter-from,
.popover-fade-leave-to {
  opacity: 0;
  transform: translateY(-4px);
}
</style>
