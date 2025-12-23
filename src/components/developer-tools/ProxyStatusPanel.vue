<template>
  <div class="proxy-status-panel">
    <h3>代理状态</h3>

    <div class="status-grid">
      <div class="status-item">
        <span class="label">当前模式:</span>
        <span class="value" :class="`mode-${proxyMode}`">
          {{ proxyModeText }}
        </span>
      </div>

      <div class="status-item">
        <span class="label">运行状态:</span>
        <span class="value" :class="`state-${proxyState}`">
          <span class="status-indicator" :class="`state-${proxyState}`"></span>
          {{ proxyStateText }}
        </span>
      </div>

      <div class="status-item" v-if="proxyUrl">
        <span class="label">代理服务器:</span>
        <span class="value">{{ proxyUrl }}</span>
      </div>

      <div class="status-item">
        <span class="label">自定义传输层:</span>
        <span
          class="value"
          :class="customTransportDisabled ? 'disabled' : 'enabled'"
        >
          {{ customTransportDisabled ? "已禁用" : "已启用" }}
        </span>
      </div>
    </div>

    <!-- Fallback Info -->
    <div v-if="proxyState === 'fallback'" class="fallback-info alert-warning">
      <strong>降级信息:</strong>
      <p>{{ fallbackReason || "代理连接失败，已自动切换到直连模式" }}</p>
      <p v-if="failureCount"><strong>失败次数:</strong> {{ failureCount }}</p>
    </div>

    <!-- Recovering Info -->
    <div v-if="proxyState === 'recovering'" class="recovering-info alert-info">
      <strong>恢复中...</strong>
      <p>正在测试代理可用性</p>
      <p v-if="nextHealthCheckIn">
        <strong>下次健康检查:</strong> {{ nextHealthCheckIn }}秒后
      </p>
    </div>

    <!-- Health Check Stats -->
    <div v-if="healthCheckSuccessRate !== null" class="health-check-stats">
      <span class="label">健康检查成功率:</span>
      <div class="progress-bar">
        <div
          class="progress-fill"
          :style="{ width: `${(healthCheckSuccessRate || 0) * 100}%` }"
          :class="getHealthCheckClass(healthCheckSuccessRate)"
        ></div>
      </div>
      <span class="value"
        >{{ ((healthCheckSuccessRate || 0) * 100).toFixed(1) }}%</span
      >
    </div>

    <!-- Manual Control -->
    <div class="manual-control">
      <button
        v-if="proxyState === 'enabled'"
        @click="forceFallback"
        class="control-btn fallback-btn"
        :disabled="controlling"
      >
        强制降级
      </button>

      <button
        v-if="proxyState === 'fallback' || proxyState === 'recovering'"
        @click="forceRecovery"
        class="control-btn recovery-btn"
        :disabled="controlling"
      >
        强制恢复
      </button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useConfigStore } from "../../stores/config";

const configStore = useConfigStore();
const controlling = ref(false);

// State data populated from proxy events
const proxyMode = computed(() => configStore.cfg?.proxy?.mode || "off");
const proxyState = ref<"enabled" | "disabled" | "fallback" | "recovering">(
  "disabled"
);
const proxyUrl = computed(() => {
  const url = configStore.cfg?.proxy?.url;
  if (!url) return null;
  // Sanitize URL to hide credentials
  try {
    const urlObj = new URL(url);
    return `${urlObj.protocol}//${urlObj.host}`;
  } catch {
    return url;
  }
});
const customTransportDisabled = computed(
  () => configStore.cfg?.proxy?.disableCustomTransport || false
);
const fallbackReason = ref<string | null>(null);
const failureCount = ref<number | null>(null);
const healthCheckSuccessRate = ref<number | null>(null);
const nextHealthCheckIn = ref<number | null>(null);

const proxyModeText = computed(() => {
  const modes = {
    off: "关闭",
    http: "HTTP/HTTPS",
    socks5: "SOCKS5",
    system: "系统代理",
  };
  return modes[proxyMode.value as keyof typeof modes] || "未知";
});

const proxyStateText = computed(() => {
  const states = {
    enabled: "已启用",
    disabled: "已禁用",
    fallback: "已降级",
    recovering: "恢复中",
  };
  return states[proxyState.value] || "未知";
});

const getHealthCheckClass = (rate: number | null) => {
  if (rate === null) return "";
  if (rate >= 0.8) return "success";
  if (rate >= 0.5) return "warning";
  return "error";
};

const forceFallback = async () => {
  controlling.value = true;
  try {
    await invoke("force_proxy_fallback", {
      reason: "用户手动触发降级",
    });
    proxyState.value = "fallback";
    fallbackReason.value = "用户手动触发降级";
  } catch (error) {
    console.error("Failed to force fallback:", error);
    alert(`降级失败: ${error}`);
  } finally {
    controlling.value = false;
  }
};

const forceRecovery = async () => {
  controlling.value = true;
  try {
    await invoke("force_proxy_recovery");
    proxyState.value = "enabled";
    fallbackReason.value = null;
    failureCount.value = null;
  } catch (error) {
    console.error("Failed to force recovery:", error);
    alert(`恢复失败: ${error}`);
  } finally {
    controlling.value = false;
  }
};

