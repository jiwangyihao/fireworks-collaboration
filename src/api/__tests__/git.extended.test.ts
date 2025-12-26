import { describe, it, expect, vi, beforeEach } from "vitest";

const { mockInvoke } = vi.hoisted(() => ({
  mockInvoke: vi.fn(),
}));

vi.mock("../tauri", () => ({
  invoke: mockInvoke,
}));

import {
  startGitInit,
  startGitAdd,
  startGitCommit,
  startGitBranch,
  startGitCheckout,
  startGitTag,
  startGitRemoteSet,
  startGitRemoteAdd,
  startGitRemoteRemove,
  getGitBranches,
  getGitRepoStatus,
  deleteGitBranch,
  getWorktrees,
  addWorktree,
  removeWorktree,
  getRemoteBranches,
} from "../tasks";

describe("api/git extended", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
  });

  it("startGitInit calls git_init", async () => {
    mockInvoke.mockResolvedValueOnce("t-init");
    await startGitInit("path/to/repo");
    expect(mockInvoke).toHaveBeenCalledWith("git_init", {
      dest: "path/to/repo",
    });
  });

  it("startGitAdd calls git_add", async () => {
    mockInvoke.mockResolvedValueOnce("t-add");
    await startGitAdd("path/to/repo", ["file1", "file2"]);
    expect(mockInvoke).toHaveBeenCalledWith("git_add", {
      dest: "path/to/repo",
      paths: ["file1", "file2"],
    });
  });

  it("startGitCommit calls git_commit with optional params", async () => {
    mockInvoke.mockResolvedValueOnce("t-commit");
    await startGitCommit({
      dest: "repo",
      message: "feat: something",
      allowEmpty: true,
      authorName: "Me",
    });
    expect(mockInvoke).toHaveBeenCalledWith("git_commit", {
      dest: "repo",
      message: "feat: something",
      allowEmpty: true,
      authorName: "Me",
    });
  });

  it("startGitBranch calls git_branch", async () => {
    mockInvoke.mockResolvedValueOnce("t-branch");
    await startGitBranch({ dest: "repo", name: "new-branch", checkout: true });
    expect(mockInvoke).toHaveBeenCalledWith("git_branch", {
      dest: "repo",
      name: "new-branch",
      checkout: true,
    });
  });

  it("startGitCheckout calls git_checkout", async () => {
    mockInvoke.mockResolvedValueOnce("t-checkout");
    await startGitCheckout({ dest: "repo", reference: "main", create: false });
    expect(mockInvoke).toHaveBeenCalledWith("git_checkout", {
      dest: "repo",
      reference: "main",
      create: false,
    });
  });

  it("startGitTag calls git_tag", async () => {
    mockInvoke.mockResolvedValueOnce("t-tag");
    await startGitTag({
      dest: "repo",
      name: "v1.0",
      message: "release",
      annotated: true,
    });
    expect(mockInvoke).toHaveBeenCalledWith("git_tag", {
      dest: "repo",
      name: "v1.0",
      message: "release",
      annotated: true,
    });
  });

  describe("Remote operations", () => {
    it("startGitRemoteSet calls git_remote_set", async () => {
      mockInvoke.mockResolvedValueOnce("t-remote-set");
      await startGitRemoteSet({
        dest: "repo",
        name: "origin",
        url: "git@example.com:repo.git",
      });
      expect(mockInvoke).toHaveBeenCalledWith("git_remote_set", {
        dest: "repo",
        name: "origin",
        url: "git@example.com:repo.git",
      });
    });

    it("startGitRemoteAdd calls git_remote_add", async () => {
      mockInvoke.mockResolvedValueOnce("t-remote-add");
      await startGitRemoteAdd({
        dest: "repo",
        name: "upstream",
        url: "git@example.com:upstream.git",
      });
      expect(mockInvoke).toHaveBeenCalledWith("git_remote_add", {
        dest: "repo",
        name: "upstream",
        url: "git@example.com:upstream.git",
      });
    });

    it("startGitRemoteRemove calls git_remote_remove", async () => {
      mockInvoke.mockResolvedValueOnce("t-remote-rm");
      await startGitRemoteRemove({ dest: "repo", name: "upstream" });
      expect(mockInvoke).toHaveBeenCalledWith("git_remote_remove", {
        dest: "repo",
        name: "upstream",
      });
    });
  });

  describe("Query functions", () => {
    it("getGitBranches calls git_list_branches", async () => {
      mockInvoke.mockResolvedValueOnce([]);
      await getGitBranches("repo", true);
      expect(mockInvoke).toHaveBeenCalledWith("git_list_branches", {
        dest: "repo",
        includeRemote: true,
      });
    });

    it("getGitRepoStatus calls git_repo_status", async () => {
      mockInvoke.mockResolvedValueOnce({});
      await getGitRepoStatus("repo");
      expect(mockInvoke).toHaveBeenCalledWith("git_repo_status", {
        dest: "repo",
      });
    });

    it("deleteGitBranch calls git_delete_branch", async () => {
      mockInvoke.mockResolvedValueOnce(undefined);
      await deleteGitBranch("repo", "feature", true);
      expect(mockInvoke).toHaveBeenCalledWith("git_delete_branch", {
        dest: "repo",
        name: "feature",
        force: true,
      });
    });

    it("getRemoteBranches calls git_remote_branches", async () => {
      mockInvoke.mockResolvedValueOnce(["main", "dev"]);
      const res = await getRemoteBranches("repo", "origin", true);
      expect(res).toEqual(["main", "dev"]);
      expect(mockInvoke).toHaveBeenCalledWith("git_remote_branches", {
        dest: "repo",
        remote: "origin",
        fetchFirst: true,
      });
    });
  });

  describe("Worktree functions", () => {
    it("getWorktrees calls git_worktree_list", async () => {
      mockInvoke.mockResolvedValueOnce([]);
      await getWorktrees("repo");
      expect(mockInvoke).toHaveBeenCalledWith("git_worktree_list", {
        dest: "repo",
      });
    });

    it("addWorktree calls git_worktree_add", async () => {
      mockInvoke.mockResolvedValueOnce(undefined);
      await addWorktree("repo", "wt-path", "new-branch", true, "origin/base");
      expect(mockInvoke).toHaveBeenCalledWith("git_worktree_add", {
        dest: "repo",
        path: "wt-path",
        branch: "new-branch",
        createBranch: true,
        fromRemote: "origin/base",
      });
    });

    it("removeWorktree calls git_worktree_remove", async () => {
      mockInvoke.mockResolvedValueOnce(undefined);
      await removeWorktree("repo", "wt-path", true, true, "upstream", true);
      expect(mockInvoke).toHaveBeenCalledWith("git_worktree_remove", {
        dest: "repo",
        path: "wt-path",
        force: true,
        deleteRemoteBranch: true,
        remote: "upstream",
        useStoredCredential: true,
      });
    });
  });
});
