<script setup lang="ts">
/**
 * InputModal - 输入对话框组件
 *
 * (Refactored to use BaseModal)
 */
import { ref, watch, nextTick } from "vue";
import BaseModal from "./BaseModal.vue";

const props = withDefaults(
  defineProps<{
    title?: string;
    placeholder?: string;
    confirmText?: string;
    cancelText?: string;
    defaultValue?: string;
    modelValue: boolean;
  }>(),
  {
    title: "请输入",
    placeholder: "",
    confirmText: "确认",
    cancelText: "取消",
    defaultValue: "",
  }
);

const emit = defineEmits<{
  (e: "update:modelValue", value: boolean): void;
  (e: "confirm", value: string): void;
  (e: "cancel"): void;
}>();

const inputRef = ref<HTMLInputElement | null>(null);
const inputValue = ref("");

// 同步 modelValue 与 input 聚焦
watch(
  () => props.modelValue,
  async (val) => {
    if (val) {
      inputValue.value = props.defaultValue;
      await nextTick();
      // BaseModal 打开后聚焦输入框
      inputRef.value?.focus();
      inputRef.value?.select();
    }
  }
);

function handleConfirm() {
  if (!inputValue.value.trim()) return;
  emit("confirm", inputValue.value);
  emit("update:modelValue", false);
}
</script>

<template>
  <BaseModal
    :model-value="modelValue"
    :title="title"
    :confirm-text="confirmText"
    :cancel-text="cancelText"
    @update:model-value="emit('update:modelValue', $event)"
    @confirm="handleConfirm"
    @cancel="emit('cancel')"
  >
    <input
      ref="inputRef"
      v-model="inputValue"
      type="text"
      :placeholder="placeholder"
      class="input input-bordered w-full"
      @keyup.enter="handleConfirm"
    />
  </BaseModal>
</template>
