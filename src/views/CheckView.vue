<script setup lang="ts">
import { inject, onMounted, Ref, ref } from "vue";
import {
  exchangeCodeForToken,
  getUserInfo,
  loadAccessToken,
  removeAccessToken,
  saveAccessToken,
  startOAuthFlow,
  UserInfo,
  validateToken,
  syncCredentialToBackend,
} from "../utils/github-auth";
import { createCallbackServer } from "../utils/oauth-server";
import {
  checkGit,
  checkNode,
  checkPnpm,
  Status,
} from "../utils/environ-check.ts";
import { useRouter } from "vue-router";
import { startIpPoolPreheater } from "../api/ip-pool";
import { waitForIpPoolWarmup } from "../utils/check-preheat";

// 导入可复用组件
import StatusList from "../components/StatusList.vue";
import BaseIcon from "../components/BaseIcon.vue";

const router = useRouter();

const statusList = ref<Status[]>([
  {
    id: 1,
    type: "warning",
    message: "正在准备环境检查",
  },
]);

const loginLoading = ref(false);
const loginLabel = ref("从 GitHub 登录");
const authenticated = inject<Ref<boolean>>("authenticated", ref(false));
const user = inject<Ref<UserInfo | null>>("user", ref(null));

async function checkGitHubAuthentication() {
  const existingToken = await loadAccessToken();
  if (existingToken) {
    const isValid = await validateToken(existingToken);
    if (isValid) {
      // 同步凭据到后端
      await syncCredentialToBackend(existingToken);
      user.value = await getUserInfo(existingToken);
      authenticated.value = true;
      return;
    } else {
      await removeAccessToken();
    }
  }
  authenticated.value = false;
  user.value = null;
}

async function authenticateGitHub() {
  loginLabel.value = "请在浏览器中完成登录";

  // 启动 OAuth 流程
  // 创建本地回调服务器（返回动态分配的端口）
  const { server, port, getCallbackData } = await createCallbackServer();

  // 开始 OAuth 流程，传入动态端口
  const { codeVerifier, state } = await startOAuthFlow(port);

  // 等待回调
  const callbackData = await getCallbackData();

  // 关闭服务器
  server.close();

  if (callbackData.error) {
    throw new Error(
      `GitHub 授权失败: ${callbackData.error_description || callbackData.error}`
    );
  }

  if (!callbackData.code) {
    throw new Error("未收到有效的授权码");
  }

  // 如果有 state 参数，验证它是否匹配；如果没有，记录警告但继续
  if (callbackData.state) {
    if (callbackData.state !== state) {
      throw new Error("状态参数不匹配，可能存在安全风险");
    }
  } else {
    console.warn("GitHub 回调中缺少 state 参数，这可能是一个安全问题");
    // 暂时允许继续，但记录警告
  }

  loginLabel.value = "正在交换访问令牌...";

  // 使用授权码换取访问令牌，传入动态端口
  const accessToken = await exchangeCodeForToken(
    callbackData.code,
    codeVerifier,
    port
  );

  // 保存访问令牌
  await saveAccessToken(accessToken);

  // 获取用户信息
  user.value = await getUserInfo(accessToken);
  authenticated.value = true;
  if (authenticated.value && user.value) {
    loginLabel.value = `你好，@${user.value.name || user.value.login}`;
  } else {
    loginLabel.value = "从 GitHub 登录";
  }
}

async function updateStatus(generator: AsyncGenerator<Partial<Status>>) {
  for await (let status of generator) {
    statusList.value[statusList.value.length - 1] = {
      ...statusList.value[statusList.value.length - 1],
      ...status,
    };
  }
}

