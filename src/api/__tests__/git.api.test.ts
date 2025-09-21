import { describe, it, expect, vi, beforeEach } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

import { invoke } from "@tauri-apps/api/core";
import { startGitClone, startGitFetch, startGitPush } from "../tasks";

describe("api/git clone 调用", () => {
  beforeEach(() => {
    (invoke as any).mockReset();
  });

  it("startGitClone 会调用 git_clone 并返回 taskId", async () => {
    (invoke as any).mockResolvedValueOnce("task-xyz");
    const id = await startGitClone("https://github.com/rust-lang/log", "C:/tmp/log");
    expect(invoke).toHaveBeenCalledWith("git_clone", {
      repo: "https://github.com/rust-lang/log",
      dest: "C:/tmp/log",
    });
    expect(id).toBe("task-xyz");
  });

  it("startGitFetch 会调用 git_fetch 并返回 taskId（默认 remote）", async () => {
    (invoke as any).mockResolvedValueOnce("task-abc");
    const id = await startGitFetch("", "C:/tmp/repo");
    expect(invoke).toHaveBeenCalledWith("git_fetch", { repo: "", dest: "C:/tmp/repo" });
    expect(id).toBe("task-abc");
  });

  it("startGitFetch 支持传递 preset 并透传给后端", async () => {
    (invoke as any).mockResolvedValueOnce("task-def");
    const id = await startGitFetch("", "C:/tmp/repo", { preset: "branches+tags" });
    expect(invoke).toHaveBeenCalledWith("git_fetch", { repo: "", dest: "C:/tmp/repo", preset: "branches+tags" });
    expect(id).toBe("task-def");
  });

  it("startGitPush 仅传 dest 使用默认参数", async () => {
    (invoke as any).mockResolvedValueOnce("task-push-1");
    const id = await startGitPush({ dest: "C:/tmp/repo" });
    expect(invoke).toHaveBeenCalledWith("git_push", { dest: "C:/tmp/repo" });
    expect(id).toBe("task-push-1");
  });

  it("startGitPush 透传 remote/refspecs/credentials", async () => {
    (invoke as any).mockResolvedValueOnce("task-push-2");
    const id = await startGitPush({ dest: "C:/tmp/repo", remote: "origin", refspecs: ["refs/heads/main:refs/heads/main"], username: "x-access-token", password: "ghp_xxx" });
    expect(invoke).toHaveBeenCalledWith("git_push", {
      dest: "C:/tmp/repo",
      remote: "origin",
      refspecs: ["refs/heads/main:refs/heads/main"],
      username: "x-access-token",
      password: "ghp_xxx",
    });
    expect(id).toBe("task-push-2");
  });

  it("startGitClone 透传 strategyOverride", async () => {
    (invoke as any).mockResolvedValueOnce("task-so1");
    const id = await startGitClone("repo", "C:/tmp/repo", { strategyOverride: { http: { followRedirects: false, maxRedirects: 0 } } });
    expect(invoke).toHaveBeenCalledWith("git_clone", { repo: "repo", dest: "C:/tmp/repo", strategy_override: { http: { followRedirects: false, maxRedirects: 0 } } });
    expect(id).toBe("task-so1");
  });

  it("startGitClone 仅传 depth/filter (无 strategyOverride)", async () => {
    (invoke as any).mockResolvedValueOnce("task-df");
    const id = await startGitClone("repoX", "C:/tmp/r2", { depth: 1, filter: "blob:none" });
    expect(id).toBe("task-df");
    expect(invoke).toHaveBeenCalledWith("git_clone", { repo: "repoX", dest: "C:/tmp/r2", depth: 1, filter: "blob:none" });
  });

  it("startGitFetch 透传 strategyOverride + depth/filter", async () => {
    (invoke as any).mockResolvedValueOnce("task-so2");
    const id = await startGitFetch("repo2", "C:/tmp/repo2", { depth: 1, filter: "blob:none", strategyOverride: { retry: { max: 2 } } });
    expect(invoke).toHaveBeenCalledWith("git_fetch", { repo: "repo2", dest: "C:/tmp/repo2", depth: 1, filter: "blob:none", strategy_override: { retry: { max: 2 } } });
    expect(id).toBe("task-so2");
  });

  it("startGitPush 透传 strategyOverride", async () => {
    (invoke as any).mockResolvedValueOnce("task-so3");
    const id = await startGitPush({ dest: "C:/tmp/repo3", strategyOverride: { tls: { insecureSkipVerify: true } } });
    expect(invoke).toHaveBeenCalledWith("git_push", { dest: "C:/tmp/repo3", strategy_override: { tls: { insecureSkipVerify: true } } });
    expect(id).toBe("task-so3");
  });

  it("startGitPush 透传 http+tls+retry 全量 strategyOverride", async () => {
    (invoke as any).mockResolvedValueOnce("task-so4");
    const override = { http: { followRedirects: false, maxRedirects: 1 }, tls: { skipSanWhitelist: true }, retry: { max: 2, baseMs: 400, factor: 1.2, jitter: false } };
    const id = await startGitPush({ dest: "C:/tmp/repo4", strategyOverride: override });
    expect(id).toBe("task-so4");
    expect(invoke).toHaveBeenCalledWith("git_push", { dest: "C:/tmp/repo4", strategy_override: override });
  });

  it("startGitFetch 组合 preset+depth+filter+strategyOverride", async () => {
    (invoke as any).mockResolvedValueOnce("task-so5");
    const id = await startGitFetch("r-fetch", "C:/tmp/fetch", { preset: "branches", depth: 2, filter: "blob:none", strategyOverride: { http: { followRedirects: true, maxRedirects: 2 }, retry: { max: 3 } } });
    expect(id).toBe("task-so5");
    expect(invoke).toHaveBeenCalledWith("git_fetch", { repo: "r-fetch", dest: "C:/tmp/fetch", preset: "branches", depth: 2, filter: "blob:none", strategy_override: { http: { followRedirects: true, maxRedirects: 2 }, retry: { max: 3 } } });
  });

  it("startGitFetch 仅 strategyOverride", async () => {
    (invoke as any).mockResolvedValueOnce("task-so5b");
    const id = await startGitFetch("r-fetch2", "C:/tmp/fetch2", { strategyOverride: { http: { followRedirects: false } } });
    expect(id).toBe("task-so5b");
    expect(invoke).toHaveBeenCalledWith("git_fetch", { repo: "r-fetch2", dest: "C:/tmp/fetch2", strategy_override: { http: { followRedirects: false } } });
  });

  it("startGitClone depth+filter+strategyOverride 全量", async () => {
    (invoke as any).mockResolvedValueOnce("task-so6");
    const id = await startGitClone("r-clone", "C:/tmp/clone", { depth: 3, filter: "blob:none", strategyOverride: { tls: { insecureSkipVerify: true }, retry: { max: 1, jitter: false } } });
    expect(id).toBe("task-so6");
    expect(invoke).toHaveBeenCalledWith("git_clone", { repo: "r-clone", dest: "C:/tmp/clone", depth: 3, filter: "blob:none", strategy_override: { tls: { insecureSkipVerify: true }, retry: { max: 1, jitter: false } } });
  });

  it("startGitClone 空 strategyOverride 对象", async () => {
    (invoke as any).mockResolvedValueOnce("task-so6b");
    const id = await startGitClone("r-clone-empty", "C:/tmp/cloneE", { strategyOverride: {} });
    expect(id).toBe("task-so6b");
    expect(invoke).toHaveBeenCalledWith("git_clone", { repo: "r-clone-empty", dest: "C:/tmp/cloneE", strategy_override: {} });
  });

  it("startGitPush retry-only strategyOverride", async () => {
    (invoke as any).mockResolvedValueOnce("task-so7");
    const id = await startGitPush({ dest: "C:/tmp/retry", strategyOverride: { retry: { max: 5, baseMs: 250 } } });
    expect(id).toBe("task-so7");
    expect(invoke).toHaveBeenCalledWith("git_push", { dest: "C:/tmp/retry", strategy_override: { retry: { max: 5, baseMs: 250 } } });
  });

  it("startGitPush credentials + refspecs + strategyOverride", async () => {
    (invoke as any).mockResolvedValueOnce("task-so7b");
    const id = await startGitPush({ dest: "C:/tmp/retry2", remote: "origin", refspecs: ["refs/heads/main:refs/heads/main"], username: "x-access-token", password: "tok", strategyOverride: { http: { followRedirects: false }, tls: { skipSanWhitelist: true } } });
    expect(id).toBe("task-so7b");
    expect(invoke).toHaveBeenCalledWith("git_push", { dest: "C:/tmp/retry2", remote: "origin", refspecs: ["refs/heads/main:refs/heads/main"], username: "x-access-token", password: "tok", strategy_override: { http: { followRedirects: false }, tls: { skipSanWhitelist: true } } });
  });
});
