<script setup lang="ts">
/**
 * InputModal - 输入对话框组件
 *
 * 用于获取用户输入的通用对话框
 */
import { ref, watch, nextTick } from "vue";

const props = withDefaults(
  defineProps<{
    /** 对话框标题 */
    title?: string;
    /** 输入框提示语 */
    placeholder?: string;
    /** 确认按钮文本 */
    confirmText?: string;
    /** 取消按钮文本 */
    cancelText?: string;
    /** 初始值 */
    defaultValue?: string;
    /** 是否显示 */
    modelValue?: boolean;
  }>(),
  {
    title: "请输入",
    placeholder: "",
    confirmText: "确认",
    cancelText: "取消",
    defaultValue: "",
    modelValue: false,
  }
);

const emit = defineEmits<{
  (e: "update:modelValue", value: boolean): void;
  (e: "confirm", value: string): void;
  (e: "cancel"): void;
}>();

const dialogRef = ref<HTMLDialogElement | null>(null);
const inputRef = ref<HTMLInputElement | null>(null);
const inputValue = ref("");

// 同步 modelValue 与 dialog 状态
watch(
  () => props.modelValue,
  async (val) => {
    if (val) {
      inputValue.value = props.defaultValue;
      dialogRef.value?.showModal();
      await nextTick();
      inputRef.value?.focus();
      inputRef.value?.select();
    } else {
      dialogRef.value?.close();
    }
  }
);

function handleConfirm() {
  if (!inputValue.value.trim()) return;
  emit("confirm", inputValue.value);
  emit("update:modelValue", false);
}

function handleCancel() {
  emit("cancel");
  emit("update:modelValue", false);
}

function handleBackdropClick() {
  handleCancel();
}
</script>

<template>
  <dialog ref="dialogRef" class="modal" @close="handleCancel">
    <div class="modal-box">
      <h3 class="font-bold text-lg mb-4">{{ title }}</h3>

      <input
        ref="inputRef"
        v-model="inputValue"
        type="text"
        :placeholder="placeholder"
        class="input input-bordered w-full"
        @keyup.enter="handleConfirm"
      />

      <div class="modal-action">
        <button class="btn btn-ghost" @click="handleCancel">
          {{ cancelText }}
        </button>
        <button class="btn btn-primary" @click="handleConfirm">
          {{ confirmText }}
        </button>
      </div>
    </div>
    <form method="dialog" class="modal-backdrop" @click="handleBackdropClick">
      <button>关闭</button>
    </form>
  </dialog>
</template>
