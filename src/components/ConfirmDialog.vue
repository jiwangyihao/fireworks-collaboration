<template>
  <dialog :open="show" class="modal">
    <div class="modal-box">
      <h3 class="font-bold text-lg">{{ title }}</h3>
      <p class="py-4">{{ message }}</p>
      <div v-if="details" class="text-sm text-base-content/70 mb-4">
        {{ details }}
      </div>
      <div class="modal-action">
        <button class="btn" @click="onCancel">取消</button>
        <button :class="buttonClass" @click="onConfirm">
          {{ confirmText }}
        </button>
      </div>
    </div>
    <form method="dialog" class="modal-backdrop" @submit.prevent="onCancel">
      <button type="button" @click="onCancel">关闭</button>
    </form>
  </dialog>
</template>

<script setup lang="ts">
import { computed } from 'vue';

export interface ConfirmDialogProps {
  show: boolean;
  title: string;
  message: string;
  details?: string;
  confirmText?: string;
  variant?: 'danger' | 'warning' | 'info';
}

const props = withDefaults(defineProps<ConfirmDialogProps>(), {
  confirmText: '确认',
  variant: 'danger',
});

const emit = defineEmits<{
  confirm: [];
  cancel: [];
}>();

const buttonClass = computed(() => {
  const baseClass = 'btn';
  switch (props.variant) {
    case 'danger':
      return `${baseClass} btn-error`;
    case 'warning':
      return `${baseClass} btn-warning`;
    case 'info':
      return `${baseClass} btn-info`;
    default:
      return `${baseClass} btn-primary`;
  }
});

const onConfirm = () => {
  emit('confirm');
};

const onCancel = () => {
  emit('cancel');
};
</script>
