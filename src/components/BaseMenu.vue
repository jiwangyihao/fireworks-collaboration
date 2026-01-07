<script setup lang="ts">
interface Props {
  size?: "xs" | "sm" | "md";
  class?: string;
}

withDefaults(defineProps<Props>(), {
  size: "xs",
});
</script>

<template>
  <ul
    class="menu bg-base-100 rounded-xl shadow-xl border border-base-content/10 p-1.5 gap-1 overflow-hidden"
    :class="[`menu-${size}`, $props.class]"
  >
    <slot />
  </ul>
</template>
