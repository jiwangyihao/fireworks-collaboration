import { defineStore } from 'pinia';

export type ToastType = 'info' | 'success' | 'warning' | 'error';

export interface Toast {
    id: number;
    type: ToastType;
    message: string;
    duration: number; // ms, 0 = 不自动关闭
}

let toastId = 0;

export const useToastStore = defineStore('toast', {
    state: () => ({
        toasts: [] as Toast[],
    }),

    actions: {
        add(type: ToastType, message: string, duration = 5000) {
            const id = ++toastId;
            this.toasts.push({ id, type, message, duration });

            if (duration > 0) {
                setTimeout(() => {
                    this.remove(id);
                }, duration);
            }

            return id;
        },

        remove(id: number) {
            const index = this.toasts.findIndex(t => t.id === id);
            if (index !== -1) {
                this.toasts.splice(index, 1);
            }
        },

        // 快捷方法
        info(message: string, duration = 5000) {
            return this.add('info', message, duration);
        },

        success(message: string, duration = 3000) {
            return this.add('success', message, duration);
        },

        warning(message: string, duration = 5000) {
            return this.add('warning', message, duration);
        },

        error(message: string, duration = 0) {
            // 错误默认不自动关闭
            return this.add('error', message, duration);
        },

        clear() {
            this.toasts = [];
        },
    },
});
