// src/composables/usePreviewSync.ts
/**
 * Preview Sync Composable (E2.5)
 *
 * 管理编辑器与 VitePress 预览之间的同步逻辑：
 * - 文档变更时自动刷新预览
 * - 滚动位置同步（预留）
 */

import { ref, watch, type Ref } from "vue";
import { useDocumentStore } from "@/stores/document";

export interface PreviewSyncOptions {
  /** 预览刷新的防抖延迟（毫秒） */
  debounceMs?: number;
  /** 是否启用自动刷新 */
  autoRefresh?: boolean;
}

export function usePreviewSync(options: PreviewSyncOptions = {}) {
  const { debounceMs = 1000, autoRefresh = true } = options;

  const docStore = useDocumentStore();

  // 预览 iframe 引用
  const previewIframe = ref<HTMLIFrameElement | null>(null);

  // 预览状态
  const isRefreshing = ref(false);
  const lastRefreshTime = ref<Date | null>(null);

  // 防抖定时器
  let debounceTimer: ReturnType<typeof setTimeout> | null = null;

  // 刷新预览（防抖）
  function debouncedRefresh() {
    if (debounceTimer) {
      clearTimeout(debounceTimer);
    }
    debounceTimer = setTimeout(() => {
      refreshPreview();
    }, debounceMs);
  }

  /**
   * 刷新预览
   * VitePress Dev Server 会自动监听文件变更并通过 HMR 刷新
   * 我们只需要在保存后等待 HMR 生效
   */
  function refreshPreview() {
    if (!previewIframe.value) return;

    isRefreshing.value = true;

    try {
      // 方式1：通过 postMessage 通知 iframe 刷新（如果有自定义通信）
      // previewIframe.value.contentWindow?.postMessage({ type: 'refresh' }, '*')

      // 方式2：强制重新加载 iframe（简单但会丢失滚动位置）
      // 注意：VitePress HMR 通常会自动处理，不需要手动刷新
      // 仅在需要强制刷新时使用
      // const currentSrc = previewIframe.value.src
      // previewIframe.value.src = ''
      // setTimeout(() => {
      //   if (previewIframe.value) {
      //     previewIframe.value.src = currentSrc
      //   }
      // }, 50)

      lastRefreshTime.value = new Date();
    } finally {
      // 模拟刷新完成（实际由 HMR 完成）
      setTimeout(() => {
        isRefreshing.value = false;
      }, 500);
    }
  }

  // 监听保存完成事件，触发预览刷新
  if (autoRefresh) {
    watch(
      () => docStore.isSaving,
      (saving, wasSaving) => {
        // 保存完成时刷新预览
        if (wasSaving && !saving && !docStore.isDirty) {
          debouncedRefresh();
        }
      }
    );
  }

  /**
   * 滚动同步（预留功能）
   * 将编辑器中的 Block ID 映射到预览中的对应元素并滚动
   */
  function syncScrollToBlock(blockId: string) {
    if (!previewIframe.value?.contentWindow) return;

    try {
      // 尝试通过 ID 或 data 属性查找对应元素
      const doc = previewIframe.value.contentDocument;
      if (!doc) return;

      // VitePress 会将标题生成为锚点
      // 尝试查找对应元素
      const element =
        doc.getElementById(blockId) ||
        doc.querySelector(`[data-block-id="${blockId}"]`);

      if (element) {
        element.scrollIntoView({ behavior: "smooth", block: "center" });
      }
    } catch (e) {
      // 跨域限制可能导致无法访问 contentDocument
      console.warn("[PreviewSync] Cannot access iframe content:", e);
    }
  }

  /**
   * 手动触发刷新
   */
  function forceRefresh() {
    if (!previewIframe.value) return;

    const currentSrc = previewIframe.value.src;
    if (currentSrc) {
      previewIframe.value.src = "";
      setTimeout(() => {
        if (previewIframe.value) {
          previewIframe.value.src = currentSrc;
        }
      }, 50);
    }
  }

  return {
    previewIframe,
    isRefreshing,
    lastRefreshTime,
    refreshPreview,
    syncScrollToBlock,
    forceRefresh,
  };
}
