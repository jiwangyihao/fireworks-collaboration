<script setup lang="ts">
/**
 * ConfirmModal - 确认对话框组件
 *
 * 通用的确认/取消对话框，支持自定义内容
 */
import { ref, watch } from "vue";

const props = withDefaults(
  defineProps<{
    /** 对话框标题 */
    title?: string;
    /** 确认按钮文本 */
    confirmText?: string;
    /** 取消按钮文本 */
    cancelText?: string;
    /** 确认按钮变体 */
    confirmVariant?: "primary" | "error" | "warning";
    /** 是否显示 */
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

const dialogRef = ref<HTMLDialogElement | null>(null);

// 同步 modelValue 与 dialog 状态
watch(
  () => props.modelValue,
  (val) => {
    if (val) {
      dialogRef.value?.showModal();
    } else {
      dialogRef.value?.close();
    }
  }
);

function handleConfirm() {
  emit("confirm");
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
    <div class="modal-box p-4">
      <h3 class="font-bold text-lg mt-0 mb-0">{{ title }}</h3>

      <div class="text-base-content/80 text-sm">
        <slot></slot>
      </div>

      <div class="modal-action mt-4">
        <button class="btn btn-ghost" @click="handleCancel">
          {{ cancelText }}
        </button>
        <button
          class="btn"
          :class="`btn-${confirmVariant}`"
          @click="handleConfirm"
        >
          {{ confirmText }}
        </button>
      </div>
    </div>
    <form method="dialog" class="modal-backdrop" @click="handleBackdropClick">
      <button>关闭</button>
    </form>
  </dialog>
</template>
