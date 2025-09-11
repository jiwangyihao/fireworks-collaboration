import { createApp } from "vue";
import { createPinia } from "pinia";
import App from "./App.vue";
import router from "./router";
import { useConfigStore } from "./stores/config";

const app = createApp(App);
const pinia = createPinia();
app.use(pinia);
app.use(router);

// 预加载配置（非阻塞）
const cfgStore = useConfigStore();
cfgStore.refresh().catch(() => {
  /* ignore at boot */
});

app.mount("#app");
