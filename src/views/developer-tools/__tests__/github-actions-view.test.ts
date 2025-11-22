import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { mount, flushPromises } from "@vue/test-utils";

vi.mock("../../../utils/github-auth", () => ({
  getUserInfo: vi.fn(),
}));
vi.mock("../../../utils/github-api", () => ({
  checkIfForked: vi.fn(),
  forkRepository: vi.fn(),
  createPullRequest: vi.fn(),
  listSSHKeys: vi.fn(),
  addSSHKey: vi.fn(),
  deleteSSHKey: vi.fn(),
  syncFork: vi.fn(),
}));

import { getUserInfo } from "../../../utils/github-auth";
import { checkIfForked, listSSHKeys } from "../../../utils/github-api";
import GitHubActionsView from "../GitHubActionsView.vue";

describe("views/GitHubActionsView", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    localStorage.clear();
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it("onMounted: 本地存在 token 时加载用户并预填 PR head", async () => {
    localStorage.setItem("github_access_token", "tok");
    (getUserInfo as any).mockResolvedValueOnce({ login: "me", name: "Me" });

    const wrapper = mount(GitHubActionsView);
    await flushPromises();

    expect(getUserInfo).toHaveBeenCalledWith("tok");
    const headInput = wrapper.get('input[placeholder="username:branch"]');
    expect((headInput.element as HTMLInputElement).value).toBe(
      "me:feature-branch",
    );
  });

  it("checkForkStatus: 显示需要同步的状态", async () => {
    localStorage.setItem("github_access_token", "tok");
    (getUserInfo as any).mockResolvedValueOnce({ login: "me", name: "Me" });
    (checkIfForked as any).mockResolvedValueOnce({
      isForked: true,
      syncStatus: { aheadBy: 1, behindBy: 0, isSynced: false },
      forkData: {},
    });

    const wrapper = mount(GitHubActionsView);
    await flushPromises();

    // 触发检查
    const btn = wrapper.get("button.btn.btn-outline");
    await btn.trigger("click");
    await flushPromises();

    // 展示警告状态与“需要同步”文案
    const alert = wrapper.get(".alert.alert-warning");
    expect(alert.text()).toMatch(/需要同步/);
  });

  it("SSH: 刷新密钥列表后展示项", async () => {
    (getUserInfo as any).mockResolvedValueOnce(null); // 不必加载用户
    (listSSHKeys as any).mockResolvedValueOnce([
      { id: 1, title: "k1", key: "ssh-rsa AAA...", created_at: Date.now() },
    ]);

    const wrapper = mount(GitHubActionsView);
    await flushPromises();

    const btns = wrapper.findAll("button.btn.btn-outline");
    const refresh = btns.find((b) => b.text().includes("刷新密钥列表"));
    expect(refresh).toBeTruthy();

    await refresh!.trigger("click");
    await flushPromises();

    expect(wrapper.text()).toContain("k1");
  });
});