// Update state based on proxy mode
const updateStateFromConfig = () => {
  if (proxyMode.value === "off") {
    proxyState.value = "disabled";
  } else if (configStore.cfg?.proxy?.url) {
    proxyState.value = "enabled";
  }
};

// Watch for config changes
configStore.$subscribe(() => {
  updateStateFromConfig();
});

updateStateFromConfig();

// Event listener handles
let unlistenProxyState: UnlistenFn | null = null;

// Proxy event payload interface
interface ProxyStateEvent {
  proxy_mode?: string;
  proxy_state?: string;
  fallback_reason?: string;
  failure_count?: number;
  health_check_success_rate?: number;
  next_health_check_in?: number;
  custom_transport_disabled?: boolean;
}

// Setup event listeners
onMounted(async () => {
  try {
    // Listen for proxy state events
    unlistenProxyState = await listen<ProxyStateEvent>(
      "proxy://state",
      (event) => {
        const payload = event.payload;
        console.log("Received proxy state event:", payload);

        // Update local state from event
        if (payload.proxy_state) {
          const stateMap: Record<string, typeof proxyState.value> = {
            Enabled: "enabled",
            Disabled: "disabled",
            Fallback: "fallback",
            Recovering: "recovering",
          };
          proxyState.value = stateMap[payload.proxy_state] || "disabled";
        }

        if (payload.fallback_reason !== undefined) {
          fallbackReason.value = payload.fallback_reason;
        }

        if (payload.failure_count !== undefined) {
          failureCount.value = payload.failure_count;
        }

        if (payload.health_check_success_rate !== undefined) {
          healthCheckSuccessRate.value = payload.health_check_success_rate;
        }

        if (payload.next_health_check_in !== undefined) {
          nextHealthCheckIn.value = payload.next_health_check_in;
        }
      }
    );
  } catch (error) {
    console.error("Failed to setup proxy event listeners:", error);
  }
});

// Cleanup event listeners
onUnmounted(() => {
  if (unlistenProxyState) {
    unlistenProxyState();
  }
});
</script>

<style scoped>
.proxy-status-panel {
  padding: 20px;
  background: #fff;
  border: 1px solid #ddd;
  border-radius: 8px;
}

.status-grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
  gap: 15px;
  margin-bottom: 20px;
}

.status-item {
  display: flex;
  flex-direction: column;
  gap: 5px;
}

.status-item .label {
  font-size: 12px;
  color: #666;
  font-weight: 500;
}

.status-item .value {
  font-size: 14px;
  font-weight: 600;
}

.status-indicator {
  display: inline-block;
  width: 8px;
  height: 8px;
  border-radius: 50%;
  margin-right: 5px;
}

.status-indicator.state-enabled {
  background: #4caf50;
}

.status-indicator.state-disabled {
  background: #999;
}

.status-indicator.state-fallback {
  background: #ff9800;
}

.status-indicator.state-recovering {
  background: #2196f3;
}

.mode-off {
  color: #999;
}

.mode-http,
.mode-socks5,
.mode-system {
  color: #2196f3;
}

.state-enabled {
  color: #4caf50;
}

.state-disabled {
  color: #999;
}

.state-fallback {
  color: #ff9800;
}

.state-recovering {
  color: #2196f3;
}

.disabled {
  color: #999;
}

.enabled {
  color: #4caf50;
}

.fallback-info,
.recovering-info {
  padding: 15px;
  margin: 15px 0;
  border-radius: 4px;
}

.alert-warning {
  background: #fff3cd;
  border: 1px solid #ffc107;
  color: #856404;
}

.alert-info {
  background: #d1ecf1;
  border: 1px solid #17a2b8;
  color: #0c5460;
}

.fallback-info strong,
.recovering-info strong {
  display: block;
  margin-bottom: 5px;
}

.fallback-info p,
.recovering-info p {
  margin: 5px 0;
}

.health-check-stats {
  display: flex;
  align-items: center;
  gap: 10px;
  margin: 15px 0;
}

.progress-bar {
  flex: 1;
  height: 20px;
  background: #e0e0e0;
  border-radius: 10px;
  overflow: hidden;
}

.progress-fill {
  height: 100%;
  transition: width 0.3s ease;
}

.progress-fill.success {
  background: #4caf50;
}

.progress-fill.warning {
  background: #ff9800;
}

.progress-fill.error {
  background: #f44336;
}

.manual-control {
  display: flex;
  gap: 10px;
  margin-top: 20px;
  padding-top: 20px;
  border-top: 1px solid #eee;
}

.control-btn {
  padding: 8px 16px;
  border: none;
  border-radius: 4px;
  cursor: pointer;
  font-weight: 500;
  transition: all 0.2s;
}

.fallback-btn {
  background: #ff9800;
  color: white;
}

.fallback-btn:hover:not(:disabled) {
  background: #f57c00;
}

.recovery-btn {
  background: #4caf50;
  color: white;
}

.recovery-btn:hover:not(:disabled) {
  background: #45a049;
}

.control-btn:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}
</style>
