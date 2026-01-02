<script setup lang="ts">
/**
 * ConfirmModal - 确认对话框组件
 *
 * (Refactored to use BaseModal)
 */
import BaseModal from "./BaseModal.vue";

const props = withDefaults(
  defineProps<{
    title?: string;
    confirmText?: string;
    cancelText?: string;
    confirmVariant?: "primary" | "error" | "warning";
    modelValue?: boolean;
  }>(),
  {
    title: "确认",
    confirmText: "确认",
    cancelText: "取消",
    confirmVariant: "primary",
    modelValue: false,
  }
);

const emit = defineEmits<{
  (e: "update:modelValue", value: boolean): void;
  (e: "confirm"): void;
  (e: "cancel"): void;
}>();

function handleConfirm() {
  emit("confirm");
  emit("update:modelValue", false); // Auto-close on confirm
}
</script>

<template>
  <BaseModal
    :model-value="modelValue"
    :title="title"
    :confirm-text="confirmText"
    :cancel-text="cancelText"
    :confirm-variant="confirmVariant"
    @update:model-value="emit('update:modelValue', $event)"
    @confirm="handleConfirm"
    @cancel="emit('cancel')"
  >
    <slot></slot>
  </BaseModal>
</template>
