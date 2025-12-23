<template>
  <div class="proxy-config">
    <h3>代理配置</h3>

    <div class="form-group">
      <label for="proxy-mode">代理模式</label>
      <select id="proxy-mode" v-model="localConfig.mode" @change="onModeChange">
        <option value="off">关闭 (Off)</option>
        <option value="http">HTTP/HTTPS</option>
        <option value="socks5">SOCKS5</option>
        <option value="system">系统代理 (System)</option>
      </select>
    </div>

    <div v-if="showProxySettings" class="form-group">
      <label for="proxy-url">代理服务器地址</label>
      <input
        id="proxy-url"
        v-model="localConfig.url"
        type="text"
        placeholder="例如: http://proxy.example.com:8080"
        :disabled="localConfig.mode === 'system'"
      />
    </div>

    <div v-if="showProxySettings" class="form-group">
      <label for="proxy-username">用户名 (可选)</label>
      <input
        id="proxy-username"
        v-model="localConfig.username"
        type="text"
        placeholder="代理认证用户名"
      />
    </div>

    <div v-if="showProxySettings" class="form-group">
      <label for="proxy-password">密码 (可选)</label>
      <input
        id="proxy-password"
        v-model="localConfig.password"
        type="password"
        placeholder="代理认证密码"
      />
    </div>

    <!-- System Proxy Detection -->
    <div v-if="localConfig.mode === 'system'" class="system-proxy-detection">
      <h4>系统代理检测</h4>
      <button @click="detectSystemProxy" :disabled="detecting">
        {{ detecting ? "检测中..." : "检测系统代理" }}
      </button>

      <div v-if="detectionResult" class="detection-result">
        <p v-if="detectionResult.url">
          <strong>检测到代理:</strong> {{ detectionResult.url }}
          <br />
          <strong>类型:</strong> {{ detectionResult.type || "未知" }}
        </p>
        <p v-else class="no-proxy">未检测到系统代理</p>

        <button
          v-if="detectionResult.url"
          @click="applySystemProxy"
          class="apply-btn"
        >
          一键应用
        </button>
      </div>
    </div>

    <!-- Advanced Settings -->
    <div class="form-group">
      <label>
        <input
          v-model="localConfig.disableCustomTransport"
          type="checkbox"
          :disabled="isProxyEnabled"
        />
        禁用自定义传输层
        <span class="hint" v-if="isProxyEnabled"> (代理启用时自动禁用) </span>
      </label>
      <p class="description">
        禁用后将使用 libgit2 默认传输，同时禁用 Fake SNI 和 IP 优选功能
      </p>
    </div>

    <div class="form-group">
      <label>
        <input v-model="localConfig.debugProxyLogging" type="checkbox" />
        启用代理调试日志
      </label>
      <p class="description">
        输出详细的代理连接信息（URL已脱敏、认证状态、耗时等）
      </p>
    </div>

    <div class="form-actions">
      <button @click="saveConfig" class="primary-btn">保存配置</button>
      <button @click="resetConfig" class="secondary-btn">重置</button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { useConfigStore } from "../../stores/config";

interface ProxyConfig {
  mode: "off" | "http" | "socks5" | "system";
  url: string;
  username?: string;
  password?: string;
  disableCustomTransport: boolean;
  debugProxyLogging: boolean;
}

interface SystemProxyResult {
  url?: string;
  type?: string;
}

const configStore = useConfigStore();

const localConfig = ref<ProxyConfig>({
  mode: "off",
  url: "",
  username: undefined,
  password: undefined,
  disableCustomTransport: false,
  debugProxyLogging: false,
});

const detecting = ref(false);
const detectionResult = ref<SystemProxyResult | null>(null);

const showProxySettings = computed(() => {
  return localConfig.value.mode !== "off";
});

const isProxyEnabled = computed(() => {
  return (
    localConfig.value.mode !== "off" && localConfig.value.url.trim() !== ""
  );
});

