<script setup lang="ts">
import { computed } from "vue";
import { storeToRefs } from "pinia";
import { useConfigStore } from "../../stores/config";

const configStore = useConfigStore();
const { cfg: config } = storeToRefs(configStore);

const observabilityAvailable = computed(() => {
  const cfg = config.value;
  if (!cfg) {
    return true;
  }
  const obs = cfg.observability;
  if (!obs) {
    return false;
  }
  if (!obs.enabled) {
    return false;
  }
  if (obs.uiEnabled === false) {
    return false;
  }
  return true;
});

const toolEntries = computed(() => {
  const tools = [
    {
      to: "/credentials",
      title: "凭据管理",
      description: "管理代理凭据并维护审计日志。",
      icon: "credentials",
    },
    {
      to: "/workspace",
      title: "工作区",
      description: "管理共享配置与团队任务看板。",
      icon: "workspace",
    },
    {
      to: "/test",
      title: "GitHub Actions 调试",
      description: "查看最近的 GitHub Actions 任务状态并定位问题。",
      icon: "github",
    },
    {
      to: "/git",
      title: "Git 面板",
      description: "检查仓库状态并执行常用 Git 操作。",
      icon: "git",
    },
    {
      to: "/http-tester",
      title: "HTTP 测试",
      description: "发送调试请求验证 API 和代理配置。",
      icon: "http",
    },
    {
      to: "/ip-pool",
      title: "IP 池实验室",
      description: "调试与管理代理 IP 资源。",
      icon: "ip",
    },
  ];

  if (observabilityAvailable.value) {
    tools.push({
      to: "/observability",
      title: "可观测性",
      description: "查看服务指标与报警信息。",
      icon: "observability",
    });
  }

  return tools;
});
</script>

<template>
  <section class="px-6 py-8 space-y-6">
    <header class="space-y-2">
      <h1 class="text-2xl font-semibold">开发人员调试工具</h1>
      <p class="text-sm text-base-content/70">
        在这里可以快速进入常用的开发调试页面。
      </p>
    </header>
    <div class="grid gap-4 sm:grid-cols-2 xl:grid-cols-3">
      <RouterLink
        v-for="tool in toolEntries"
        :key="tool.to"
        :to="tool.to"
        class="card border border-base-content/10 bg-base-100 transition-colors hover:border-primary/40"
      >
        <div class="card-body gap-3">
          <div class="flex items-center gap-3">
            <span class="bg-base-200 rounded-md p-2">
              <svg
                v-if="tool.icon === 'github'"
                xmlns="http://www.w3.org/2000/svg"
                width="20"
                height="20"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="2"
                stroke-linecap="round"
                stroke-linejoin="round"
              >
                <path
                  d="M9 19c-4 1.5-4-2.5-6-3m12 6v-3.87a3.37 3.37 0 0 0-.94-2.61c3.14-.35 6.44-1.54 6.44-7A5.44 5.44 0 0 0 18 4.77 5.07 5.07 0 0 0 17.91 1S16.73.65 15 2.24a13.38 13.38 0 0 0-6 0C7.27.65 6.09 1 6.09 1A5.07 5.07 0 0 0 6 4.77 5.44 5.44 0 0 0 4.5 8.5c0 5.42 3.3 6.61 6.44 7A3.37 3.37 0 0 0 10 18.13V22"
                ></path>
              </svg>
              <svg
                v-else-if="tool.icon === 'git'"
                xmlns="http://www.w3.org/2000/svg"
                width="20"
                height="20"
                viewBox="0 0 24 24"
                fill="currentColor"
              >
                <path
                  d="M23.546 10.93L13.067.452c-.604-.603-1.582-.603-2.188 0L8.708 2.627l2.76 2.76c.645-.215 1.379-.07 1.889.441.516.515.658 1.258.438 1.9l2.658 2.66c.645-.223 1.387-.078 1.9.435.721.72.721 1.884 0 2.604-.719.719-1.881.719-2.6 0-.539-.541-.674-1.337-.404-1.996L12.86 8.955v6.525c.176.086.342.203.488.348.713.721.713 1.883 0 2.6-.719.721-1.889.721-2.609 0-.719-.719-.719-1.879 0-2.598.182-.18.387-.316.605-.406V8.835c-.217-.091-.424-.222-.6-.401-.545-.545-.676-1.342-.396-2.009L7.636 3.7.45 10.881c-.6.605-.6 1.584 0 2.189l10.48 10.477c.604.604 1.582.604 2.186 0l10.43-10.43c.605-.603.605-1.582 0-2.188"
                ></path>
              </svg>
              <svg
                v-else-if="tool.icon === 'http'"
                xmlns="http://www.w3.org/2000/svg"
                width="20"
                height="20"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="2"
                stroke-linecap="round"
                stroke-linejoin="round"
              >
                <circle cx="12" cy="12" r="10"></circle>
                <line x1="2" y1="12" x2="22" y2="12"></line>
                <path
                  d="M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z"
                ></path>
              </svg>
              <svg
                v-else-if="tool.icon === 'credentials'"
                xmlns="http://www.w3.org/2000/svg"
                width="20"
                height="20"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="2"
                stroke-linecap="round"
                stroke-linejoin="round"
              >
                <rect x="3" y="11" width="18" height="11" rx="2" ry="2"></rect>
                <path d="M7 11V7a5 5 0 0 1 10 0v4"></path>
              </svg>
              <svg
                v-else-if="tool.icon === 'workspace'"
                xmlns="http://www.w3.org/2000/svg"
                width="20"
                height="20"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="2"
                stroke-linecap="round"
                stroke-linejoin="round"
              >
                <rect x="3" y="3" width="18" height="18" rx="2" ry="2"></rect>
                <path d="M3 9h18"></path>
                <path d="M9 21V9"></path>
              </svg>
              <svg
                v-else-if="tool.icon === 'ip'"
                xmlns="http://www.w3.org/2000/svg"
                width="20"
                height="20"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="2"
                stroke-linecap="round"
                stroke-linejoin="round"
              >
                <circle cx="12" cy="12" r="3"></circle>
                <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06A1.65 1.65 0 0 0 15 19.4"></path>
                <path d="M4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 9 4.6"></path>
                <path d="M15 4.6a1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9"></path>
                <path d="M4.6 15a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 0 2.83 2.83l.06-.06A1.65 1.65 0 0 0 9 19.4"></path>
              </svg>
              <svg
                v-else-if="tool.icon === 'observability'"
                xmlns="http://www.w3.org/2000/svg"
                width="20"
                height="20"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="2"
                stroke-linecap="round"
                stroke-linejoin="round"
              >
                <path d="M4 19V9"></path>
                <path d="M8 19V5"></path>
                <path d="M12 19v-7"></path>
                <path d="M16 19v-3"></path>
                <path d="M20 19V8"></path>
              </svg>
            </span>
            <h2 class="card-title text-base">{{ tool.title }}</h2>
          </div>
          <p class="text-sm text-base-content/70">{{ tool.description }}</p>
        </div>
      </RouterLink>
    </div>
  </section>
</template>
