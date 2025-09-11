import { describe, it, expect, vi, beforeEach } from "vitest";
import { mount, flushPromises } from "@vue/test-utils";
import { createMemoryHistory, createRouter } from "vue-router";
import { ref } from "vue";

// 模拟 environ-check，返回一次成功项的异步生成器
vi.mock("../../utils/environ-check.ts", () => {
  function makeSuccessGen(label: string) {
    return (async function* () {
      yield { type: "success", message: `${label} OK` } as any;
    })();
  }
  return {
    checkGit: () => makeSuccessGen("Git"),
    checkNode: () => makeSuccessGen("Node"),
    checkPnpm: () => makeSuccessGen("pnpm"),
  };
});

// 模拟 GitHub 认证相关（此测试不触发 OAuth，保持最小化）
vi.mock("../../utils/github-auth", () => ({
  loadAccessToken: vi.fn().mockResolvedValue("tok"),
  validateToken: vi.fn().mockResolvedValue(true),
  getUserInfo: vi.fn().mockResolvedValue({ login: "u", name: "U" }),
  removeAccessToken: vi.fn(),
  startOAuthFlow: vi.fn().mockResolvedValue({ codeVerifier: "v", state: "s" }),
}));
vi.mock("../../utils/oauth-server", () => ({
  createCallbackServer: vi.fn().mockResolvedValue({
    server: { close: vi.fn() },
    getCallbackData: vi.fn(),
  }),
}));

import CheckView from "../CheckView.vue";

function makeRouter() {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: [
      { path: "/", name: "root", component: { template: "<div/>" } },
      { path: "/project", name: "project", component: { template: "<div/>" } },
    ],
  });
  return router;
}

describe("views/CheckView", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("环境检查成功后按钮可用，点击后跳转到 /project（已认证不触发 OAuth）", async () => {
    const router = makeRouter();
    await router.push("/");
    await router.isReady();

    const authenticated = ref(false); // 初始无所谓，onMounted 会设置为 true
    const user = ref(null);

    const wrapper = mount(CheckView, {
      global: {
        plugins: [router],
        provide: { authenticated, user },
        stubs: { TransitionGroup: false },
      },
    });

    await flushPromises();

    const btn = wrapper.get("button.btn");
    expect((btn.element as HTMLButtonElement).disabled).toBe(false);

    await btn.trigger("click");
    await flushPromises();

    expect(router.currentRoute.value.fullPath).toBe("/project");
  });
});
