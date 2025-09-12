import { createApp } from "vue";
import { createPinia } from "pinia";
import App from "./App.vue";
import router from "./router";
import { useConfigStore } from "./stores/config";
import { initTaskEvents } from "./api/tasks";
// + dev 调试句柄（方案 B）
import { invoke } from "@tauri-apps/api/core";
import { listen, emit } from "@tauri-apps/api/event";

const app = createApp(App);
const pinia = createPinia();
app.use(pinia);
app.use(router);

// 预加载配置（非阻塞）
const cfgStore = useConfigStore();
cfgStore.refresh().catch(() => {
  /* ignore at boot */
});

// 初始化任务事件监听（忽略失败）
initTaskEvents().catch(() => {});

// 仅开发模式暴露调试对象，方便在浏览器控制台直接使用，而无需裸模块解析
if (import.meta.env.DEV) {
  // 不放进生产，避免暴露内部接口
  (window as any).__fw_debug = { invoke, listen, emit };
  // 可选提示（不影响功能）
  // eslint-disable-next-line no-console
  console.info("[dev] __fw_debug 已注入: { invoke, listen, emit }");
}

app.mount("#app");
