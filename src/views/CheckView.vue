<script setup lang="ts">
import { onMounted, ref } from "vue";
import {
  exchangeCodeForToken,
  getUserInfo,
  loadAccessToken,
  removeAccessToken,
  saveAccessToken,
  startOAuthFlow,
  validateToken,
} from "../utils/github-auth";
import { createCallbackServer } from "../utils/oauth-server";
import {
  checkGit,
  checkNode,
  checkPnpm,
  Status,
} from "../utils/environ-check.ts";
import { useRouter } from "vue-router";

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
const authenticated = ref(false);

async function checkGitHubAuthentication() {
  const existingToken = await loadAccessToken();
  if (existingToken) {
    const isValid = await validateToken(existingToken);
    if (isValid) {
      const userInfo = await getUserInfo(existingToken);
      return {
        authenticated: true,
        user: userInfo,
      };
    } else {
      await removeAccessToken();
      return {
        authenticated: false,
        user: null,
      };
    }
  } else {
    return {
      authenticated: false,
      user: null,
    };
  }
}

async function authenticateGitHub() {
  loginLabel.value = "请在浏览器中完成登录";

  // 启动 OAuth 流程
  // 创建本地回调服务器
  const { server, getCallbackData } = await createCallbackServer();

  // 开始 OAuth 流程
  const { codeVerifier, state } = await startOAuthFlow();

  // 等待回调
  const callbackData = await getCallbackData();

  // 关闭服务器
  server.close();

  if (callbackData.error) {
    throw new Error(
      `GitHub 授权失败: ${callbackData.error_description || callbackData.error}`,
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

  // 使用授权码换取访问令牌
  const accessToken = await exchangeCodeForToken(
    callbackData.code,
    codeVerifier,
  );

  // 保存访问令牌
  await saveAccessToken(accessToken);

  // 获取用户信息
  const user = await getUserInfo(accessToken);
  authenticated.value = true;
  if (authenticated.value && user) {
    loginLabel.value = `你好，@${user.name || user.login}`;
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

  loginLoading.value = true;
  loginLabel.value = "正在检查登录状态...";
  const { authenticated: auth, user } = await checkGitHubAuthentication();
  authenticated.value = auth;
  if (auth && user) {
    loginLabel.value = `你好，@${user.name || user.login}`;
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
  <main
    class="w-full min-w-full h-full prose flex flex-col items-center justify-center"
  >
    <h1>薪火笔记社 · 文档贡献工具</h1>

    <TransitionGroup name="list" tag="ul" class="max-w-1/2">
      <li
        v-for="status in statusList"
        class="flex items-center gap-2 font-bold my-2!"
        :key="status.id"
      >
        <span class="inline-grid *:[grid-area:1/1]">
          <span
            v-if="status.type !== 'success'"
            class="status animate-ping"
            :class="`status-${status.type}`"
          ></span>
          <span class="status" :class="`status-${status.type}`"></span>
        </span>
        {{ status.message }}
      </li>
    </TransitionGroup>
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
            await router.push('/test');
          }
        }
      "
    >
      <span v-if="loginLoading" class="loading loading-spinner"></span>
      <svg
        v-else
        aria-label="GitHub logo"
        width="16"
        height="16"
        xmlns="http://www.w3.org/2000/svg"
        viewBox="0 0 24 24"
      >
        <path
          fill="currentColor"
          d="M12,2A10,10 0 0,0 2,12C2,16.42 4.87,20.17 8.84,21.5C9.34,21.58 9.5,21.27 9.5,21C9.5,20.77 9.5,20.14 9.5,19.31C6.73,19.91 6.14,17.97 6.14,17.97C5.68,16.81 5.03,16.5 5.03,16.5C4.12,15.88 5.1,15.9 5.1,15.9C6.1,15.97 6.63,16.93 6.63,16.93C7.5,18.45 8.97,18 9.54,17.76C9.63,17.11 9.89,16.67 10.17,16.42C7.95,16.17 5.62,15.31 5.62,11.5C5.62,10.39 6,9.5 6.65,8.79C6.55,8.54 6.2,7.5 6.75,6.15C6.75,6.15 7.59,5.88 9.5,7.17C10.29,6.95 11.15,6.84 12,6.84C12.85,6.84 13.71,6.95 14.5,7.17C16.41,5.88 17.25,6.15 17.25,6.15C17.8,7.5 17.45,8.54 17.35,8.79C18,9.5 18.38,10.39 18.38,11.5C18.38,15.32 16.04,16.16 13.81,16.41C14.17,16.72 14.5,17.33 14.5,18.26C14.5,19.6 14.5,20.68 14.5,21C14.5,21.27 14.66,21.59 15.17,21.5C19.14,20.16 22,16.42 22,12A10,10 0 0,0 12,2Z"
        ></path>
      </svg>
      {{ loginLabel }}
    </button>
  </main>
</template>
