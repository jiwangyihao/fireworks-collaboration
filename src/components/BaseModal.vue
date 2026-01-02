<script setup lang="ts">
/**
 * BaseModal - 基础模态框组件
 *
 * 封装通用的 Dialog 逻辑、样式和交互
 */
import { ref, watch } from "vue";

const props = withDefaults(
  defineProps<{
    /** 标题 */
    title?: string;
    /** 是否显示 (v-model) */
    modelValue?: boolean;
    /** 确认按钮文字 */
    confirmText?: string;
    /** 取消按钮文字 */
    cancelText?: string;
    /** 确认按钮样式变体 */
    confirmVariant?: "primary" | "error" | "warning";
    /** 是否禁用确认按钮 */
    disabled?: boolean;
    /** 是否隐藏原本的底部按钮栏 (用于自定义操作区) */
    hideActions?: boolean;
  }>(),
  {
    title: "提示",
    modelValue: false,
    confirmText: "确认",
    cancelText: "取消",
    confirmVariant: "primary",
    disabled: false,
    hideActions: false,
  }
);

const emit = defineEmits<{
  (e: "update:modelValue", value: boolean): void;
  (e: "confirm"): void;
  (e: "cancel"): void;
}>();

const dialogRef = ref<HTMLDialogElement | null>(null);

// 监听 modelValue 控制显示/隐藏
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

function handleCancel() {
  emit("cancel");
  emit("update:modelValue", false);
}

function handleConfirm() {
  if (props.disabled) return;
  emit("confirm");
  // 注意：Confirm 不自动关闭，交由父组件决定（例如异步操作完成后关闭）
  // 但为了兼容简单的 ConfirmModal，如果父组件没有处理关闭，可能需要手动关闭？
  // 之前的逻辑是：ConfirmModal 在 handleConfirm 里手动 emit false。
  // BaseModal 可以只 emit confirm，由父组件 logic 决定何时关闭。
  // 但为了方便，对于 ConfirmModal 这种简单场景，通常 confirm 后就关闭。
  // 让我们保持简单：BaseModal 只负责 emit confirm。具体关闭逻辑由父组件控制。
  // 不过 ConfirmModal 之前的实现是: emit('confirm'); emit('update:modelValue', false);
  // 所以 BaseModal 最好不要自动关闭，以支持 InputModal 的校验场景。
}

function handleBackdropClick() {
  handleCancel();
}
</script>

<template>
  <dialog ref="dialogRef" class="modal" @close="handleCancel">
    <div class="modal-box p-4">
      <h3 v-if="title" class="font-bold text-lg mt-0 mb-0">{{ title }}</h3>

      <div class="py-4 text-base-content/80 text-sm">
        <slot></slot>
      </div>

      <div v-if="!hideActions" class="modal-action mt-0">
        <button class="btn btn-sm btn-ghost" @click="handleCancel">
          {{ cancelText }}
        </button>
        <button
          class="btn btn-sm"
          :class="`btn-${confirmVariant}`"
          :disabled="disabled"
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
