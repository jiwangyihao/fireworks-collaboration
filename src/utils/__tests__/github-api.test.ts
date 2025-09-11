import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";

vi.mock("../github-auth", () => ({
  loadAccessToken: vi.fn(),
}));

import { loadAccessToken } from "../github-auth";
import * as api from "../github-api";

function mockFetchResponse({
  ok,
  status = 200,
  statusText = "OK",
  body,
}: {
  ok: boolean;
  status?: number;
  statusText?: string;
  body: any;
}) {
  return {
    ok,
    status,
    statusText,
    json: async () => (typeof body === "string" ? JSON.parse(body) : body),
    text: async () => (typeof body === "string" ? body : JSON.stringify(body)),
  } as any;
}

describe("utils/github-api", () => {
  const g: any = globalThis as any;

  beforeEach(() => {
    (loadAccessToken as any).mockReset();
    g.fetch = vi.fn();
    (loadAccessToken as any).mockResolvedValue("tok");
  });
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("forkRepository: 成功返回 JSON", async () => {
    (g.fetch as any).mockResolvedValueOnce(
      mockFetchResponse({ ok: true, body: { id: 1 } }),
    );
    const res = await api.forkRepository("o", "r");
    expect(res.id).toBe(1);
    const [url, init] = (g.fetch as any).mock.calls[0];
    expect(url).toContain("/repos/o/r/forks");
    expect(init.method).toBe("POST");
    expect(init.headers.Authorization).toContain("Bearer tok");
  });

  it("forkRepository: 失败返回 message", async () => {
    (g.fetch as any).mockResolvedValueOnce(
      mockFetchResponse({
        ok: false,
        body: { message: "Nope" },
        status: 403,
        statusText: "Forbidden",
      }),
    );
    await expect(api.forkRepository("o", "r")).rejects.toThrow(/Nope|失败/);
  });

  it("createPullRequest: 成功与失败", async () => {
    (g.fetch as any).mockResolvedValueOnce(
      mockFetchResponse({ ok: true, body: { number: 10 } }),
    );
    const res = await api.createPullRequest("o", "r", {
      title: "t",
      head: "u:b",
      base: "main",
    });
    expect(res.number).toBe(10);

    (g.fetch as any).mockResolvedValueOnce(
      mockFetchResponse({
        ok: false,
        body: { message: "bad" },
        status: 422,
        statusText: "Unprocessable Entity",
      }),
    );
    await expect(
      api.createPullRequest("o", "r", {
        title: "t",
        head: "u:b",
        base: "main",
      }),
    ).rejects.toThrow(/bad|失败/);
  });

  it("listSSHKeys: token 缺失抛错", async () => {
    (loadAccessToken as any).mockResolvedValueOnce(null);
    await expect(api.listSSHKeys()).rejects.toThrow(/未找到访问令牌/);
  });

  it("listSSHKeys: 成功与失败", async () => {
    (g.fetch as any).mockResolvedValueOnce(
      mockFetchResponse({ ok: true, body: [{ id: 1 }] }),
    );
    const keys = await api.listSSHKeys();
    expect(keys.length).toBe(1);

    (g.fetch as any).mockResolvedValueOnce(
      mockFetchResponse({
        ok: false,
        body: { message: "err" },
        status: 500,
        statusText: "ISE",
      }),
    );
    await expect(api.listSSHKeys()).rejects.toThrow(/失败/);
  });

  it("addSSHKey/deleteSSHKey: 成功与失败", async () => {
    (g.fetch as any).mockResolvedValueOnce(
      mockFetchResponse({ ok: true, body: { id: 2 } }),
    );
    const created = await api.addSSHKey("t", "k");
    expect(created.id).toBe(2);

    (g.fetch as any).mockResolvedValueOnce(
      mockFetchResponse({
        ok: false,
        body: { message: "no" },
        status: 400,
        statusText: "Bad",
      }),
    );
    await expect(api.addSSHKey("t", "k")).rejects.toThrow(/失败/);

    (g.fetch as any).mockResolvedValueOnce(
      mockFetchResponse({ ok: true, body: {} }),
    );
    await expect(api.deleteSSHKey(1)).resolves.toBeUndefined();

    (g.fetch as any).mockResolvedValueOnce(
      mockFetchResponse({ ok: false, body: { message: "bad" } }),
    );
    await expect(api.deleteSSHKey(1)).rejects.toThrow(/失败/);
  });

  it("getRepository: 成功与失败", async () => {
    (g.fetch as any).mockResolvedValueOnce(
      mockFetchResponse({ ok: true, body: { full_name: "o/r" } }),
    );
    const repo = await api.getRepository("o", "r");
    expect(repo.full_name).toBe("o/r");

    (g.fetch as any).mockResolvedValueOnce(
      mockFetchResponse({
        ok: false,
        body: { message: "404" },
        status: 404,
        statusText: "NF",
      }),
    );
    await expect(api.getRepository("o", "r")).rejects.toThrow(/失败/);
  });

  it("checkIfForked: 已 fork 且返回同步状态", async () => {
    // 第一次 fetch: 查询 me/r 仓库信息
    (g.fetch as any).mockResolvedValueOnce(
      mockFetchResponse({
        ok: true,
        body: { fork: true, parent: { full_name: "o/r" } },
      }),
    );
    // 第二次 fetch: compare 接口，返回 ahead/behind
    (g.fetch as any).mockResolvedValueOnce(
      mockFetchResponse({ ok: true, body: { ahead_by: 1, behind_by: 0 } }),
    );

    const res = await api.checkIfForked("o", "r", "me");
    expect(res.isForked).toBe(true);
    expect(res.syncStatus?.aheadBy).toBe(1);
  });

  it("checkIfForked: 非 fork 或错误返回 false", async () => {
    (g.fetch as any).mockResolvedValueOnce(
      mockFetchResponse({ ok: true, body: { fork: false } }),
    );
    const res1 = await api.checkIfForked("o", "r", "me");
    expect(res1.isForked).toBe(false);

    (g.fetch as any).mockRejectedValueOnce(new Error("net"));
    const res2 = await api.checkIfForked("o", "r", "me");
    expect(res2.isForked).toBe(false);
  });

  it("getForkSyncStatus: main 失败回退 master", async () => {
    (g.fetch as any)
      .mockResolvedValueOnce(
        mockFetchResponse({
          ok: false,
          body: { message: "not found" },
          status: 404,
          statusText: "NF",
        }),
      )
      .mockResolvedValueOnce(
        mockFetchResponse({ ok: true, body: { ahead_by: 0, behind_by: 0 } }),
      );
    const res = await api.getForkSyncStatus("me", "r", "o", "r");
    expect(res.isSynced).toBe(true);
  });

  it("syncFork: main 失败且包含 branch 触发回退 master", async () => {
    (g.fetch as any)
      .mockResolvedValueOnce(
        mockFetchResponse({
          ok: false,
          body: { message: "branch not found" },
          status: 422,
          statusText: "bad",
        }),
      )
      .mockResolvedValueOnce(
        mockFetchResponse({ ok: true, body: { merged: true } }),
      );
    const res = await api.syncFork("me", "r");
    expect(res.merged).toBe(true);
  });

  it("canSyncFork: 根据权限判断 true/false 与错误返回 false", async () => {
    (g.fetch as any).mockResolvedValueOnce(
      mockFetchResponse({
        ok: true,
        body: { fork: true, permissions: { push: true } },
      }),
    );
    expect(await api.canSyncFork("me", "r")).toBe(true);

    (g.fetch as any).mockResolvedValueOnce(
      mockFetchResponse({ ok: false, body: {} }),
    );
    expect(await api.canSyncFork("me", "r")).toBe(false);

    (g.fetch as any).mockRejectedValueOnce(new Error("net"));
    expect(await api.canSyncFork("me", "r")).toBe(false);
  });

  it("getForkDefaultBranch: 返回 default_branch 或抛错", async () => {
    // 成功：getRepository -> ok true
    (g.fetch as any).mockResolvedValueOnce(
      mockFetchResponse({ ok: true, body: { default_branch: "dev" } }),
    );
    expect(await api.getForkDefaultBranch("me", "r")).toBe("dev");

    // 失败：getRepository -> ok false 触发抛错
    (g.fetch as any).mockResolvedValueOnce(
      mockFetchResponse({
        ok: false,
        status: 404,
        statusText: "NF",
        body: { message: "not" },
      }),
    );
    await expect(api.getForkDefaultBranch("me", "r")).rejects.toThrow(/失败/);
  });

  it("listUserRepositories: 支持 username 路径", async () => {
    (g.fetch as any).mockResolvedValueOnce(
      mockFetchResponse({ ok: true, body: [] }),
    );
    await api.listUserRepositories("someone");
    expect((g.fetch as any).mock.calls[0][0]).toContain("/users/someone/repos");
  });
});