// Load config from store
watch(
  () => configStore.cfg?.proxy,
  (proxyConfig) => {
    if (proxyConfig) {
      localConfig.value = {
        mode: proxyConfig.mode || "off",
        url: proxyConfig.url || "",
        username: proxyConfig.username,
        password: proxyConfig.password,
        disableCustomTransport: proxyConfig.disableCustomTransport || false,
        debugProxyLogging: proxyConfig.debugProxyLogging || false,
      };
    }
  },
  { immediate: true, deep: true }
);

// Auto-disable custom transport when proxy is enabled
watch(isProxyEnabled, (enabled) => {
  if (enabled) {
    localConfig.value.disableCustomTransport = true;
  }
});

const onModeChange = () => {
  if (localConfig.value.mode === "off") {
    detectionResult.value = null;
  }
};

const detectSystemProxy = async () => {
  detecting.value = true;
  detectionResult.value = null;

  try {
    const result = await invoke<SystemProxyResult>("detect_system_proxy");
    detectionResult.value = result;
  } catch (error) {
    console.error("Failed to detect system proxy:", error);
    alert(`检测失败: ${error}`);
  } finally {
    detecting.value = false;
  }
};

const applySystemProxy = () => {
  if (detectionResult.value?.url) {
    localConfig.value.url = detectionResult.value.url;

    // Update mode based on detected type
    if (detectionResult.value.type === "socks5") {
      localConfig.value.mode = "socks5";
    } else {
      localConfig.value.mode = "http";
    }
  }
};

const saveConfig = async () => {
  try {
    if (!configStore.cfg) {
      throw new Error("配置未加载");
    }

    const updatedConfig = {
      ...configStore.cfg,
      proxy: {
        ...configStore.cfg.proxy,
        ...localConfig.value,
      },
    };

    await configStore.save(updatedConfig);
    alert("代理配置已保存");
  } catch (error) {
    console.error("Failed to save proxy config:", error);
    alert(`保存失败: ${error}`);
  }
};

const resetConfig = () => {
  localConfig.value = {
    mode: "off",
    url: "",
    username: undefined,
    password: undefined,
    disableCustomTransport: false,
    debugProxyLogging: false,
  };
  detectionResult.value = null;
};
</script>

<style scoped>
.proxy-config {
  padding: 20px;
  background: #f5f5f5;
  border-radius: 8px;
}

.form-group {
  margin-bottom: 15px;
}

.form-group label {
  display: block;
  margin-bottom: 5px;
  font-weight: 500;
}

.form-group input[type="text"],
.form-group input[type="password"],
.form-group select {
  width: 100%;
  padding: 8px;
  border: 1px solid #ddd;
  border-radius: 4px;
}

.form-group input:disabled {
  background: #e9e9e9;
  cursor: not-allowed;
}

.description {
  margin: 5px 0 0;
  font-size: 12px;
  color: #666;
}

.hint {
  font-size: 12px;
  color: #999;
  font-weight: normal;
}

.system-proxy-detection {
  margin: 20px 0;
  padding: 15px;
  background: #fff;
  border: 1px solid #ddd;
  border-radius: 4px;
}

.system-proxy-detection h4 {
  margin-top: 0;
}

.detection-result {
  margin-top: 10px;
  padding: 10px;
  background: #f9f9f9;
  border-radius: 4px;
}

.detection-result .no-proxy {
  color: #999;
  font-style: italic;
}

.apply-btn {
  margin-top: 10px;
  padding: 6px 12px;
  background: #4caf50;
  color: white;
  border: none;
  border-radius: 4px;
  cursor: pointer;
}

.apply-btn:hover {
  background: #45a049;
}

.form-actions {
  display: flex;
  gap: 10px;
  margin-top: 20px;
}

.primary-btn {
  padding: 10px 20px;
  background: #2196f3;
  color: white;
  border: none;
  border-radius: 4px;
  cursor: pointer;
}

.primary-btn:hover {
  background: #0b7dda;
}

.secondary-btn {
  padding: 10px 20px;
  background: #fff;
  color: #333;
  border: 1px solid #ddd;
  border-radius: 4px;
  cursor: pointer;
}

.secondary-btn:hover {
  background: #f5f5f5;
}

button:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}
</style>