onMounted(async () => {
  statusList.value[statusList.value.length - 1].message = "正在检查 Git 版本";
  await updateStatus(checkGit());

  statusList.value.push({
    id: statusList.value.length + 1,
    type: "warning",
    message: "正在检查 Node.js 版本",
  });
  await updateStatus(checkNode());

  statusList.value.push({
    id: statusList.value.length + 1,
    type: "warning",
    message: "正在检查 pnpm 版本",
  });
  await updateStatus(checkPnpm());

  statusList.value.push({
    id: statusList.value.length + 1,
    type: "warning",
    message: "正在启动代理 IP 池预热",
  });

  try {
    const activation = await startIpPoolPreheater();
    const idx = statusList.value.length - 1;

    if (!activation.enabled) {
      statusList.value[idx] = {
        ...statusList.value[idx],
        type: "success",
        message: "IP 池功能未启用，已跳过预热",
      };
    } else if (activation.preheatTargets === 0) {
      statusList.value[idx] = {
        ...statusList.value[idx],
        type: "success",
        message: "未配置预热域名，跳过 IP 池预热",
      };
    } else if (!activation.activationChanged && activation.preheaterActive) {
      statusList.value[idx] = {
        ...statusList.value[idx],
        type: "success",
        message: "IP 池预热已在后台运行",
      };
    } else {
      statusList.value[idx] = {
        ...statusList.value[idx],
        message: "正在预热域名解析 IP 池，等待加载候选 DNS...",
      };

      const warmup = await waitForIpPoolWarmup(activation.preheatTargets);
      if (warmup.state === "ready") {
        const { completedTargets, totalTargets } = warmup;
        const summary =
          totalTargets > 0
            ? `已覆盖 ${completedTargets}/${totalTargets} 个预热目标`
            : "无需加载预热目标";
        statusList.value[idx] = {
          ...statusList.value[idx],
          type: "success",
          message: `IP 池预热完成，${summary}`,
        };
      } else if (warmup.state === "disabled") {
        statusList.value[idx] = {
          ...statusList.value[idx],
          type: "success",
          message: "IP 池已禁用，跳过预热",
        };
      } else if (warmup.state === "inactive") {
        const { completedTargets, totalTargets } = warmup;
        const summary =
          totalTargets > 0
            ? `当前已加载 ${completedTargets}/${totalTargets} 个预热目标`
            : "当前无预热目标";
        statusList.value[idx] = {
          ...statusList.value[idx],
          type: "success",
          message: `IP 池预热未启动，${summary}`,
        };
      } else {
        const { completedTargets, totalTargets } = warmup;
        statusList.value[idx] = {
          ...statusList.value[idx],
          type: "success",
          message: `IP 池预热正在后台继续（已加载 ${completedTargets}/${totalTargets} 个预热目标）`,
        };
      }
    }
  } catch (error) {
    const idx = statusList.value.length - 1;
    statusList.value[idx] = {
      ...statusList.value[idx],
      type: "error",
      message: `IP 池预热失败：${String(error)}`,
    };
  }

  loginLoading.value = true;
  loginLabel.value = "正在检查登录状态...";
  await checkGitHubAuthentication();
  if (authenticated.value && user.value) {
    loginLabel.value = `你好，@${user.value.name || user.value.login}`;
  } else {
    loginLabel.value = "从 GitHub 登录";
  }
  loginLoading.value = false;

  // // 系统代理检查
  // statusList.value.push({
  //   id: statusList.value.length + 1,
  //   type: "warning",
  //   message: "正在检查系统代理设置",
  // });
  //
  // try {
  //   const proxyInfo = await getSystemProxy();
  //   const proxyDesc = getProxyTypeDescription(proxyInfo);
  //   const proxyAddr = formatProxyAddress(proxyInfo);
  //
  //   statusList.value[statusList.value.length - 1] = {
  //     id: statusList.value.length,
  //     type: "success",
  //     message: `系统代理检查完成：${proxyDesc}${proxyInfo.enabled ? ` (${proxyAddr})` : ""}`,
  //   };
  // } catch (error) {
  //   statusList.value[statusList.value.length - 1] = {
  //     id: statusList.value.length,
  //     type: "error",
  //     message: `系统代理检查失败：${error}`,
  //   };
  // }
  //
  // // GitHub 身份验证
  // statusList.value.push({
  //   id: statusList.value.length + 1,
  //   type: "warning",
  //   message: "正在检查 GitHub 身份验证状态",
  // });
  //
  // const authSuccess = await authenticateGitHub();
  // if (!authSuccess) {
  //   return;
  // }
  //
  // statusList.value.push({
  //   id: statusList.value.length + 1,
  //   type: "warning",
  //   message: "正在检查仓库目录",
  // });
  // if (!(await exists("", { baseDir: BaseDirectory.AppData }))) {
  //   await mkdir("", { baseDir: BaseDirectory.AppData });
  // }
  //
  // if (await exists("repository", { baseDir: BaseDirectory.AppData })) {
  //   statusList.value[statusList.value.length - 1].message = "正在检查仓库状态";
  //   try {
  //     let result = await Command.create("cmd", ["/c", "git", "status"], {
  //       cwd: (await appDataDir()).concat("repository"),
  //     }).execute();
  //     console.log(result);
  //   } catch (e) {
  //     //
  //   }
  // } else {
  //   statusList.value[statusList.value.length - 1].message =
  //     "仓库目录不存在，正在克隆仓库";
  //   try {
  //     // 使用认证的 GitHub token 克隆私有仓库
  //     // let result = await Command.create(
  //     //   "cmd",
  //     //   [
  //     //     "/c",
  //     //     "git",
  //     //     "clone",
  //     //     "https://github.com/HIT-Fireworks/fireworks-notes-society.git",
  //     //     "repository",
  //     //   ],
  //     //   {
  //     //     cwd: await appDataDir(),
  //     //   },
  //     // ).execute();
  //     // console.log(result);
  //   } catch (e) {
  //     console.error(e);
  //   }
  // }
});
</script>

<template>
  <main class="page items-center justify-center">
    <h1>薪火笔记社 · 文档贡献工具</h1>

    <StatusList :items="statusList" />
    <button
      class="btn btn-primary"
      :disabled="
        loginLoading ||
        !(
          statusList.length >= 3 &&
          statusList.every((status) => status.type === 'success')
        )
      "
      @click="
        async () => {
          if (!loginLoading) {
            loginLoading = true;
            if (!authenticated) {
              await authenticateGitHub();
            }
            loginLoading = false;
            await router.push('/project');
          }
        }
      "
    >
      <span v-if="loginLoading" class="loading loading-spinner"></span>
      <BaseIcon v-else icon="simple-icons--github" size="sm" />
      {{ loginLabel }}
    </button>
  </main>
</template>
