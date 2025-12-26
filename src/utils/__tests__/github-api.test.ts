import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";

const { tauriFetchMock } = vi.hoisted(() => ({
  tauriFetchMock: vi.fn(),
}));

vi.mock("../../api/tauri-fetch", () => ({
  fetch: tauriFetchMock,
  tauriFetch: tauriFetchMock,
}));

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
  beforeEach(() => {
    (loadAccessToken as any).mockReset();
    (loadAccessToken as any).mockResolvedValue("tok");
    tauriFetchMock.mockReset();
  });
  afterEach(() => {
    vi.clearAllMocks();
  });

  it("forkRepository: 成功返回 JSON", async () => {
    tauriFetchMock.mockResolvedValueOnce(
      mockFetchResponse({ ok: true, body: { id: 1 } })
    );
    const res = await api.forkRepository("o", "r");
    expect(res.id).toBe(1);
    const [url, init] = tauriFetchMock.mock.calls[0];
    expect(url).toContain("/repos/o/r/forks");
    expect(init.method).toBe("POST");
    expect(init.headers.Authorization).toContain("Bearer tok");
  });

  it("forkRepository: 失败返回 message", async () => {
    tauriFetchMock.mockResolvedValueOnce(
      mockFetchResponse({
        ok: false,
        body: { message: "Nope" },
        status: 403,
        statusText: "Forbidden",
      })
    );
    await expect(api.forkRepository("o", "r")).rejects.toThrow(/Nope|失败/);
  });

  it("createPullRequest: 成功与失败", async () => {
    tauriFetchMock.mockResolvedValueOnce(
      mockFetchResponse({ ok: true, body: { number: 10 } })
    );
    const res = await api.createPullRequest("o", "r", {
      title: "t",
      head: "u:b",
      base: "main",
    });
    expect(res.number).toBe(10);

    tauriFetchMock.mockResolvedValueOnce(
      mockFetchResponse({
        ok: false,
        body: { message: "bad" },
        status: 422,
        statusText: "Unprocessable Entity",
      })
    );
    await expect(
      api.createPullRequest("o", "r", {
        title: "t",
        head: "u:b",
        base: "main",
      })
    ).rejects.toThrow(/bad|失败/);
  });

  it("listSSHKeys: token 缺失抛错", async () => {
    (loadAccessToken as any).mockResolvedValueOnce(null);
    await expect(api.listSSHKeys()).rejects.toThrow(/未找到访问令牌/);
  });

  it("listSSHKeys: 成功与失败", async () => {
    tauriFetchMock.mockResolvedValueOnce(
      mockFetchResponse({ ok: true, body: [{ id: 1 }] })
    );
    const keys = await api.listSSHKeys();
    expect(keys.length).toBe(1);

    tauriFetchMock.mockResolvedValueOnce(
      mockFetchResponse({
        ok: false,
        body: { message: "err" },
        status: 500,
        statusText: "ISE",
      })
    );
    await expect(api.listSSHKeys()).rejects.toThrow(/失败/);
  });

  it("addSSHKey/deleteSSHKey: 成功与失败", async () => {
    tauriFetchMock.mockResolvedValueOnce(
      mockFetchResponse({ ok: true, body: { id: 2 } })
    );
    const created = await api.addSSHKey("t", "k");
    expect(created.id).toBe(2);

    tauriFetchMock.mockResolvedValueOnce(
      mockFetchResponse({
        ok: false,
        body: { message: "no" },
        status: 400,
        statusText: "Bad",
      })
    );
    await expect(api.addSSHKey("t", "k")).rejects.toThrow(/失败/);

    tauriFetchMock.mockResolvedValueOnce(
      mockFetchResponse({ ok: true, body: {} })
    );
    await expect(api.deleteSSHKey(1)).resolves.toBeUndefined();

    tauriFetchMock.mockResolvedValueOnce(
      mockFetchResponse({ ok: false, body: { message: "bad" } })
    );
    await expect(api.deleteSSHKey(1)).rejects.toThrow(/失败/);
  });

  it("getRepository: 成功与失败", async () => {
    tauriFetchMock.mockResolvedValueOnce(
      mockFetchResponse({ ok: true, body: { full_name: "o/r" } })
    );
    const repo = await api.getRepository("o", "r");
    expect(repo.full_name).toBe("o/r");

    tauriFetchMock.mockResolvedValueOnce(
      mockFetchResponse({
        ok: false,
        body: { message: "404" },
        status: 404,
        statusText: "NF",
      })
    );
    await expect(api.getRepository("o", "r")).rejects.toThrow(/失败/);
  });

  it("checkIfForked: 已 fork 且返回同步状态", async () => {
    // 第一次 fetch: 查询 me/r 仓库信息
    tauriFetchMock.mockResolvedValueOnce(
      mockFetchResponse({
        ok: true,
        body: { fork: true, parent: { full_name: "o/r" } },
      })
    );
    // 第二次 fetch: compare 接口，返回 ahead/behind
    tauriFetchMock.mockResolvedValueOnce(
      mockFetchResponse({ ok: true, body: { ahead_by: 1, behind_by: 0 } })
    );

    const res = await api.checkIfForked("o", "r", "me");
    expect(res.isForked).toBe(true);
    expect(res.syncStatus?.aheadBy).toBe(1);
  });

  it("checkIfForked: 非 fork 或错误返回 false", async () => {
    tauriFetchMock.mockResolvedValueOnce(
      mockFetchResponse({ ok: true, body: { fork: false } })
    );
    const res1 = await api.checkIfForked("o", "r", "me");
    expect(res1.isForked).toBe(false);

    tauriFetchMock.mockRejectedValueOnce(new Error("net"));
    const res2 = await api.checkIfForked("o", "r", "me");
    expect(res2.isForked).toBe(false);
  });

  it("getForkSyncStatus: main 失败回退 master", async () => {
    tauriFetchMock
      .mockResolvedValueOnce(
        mockFetchResponse({
          ok: false,
          body: { message: "not found" },
          status: 404,
          statusText: "NF",
        })
      )
      .mockResolvedValueOnce(
        mockFetchResponse({ ok: true, body: { ahead_by: 0, behind_by: 0 } })
      );
    const res = await api.getForkSyncStatus("me", "r", "o", "r");
    expect(res.isSynced).toBe(true);
  });

  it("syncFork: main 失败且包含 branch 触发回退 master", async () => {
    tauriFetchMock
      .mockResolvedValueOnce(
        mockFetchResponse({
          ok: false,
          body: { message: "branch not found" },
          status: 422,
          statusText: "bad",
        })
      )
      .mockResolvedValueOnce(
        mockFetchResponse({ ok: true, body: { merged: true } })
      );
    const res = await api.syncFork("me", "r");
    expect(res.merged).toBe(true);
  });

  it("canSyncFork: 根据权限判断 true/false 与错误返回 false", async () => {
    tauriFetchMock.mockResolvedValueOnce(
      mockFetchResponse({
        ok: true,
        body: { fork: true, permissions: { push: true } },
      })
    );
    expect(await api.canSyncFork("me", "r")).toBe(true);

    tauriFetchMock.mockResolvedValueOnce(
      mockFetchResponse({ ok: false, body: {} })
    );
    expect(await api.canSyncFork("me", "r")).toBe(false);

    tauriFetchMock.mockRejectedValueOnce(new Error("net"));
    expect(await api.canSyncFork("me", "r")).toBe(false);
  });

  it("getForkDefaultBranch: 返回 default_branch 或抛错", async () => {
    // 成功：getRepository -> ok true
    tauriFetchMock.mockResolvedValueOnce(
      mockFetchResponse({ ok: true, body: { default_branch: "dev" } })
    );
    expect(await api.getForkDefaultBranch("me", "r")).toBe("dev");

    // 失败：getRepository -> ok false 触发抛错
    tauriFetchMock.mockResolvedValueOnce(
      mockFetchResponse({
        ok: false,
        status: 404,
        statusText: "NF",
        body: { message: "not" },
      })
    );
    await expect(api.getForkDefaultBranch("me", "r")).rejects.toThrow(/失败/);
  });

  it("listUserRepositories: 支持 username 路径", async () => {
    tauriFetchMock.mockResolvedValueOnce(
      mockFetchResponse({ ok: true, body: [] })
    );
    await api.listUserRepositories("someone");
    expect(tauriFetchMock.mock.calls[0][0]).toContain("/users/someone/repos");
  });

  it("listUserRepositories: 无 username 使用 /user/repos", async () => {
    tauriFetchMock.mockResolvedValueOnce(
      mockFetchResponse({ ok: true, body: [] })
    );
    await api.listUserRepositories();
    expect(tauriFetchMock.mock.calls[0][0]).toContain("/user/repos");
  });

  it("listUserRepositories: 失败抛错", async () => {
    tauriFetchMock.mockResolvedValueOnce(
      mockFetchResponse({
        ok: false,
        body: { message: "err" },
        status: 500,
        statusText: "ISE",
      })
    );
    await expect(api.listUserRepositories()).rejects.toThrow(/失败/);
  });

  it("forceSyncFork: main 失败回退 master", async () => {
    // 第一次获取 upstream 分支失败
    tauriFetchMock
      .mockResolvedValueOnce(
        mockFetchResponse({
          ok: false,
          body: { message: "not found" },
          status: 404,
          statusText: "NF",
        })
      )
      // 第二次使用 master 分支成功
      .mockResolvedValueOnce(
        mockFetchResponse({
          ok: true,
          body: { commit: { sha: "abc123" } },
        })
      )
      // 更新 ref 成功
      .mockResolvedValueOnce(
        mockFetchResponse({
          ok: true,
          body: { ref: "refs/heads/master" },
        })
      );
    const res = await api.forceSyncFork("me", "r", "upstream", "r");
    expect(res.ref).toBe("refs/heads/master");
  });

  it("forceSyncFork: 更新 ref 失败抛错", async () => {
    tauriFetchMock
      .mockResolvedValueOnce(
        mockFetchResponse({
          ok: true,
          body: { commit: { sha: "abc123" } },
        })
      )
      .mockResolvedValueOnce(
        mockFetchResponse({
          ok: false,
          body: { message: "forbidden" },
          status: 403,
          statusText: "Forbidden",
        })
      );
    await expect(api.forceSyncFork("me", "r", "upstream", "r")).rejects.toThrow(
      /失败/
    );
  });

  it("createBranch: 成功与失败", async () => {
    tauriFetchMock.mockResolvedValueOnce(
      mockFetchResponse({ ok: true, body: { ref: "refs/heads/new" } })
    );
    const res = await api.createBranch("o", "r", "new", "sha123");
    expect(res.ref).toBe("refs/heads/new");

    tauriFetchMock.mockResolvedValueOnce(
      mockFetchResponse({
        ok: false,
        body: { message: "exists" },
        status: 422,
        statusText: "bad",
      })
    );
    await expect(api.createBranch("o", "r", "new", "sha")).rejects.toThrow(
      /失败/
    );
  });

  it("getBranch: 成功与失败", async () => {
    tauriFetchMock.mockResolvedValueOnce(
      mockFetchResponse({ ok: true, body: { name: "main" } })
    );
    const res = await api.getBranch("o", "r", "main");
    expect(res.name).toBe("main");

    tauriFetchMock.mockResolvedValueOnce(
      mockFetchResponse({
        ok: false,
        body: { message: "not found" },
        status: 404,
        statusText: "NF",
      })
    );
    await expect(api.getBranch("o", "r", "missing")).rejects.toThrow(/失败/);
  });

  it("getFileContent: 带 ref 参数与失败", async () => {
    tauriFetchMock.mockResolvedValueOnce(
      mockFetchResponse({ ok: true, body: { content: "abc" } })
    );
    const res = await api.getFileContent("o", "r", "path/to/file", "dev");
    expect(res.content).toBe("abc");
    expect(tauriFetchMock.mock.calls[0][0]).toContain("ref=dev");

    tauriFetchMock.mockResolvedValueOnce(
      mockFetchResponse({
        ok: false,
        body: { message: "not found" },
        status: 404,
        statusText: "NF",
      })
    );
    await expect(api.getFileContent("o", "r", "missing")).rejects.toThrow(
      /失败/
    );
  });

  it("createOrUpdateFile: 成功与失败", async () => {
    tauriFetchMock.mockResolvedValueOnce(
      mockFetchResponse({ ok: true, body: { commit: { sha: "new" } } })
    );
    const res = await api.createOrUpdateFile("o", "r", "file.txt", {
      message: "add",
      content: "YWJj",
    });
    expect(res.commit.sha).toBe("new");

    tauriFetchMock.mockResolvedValueOnce(
      mockFetchResponse({
        ok: false,
        body: { message: "conflict" },
        status: 409,
        statusText: "Conflict",
      })
    );
    await expect(
      api.createOrUpdateFile("o", "r", "file.txt", {
        message: "update",
        content: "xyz",
        sha: "old",
      })
    ).rejects.toThrow(/失败/);
  });

  it("listPullRequests: 支持 options 参数", async () => {
    tauriFetchMock.mockResolvedValueOnce(
      mockFetchResponse({ ok: true, body: [{ number: 1 }] })
    );
    const res = await api.listPullRequests("o", "r", {
      state: "open",
      base: "main",
      per_page: 10,
    });
    expect(res[0].number).toBe(1);
    const url = tauriFetchMock.mock.calls[0][0];
    expect(url).toContain("state=open");
    expect(url).toContain("base=main");
    expect(url).toContain("per_page=10");
  });

  it("listPullRequests: 失败抛错", async () => {
    tauriFetchMock.mockResolvedValueOnce(
      mockFetchResponse({
        ok: false,
        body: { message: "err" },
        status: 500,
        statusText: "ISE",
      })
    );
    await expect(api.listPullRequests("o", "r")).rejects.toThrow(/失败/);
  });

  it("getPullRequest: 成功与失败", async () => {
    tauriFetchMock.mockResolvedValueOnce(
      mockFetchResponse({ ok: true, body: { number: 42 } })
    );
    const res = await api.getPullRequest("o", "r", 42);
    expect(res.number).toBe(42);

    tauriFetchMock.mockResolvedValueOnce(
      mockFetchResponse({
        ok: false,
        body: { message: "not found" },
        status: 404,
        statusText: "NF",
      })
    );
    await expect(api.getPullRequest("o", "r", 999)).rejects.toThrow(/失败/);
  });

  it("listBranches: 支持 options 参数", async () => {
    tauriFetchMock.mockResolvedValueOnce(
      mockFetchResponse({ ok: true, body: [{ name: "main" }] })
    );
    const res = await api.listBranches("o", "r", {
      protected: true,
      per_page: 5,
    });
    expect(res[0].name).toBe("main");
    const url = tauriFetchMock.mock.calls[0][0];
    expect(url).toContain("protected=true");
    expect(url).toContain("per_page=5");
  });

  it("listBranches: 失败抛错", async () => {
    tauriFetchMock.mockResolvedValueOnce(
      mockFetchResponse({
        ok: false,
        body: { message: "err" },
        status: 500,
        statusText: "ISE",
      })
    );
    await expect(api.listBranches("o", "r")).rejects.toThrow(/失败/);
  });

  it("listContributors: 成功与失败", async () => {
    tauriFetchMock.mockResolvedValueOnce(
      mockFetchResponse({ ok: true, body: [{ login: "user1" }] })
    );
    const res = await api.listContributors("o", "r", { per_page: 3 });
    expect(res[0].login).toBe("user1");

    tauriFetchMock.mockResolvedValueOnce(
      mockFetchResponse({
        ok: false,
        body: { message: "err" },
        status: 500,
        statusText: "ISE",
      })
    );
    await expect(api.listContributors("o", "r")).rejects.toThrow(/失败/);
  });

  it("getLanguages: 成功与失败", async () => {
    tauriFetchMock.mockResolvedValueOnce(
      mockFetchResponse({ ok: true, body: { TypeScript: 1000, Rust: 500 } })
    );
    const res = await api.getLanguages("o", "r");
    expect(res.TypeScript).toBe(1000);

    tauriFetchMock.mockResolvedValueOnce(
      mockFetchResponse({
        ok: false,
        body: { message: "err" },
        status: 500,
        statusText: "ISE",
      })
    );
    await expect(api.getLanguages("o", "r")).rejects.toThrow(/失败/);
  });

  it("getLatestRelease: 成功、404 返回 null、其他失败抛错", async () => {
    tauriFetchMock.mockResolvedValueOnce(
      mockFetchResponse({ ok: true, body: { tag_name: "v1.0" } })
    );
    const res = await api.getLatestRelease("o", "r");
    expect(res.tag_name).toBe("v1.0");

    tauriFetchMock.mockResolvedValueOnce(
      mockFetchResponse({
        ok: false,
        body: {},
        status: 404,
        statusText: "NF",
      })
    );
    const noRelease = await api.getLatestRelease("o", "r");
    expect(noRelease).toBeNull();

    tauriFetchMock.mockResolvedValueOnce(
      mockFetchResponse({
        ok: false,
        body: { message: "err" },
        status: 500,
        statusText: "ISE",
      })
    );
    await expect(api.getLatestRelease("o", "r")).rejects.toThrow(/失败/);
  });

  it("listCommits: 支持 options 参数", async () => {
    tauriFetchMock.mockResolvedValueOnce(
      mockFetchResponse({ ok: true, body: [{ sha: "abc" }] })
    );
    const res = await api.listCommits("o", "r", { sha: "dev", per_page: 5 });
    expect(res[0].sha).toBe("abc");
    const url = tauriFetchMock.mock.calls[0][0];
    expect(url).toContain("sha=dev");
    expect(url).toContain("per_page=5");
  });

  it("listCommits: 失败抛错", async () => {
    tauriFetchMock.mockResolvedValueOnce(
      mockFetchResponse({
        ok: false,
        body: { message: "err" },
        status: 500,
        statusText: "ISE",
      })
    );
    await expect(api.listCommits("o", "r")).rejects.toThrow(/失败/);
  });
});
