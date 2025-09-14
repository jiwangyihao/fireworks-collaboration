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
    const id = await startGitFetch("", "C:/tmp/repo", "branches+tags");
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
});
