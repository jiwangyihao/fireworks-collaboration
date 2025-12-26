import { describe, it, expect, vi, beforeEach } from "vitest";

const { mockInvoke } = vi.hoisted(() => ({
  mockInvoke: vi.fn(),
}));

vi.mock("../tauri", () => ({
  invoke: mockInvoke,
}));

import {
  getWorkspaceStatuses,
  workspaceBatchClone,
  createWorkspace,
  loadWorkspace,
  saveWorkspace,
  getWorkspace,
  closeWorkspace,
  addRepository,
  removeRepository,
  toggleRepositoryEnabled,
  updateRepositoryTags,
  reorderRepositories,
  listRepositories,
  listEnabledRepositories,
  clearWorkspaceStatusCache,
  invalidateWorkspaceStatusEntry,
  workspaceBatchFetch,
  workspaceBatchPush,
  type WorkspaceStatusQuery,
} from "../workspace";

describe("api/workspace", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
  });

  describe("getWorkspaceStatuses", () => {
    it("calls get_workspace_statuses with undefined payload when query is missing", async () => {
      mockInvoke.mockResolvedValueOnce({});
      await getWorkspaceStatuses();
      expect(mockInvoke).toHaveBeenCalledWith(
        "get_workspace_statuses",
        undefined
      );
    });

    it("sanitizes query: removes empty repoIds", async () => {
      mockInvoke.mockResolvedValueOnce({});
      const query: WorkspaceStatusQuery = {
        repoIds: [],
        includeDisabled: true,
      };
      await getWorkspaceStatuses(query);
      const args = mockInvoke.mock.calls[0][1] as any;
      expect(args.request.repoIds).toBeUndefined();
      expect(args.request.includeDisabled).toBe(true);
    });

    it("sanitizes query: cleans filter and removes it if empty", async () => {
      mockInvoke.mockResolvedValueOnce({});
      const query: WorkspaceStatusQuery = {
        filter: {
          branch: null,
          nameContains: undefined,
          tags: [],
        },
      };
      await getWorkspaceStatuses(query);
      const args = mockInvoke.mock.calls[0][1] as any;
      expect(args.request.filter).toBeUndefined();
    });

    it("sanitizes query: cleans filter but keeps valid values", async () => {
      mockInvoke.mockResolvedValueOnce({});
      const query: WorkspaceStatusQuery = {
        filter: {
          branch: "main",
          tags: ["active"],
          hasLocalChanges: null,
        },
      };
      await getWorkspaceStatuses(query);
      const args = mockInvoke.mock.calls[0][1] as any;
      expect(args.request.filter).toEqual({
        branch: "main",
        tags: ["active"],
      });
    });

    it("sanitizes query: removes sort if null", async () => {
      mockInvoke.mockResolvedValueOnce({});
      const query: WorkspaceStatusQuery = {
        sort: null,
      };
      await getWorkspaceStatuses(query);
      const args = mockInvoke.mock.calls[0][1] as any;
      expect(args.request.sort).toBeUndefined();
    });

    it("passes sort if provided", async () => {
      mockInvoke.mockResolvedValueOnce({});
      const query: WorkspaceStatusQuery = {
        sort: { field: "name", direction: "asc" },
      };
      await getWorkspaceStatuses(query);
      const args = mockInvoke.mock.calls[0][1] as any;
      expect(args.request.sort).toEqual({ field: "name", direction: "asc" });
    });
  });

  describe("workspaceBatchClone", () => {
    it("calls workspace_batch_clone with request", async () => {
      mockInvoke.mockResolvedValueOnce("job-123");
      const req = { repoIds: ["r1"], maxConcurrency: 2 };
      const res = await workspaceBatchClone(req);
      expect(res).toBe("job-123");
      expect(mockInvoke).toHaveBeenCalledWith("workspace_batch_clone", {
        request: req,
      });
    });
  });

  it("createWorkspace calls create_workspace", async () => {
    mockInvoke.mockResolvedValueOnce({});
    const req = { name: "test", rootPath: "/tmp" };
    await createWorkspace(req);
    expect(mockInvoke).toHaveBeenCalledWith("create_workspace", {
      request: req,
    });
  });

  it("loadWorkspace calls load_workspace", async () => {
    mockInvoke.mockResolvedValueOnce({});
    await loadWorkspace("/path");
    expect(mockInvoke).toHaveBeenCalledWith("load_workspace", {
      path: "/path",
    });
  });

  it("saveWorkspace calls save_workspace", async () => {
    mockInvoke.mockResolvedValueOnce(undefined);
    await saveWorkspace("/path");
    expect(mockInvoke).toHaveBeenCalledWith("save_workspace", {
      path: "/path",
    });
  });

  it("getWorkspace calls get_workspace", async () => {
    mockInvoke.mockResolvedValueOnce({});
    await getWorkspace();
    const [cmd] = mockInvoke.mock.calls[0];
    expect(cmd).toBe("get_workspace");
  });

  it("closeWorkspace calls close_workspace", async () => {
    mockInvoke.mockResolvedValueOnce(undefined);
    await closeWorkspace();
    const [cmd] = mockInvoke.mock.calls[0];
    expect(cmd).toBe("close_workspace");
  });

  it("addRepository calls add_repository", async () => {
    mockInvoke.mockResolvedValueOnce(undefined);
    const req = { id: "r1", name: "repo1", path: "/p", remoteUrl: "url" };
    await addRepository(req);
    expect(mockInvoke).toHaveBeenCalledWith("add_repository", { request: req });
  });

  it("removeRepository calls remove_repository", async () => {
    mockInvoke.mockResolvedValueOnce(undefined);
    await removeRepository("r1");
    expect(mockInvoke).toHaveBeenCalledWith("remove_repository", {
      repoId: "r1",
    });
  });

  it("toggleRepositoryEnabled calls toggle_repository_enabled", async () => {
    mockInvoke.mockResolvedValueOnce(true);
    await toggleRepositoryEnabled("r1");
    expect(mockInvoke).toHaveBeenCalledWith("toggle_repository_enabled", {
      repoId: "r1",
    });
  });

  it("updateRepositoryTags calls update_repository_tags", async () => {
    mockInvoke.mockResolvedValueOnce(undefined);
    await updateRepositoryTags("r1", ["tag"]);
    expect(mockInvoke).toHaveBeenCalledWith("update_repository_tags", {
      repoId: "r1",
      tags: ["tag"],
    });
  });

  it("reorderRepositories calls reorder_repositories", async () => {
    mockInvoke.mockResolvedValueOnce([]);
    await reorderRepositories(["r1", "r2"]);
    expect(mockInvoke).toHaveBeenCalledWith("reorder_repositories", {
      orderedIds: ["r1", "r2"],
    });
  });

  it("listRepositories calls list_repositories", async () => {
    mockInvoke.mockResolvedValueOnce([]);
    await listRepositories();
    const [cmd] = mockInvoke.mock.calls[0];
    expect(cmd).toBe("list_repositories");
  });

  it("listEnabledRepositories calls list_enabled_repositories", async () => {
    mockInvoke.mockResolvedValueOnce([]);
    await listEnabledRepositories();
    const [cmd] = mockInvoke.mock.calls[0];
    expect(cmd).toBe("list_enabled_repositories");
  });

  it("clearWorkspaceStatusCache calls clear_workspace_status_cache", async () => {
    mockInvoke.mockResolvedValueOnce(undefined);
    await clearWorkspaceStatusCache();
    const [cmd] = mockInvoke.mock.calls[0];
    expect(cmd).toBe("clear_workspace_status_cache");
  });

  it("invalidateWorkspaceStatusEntry calls invalidate_workspace_status_entry", async () => {
    mockInvoke.mockResolvedValueOnce(true);
    await invalidateWorkspaceStatusEntry("r1");
    expect(mockInvoke).toHaveBeenCalledWith(
      "invalidate_workspace_status_entry",
      {
        repoId: "r1",
      }
    );
  });

  it("workspaceBatchFetch calls workspace_batch_fetch", async () => {
    mockInvoke.mockResolvedValueOnce("job-f");
    const req = { repoIds: ["r1"] };
    await workspaceBatchFetch(req);
    expect(mockInvoke).toHaveBeenCalledWith("workspace_batch_fetch", {
      request: req,
    });
  });

  it("workspaceBatchPush calls workspace_batch_push", async () => {
    mockInvoke.mockResolvedValueOnce("job-p");
    const req = { repoIds: ["r1"], remote: "origin" };
    await workspaceBatchPush(req);
    expect(mockInvoke).toHaveBeenCalledWith("workspace_batch_push", {
      request: req,
    });
  });
});
