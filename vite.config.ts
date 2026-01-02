import { defineConfig } from "vite";
import tailwindcss from "@tailwindcss/vite";
import path from "path";
import veauryVitePlugins from "veaury/vite/index.js";

const host = process.env.TAURI_DEV_HOST;

// https://vitejs.dev/config/
export default defineConfig(() => {
  return {
    plugins: [
      // veaury 集成：Vue 为主框架，React 用于编辑器组件
      // 它内部会自动管理 @vitejs/plugin-vue, @vitejs/plugin-react 和 @vitejs/plugin-vue-jsx
      veauryVitePlugins({
        type: "vue",
      }),
      tailwindcss(),
    ],

    // 路径别名
    resolve: {
      alias: {
        "@": path.resolve(__dirname, "./src"),
      },
    },

    // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
    //
    // 1. prevent vite from obscuring rust errors
    clearScreen: false,
    // 2. tauri expects a fixed port, fail if that port is not available
    server: {
      port: 1420,
      strictPort: true,
      host: host || false,
      hmr: host
        ? {
            protocol: "ws",
            host,
            port: 1421,
          }
        : undefined,
      watch: {
        // 3. tell vite to ignore watching `src-tauri` and app config dir
        ignored: ["**/src-tauri/**", "**/config/**"],
      },
    },
    test: {
      environment: "happy-dom",
      setupFiles: ["./vitest.setup.ts"],
      coverage: {
        provider: "v8",
        reporter: ["text", "html"],
      },
    },
  };
});
