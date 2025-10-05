import { describe, it, expect, beforeEach, vi } from "vitest";
import { createPinia, setActivePinia } from "pinia";

vi.mock("../../api/workspace", () => ({
  createWorkspace: vi.fn(),
  loadWorkspace: vi.fn(),
  saveWorkspace: vi.fn(),
  getWorkspace: vi.fn(),
  closeWorkspace: vi.fn(),
  addRepository: vi.fn(),
  removeRepository: vi.fn(),
  toggleRepositoryEnabled: vi.fn(),
  updateRepositoryTags: vi.fn(),
  reorderRepositories: vi.fn(),
  listRepositories: vi.fn(),
  getWorkspaceStatuses: vi.fn(),
  clearWorkspaceStatusCache: vi.fn(),
  invalidateWorkspaceStatusEntry: vi.fn(),
  workspaceBatchClone: vi.fn(),
  workspaceBatchFetch: vi.fn(),
  workspaceBatchPush: vi.fn(),
}));

vi.mock("../../api/config", () => ({
  exportTeamConfigTemplate: vi.fn(),
  importTeamConfigTemplate: vi.fn(),
}));

import { useWorkspaceStore } from "../workspace";
import {
  loadWorkspace,
  saveWorkspace,
  getWorkspace,
  closeWorkspace,
  addRepository,
  toggleRepositoryEnabled,
  reorderRepositories,
  listRepositories,
  getWorkspaceStatuses,
  clearWorkspaceStatusCache,
  invalidateWorkspaceStatusEntry,
  workspaceBatchClone,
  workspaceBatchFetch,
  workspaceBatchPush,
} from "../../api/workspace";
import { exportTeamConfigTemplate, importTeamConfigTemplate } from "../../api/config";
import type { WorkspaceInfo, RepositoryInfo, WorkspaceStatusResponse } from "../../api/workspace";

const sampleRepo: RepositoryInfo = {
  id: "frontend",
  name: "frontend",
  path: "apps/frontend",
  remoteUrl: "https://example.com/frontend.git",
  tags: ["web"],
  enabled: true,
};

const sampleWorkspace: WorkspaceInfo = {
  name: "demo",
  rootPath: "/demo",
  repositories: [sampleRepo],
  createdAt: new Date().toISOString(),
  updatedAt: new Date().toISOString(),
  metadata: {},
};

function buildStatusResponse(): WorkspaceStatusResponse {
  return {
    generatedAt: new Date().toISOString(),
    total: 1,
    refreshed: 1,
    cached: 0,
    cacheTtlSecs: 30,
    autoRefreshSecs: null,
    queried: 1,
    missingRepoIds: [],
    summary: {
      workingStates: { clean: 1, dirty: 0, missing: 0, error: 0 },
      syncStates: { clean: 1, ahead: 0, behind: 0, diverged: 0, detached: 0, unknown: 0 },
      errorCount: 0,
      errorRepositories: [],
    },
    statuses: [],
  };
}

describe("stores/workspace", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.clearAllMocks();
  });

  it("initialize loads workspace when available", async () => {
    (getWorkspace as any).mockResolvedValueOnce(sampleWorkspace);
    const store = useWorkspaceStore();

    await store.initialize();

    expect(store.current?.name).toBe("demo");
    expect(store.repositories).toHaveLength(1);
    expect(store.lastError).toBeNull();
  });

  it("addRepository refreshes list", async () => {
    const store = useWorkspaceStore();
    store.applyWorkspace(sampleWorkspace);
    (addRepository as any).mockResolvedValueOnce(undefined);
    (listRepositories as any).mockResolvedValueOnce([sampleRepo, { ...sampleRepo, id: "backend", name: "backend" }]);

    await store.addRepository({ id: "backend", name: "backend", path: "apps/backend", remoteUrl: "https://example.com/backend.git" });

    expect(addRepository).toHaveBeenCalled();
    expect(listRepositories).toHaveBeenCalled();
    expect(store.repositories).toHaveLength(2);
  });

  it("toggleRepositoryEnabled updates repository state", async () => {
    const store = useWorkspaceStore();
    store.applyWorkspace(sampleWorkspace);
    store.selectRepositories([sampleRepo.id]);
    (toggleRepositoryEnabled as any).mockResolvedValueOnce(false);

    await store.toggleRepositoryEnabled(sampleRepo.id);

    expect(toggleRepositoryEnabled).toHaveBeenCalledWith(sampleRepo.id);
    expect(store.repositories[0].enabled).toBe(false);
    expect(store.selectedRepoIds).toHaveLength(0);
  });

  it("reorderRepositories applies returned order", async () => {
    const store = useWorkspaceStore();
    store.applyWorkspace({
      ...sampleWorkspace,
      repositories: [sampleRepo, { ...sampleRepo, id: "backend", name: "backend" }],
    });
    const reordered = [
      { ...sampleRepo, id: "backend", name: "backend" },
      sampleRepo,
    ];
    (reorderRepositories as any).mockResolvedValueOnce(reordered);

    await store.reorderRepositories(["backend", "frontend"]);

    expect(reorderRepositories).toHaveBeenCalledWith(["backend", "frontend"]);
    expect(store.repositories[0].id).toBe("backend");
  });

  it("refreshRepositories keeps selection in sync", async () => {
    const backendRepo: RepositoryInfo = { ...sampleRepo, id: "backend", name: "backend" };
    const store = useWorkspaceStore();
    store.applyWorkspace({
      ...sampleWorkspace,
      repositories: [sampleRepo, backendRepo],
    });
    store.selectRepositories([sampleRepo.id, backendRepo.id]);
    (listRepositories as any).mockResolvedValueOnce([backendRepo]);

    await store.refreshRepositories();

    expect(listRepositories).toHaveBeenCalled();
    expect(store.repositories).toEqual([backendRepo]);
    expect(store.selectedRepoIds).toEqual([backendRepo.id]);
  });

  it("fetchStatuses stores latest response", async () => {
    const store = useWorkspaceStore();
    store.applyWorkspace(sampleWorkspace);
    const response = buildStatusResponse();
    (getWorkspaceStatuses as any).mockResolvedValueOnce(response);

    await store.fetchStatuses();

    expect(getWorkspaceStatuses).toHaveBeenCalled();
    expect(store.status).toEqual(response);
  });

  it("clearStatusCache clears cache and forces a refresh", async () => {
    const store = useWorkspaceStore();
    store.applyWorkspace(sampleWorkspace);
    const response = buildStatusResponse();
    (getWorkspaceStatuses as any).mockResolvedValue(response);

    await store.clearStatusCache();

    expect(clearWorkspaceStatusCache).toHaveBeenCalledTimes(1);
    expect(getWorkspaceStatuses).toHaveBeenCalledTimes(1);
    const calls = (getWorkspaceStatuses as any).mock.calls;
    expect(calls[0][0]).toMatchObject({ forceRefresh: true });
    expect(store.status).toEqual(response);
  });

  it("invalidateStatusEntry forces refresh after cache eviction", async () => {
    const store = useWorkspaceStore();
    store.applyWorkspace(sampleWorkspace);
    const response = buildStatusResponse();
    (invalidateWorkspaceStatusEntry as any).mockResolvedValueOnce(true);
    (getWorkspaceStatuses as any).mockResolvedValue(response);

    await store.invalidateStatusEntry(sampleRepo.id);

    expect(invalidateWorkspaceStatusEntry).toHaveBeenCalledWith(sampleRepo.id);
    expect(getWorkspaceStatuses).toHaveBeenCalled();
    const calls = (getWorkspaceStatuses as any).mock.calls;
    expect(calls[0][0]).toMatchObject({ forceRefresh: true });
    expect(store.status).toEqual(response);
  });

  it("startBatchClone records task id", async () => {
    const store = useWorkspaceStore();
    (workspaceBatchClone as any).mockResolvedValueOnce("123");

    const taskId = await store.startBatchClone({});

    expect(taskId).toBe("123");
    expect(store.lastBatchTaskId).toBe("123");
    expect(store.lastBatchOperation).toBe("clone");
  });

  it("startBatchFetch records task id", async () => {
    const store = useWorkspaceStore();
    (workspaceBatchFetch as any).mockResolvedValueOnce("456");

    const taskId = await store.startBatchFetch({});

    expect(taskId).toBe("456");
    expect(store.lastBatchTaskId).toBe("456");
    expect(store.lastBatchOperation).toBe("fetch");
  });

  it("startBatchPush records task id", async () => {
    const store = useWorkspaceStore();
    (workspaceBatchPush as any).mockResolvedValueOnce("789");

    const taskId = await store.startBatchPush({});

    expect(taskId).toBe("789");
    expect(store.lastBatchTaskId).toBe("789");
    expect(store.lastBatchOperation).toBe("push");
  });

  it("exportTeamConfig returns exported path", async () => {
    const store = useWorkspaceStore();
    (exportTeamConfigTemplate as any).mockResolvedValueOnce("/tmp/team.json");

    const path = await store.exportTeamConfig();

    expect(exportTeamConfigTemplate).toHaveBeenCalled();
    expect(path).toBe("/tmp/team.json");
  });

  it("importTeamConfig stores report", async () => {
    const store = useWorkspaceStore();
    const report = {
      schemaVersion: "1.0.0",
      applied: [],
      skipped: [],
      warnings: [],
      backupPath: undefined,
    };
    (importTeamConfigTemplate as any).mockResolvedValueOnce(report);

    const result = await store.importTeamConfig();

    expect(importTeamConfigTemplate).toHaveBeenCalled();
    expect(store.lastTemplateReport).toEqual(report);
    expect(result).toEqual(report);
  });

  it("closeWorkspace clears state", async () => {
    (closeWorkspace as any).mockResolvedValueOnce(undefined);
    const store = useWorkspaceStore();
    store.applyWorkspace(sampleWorkspace);

    await store.closeWorkspace();

    expect(closeWorkspace).toHaveBeenCalled();
    expect(store.current).toBeNull();
    expect(store.repositories).toHaveLength(0);
    expect(store.lastBatchTaskId).toBeNull();
    expect(store.lastBatchOperation).toBeNull();
  });

  it("loadWorkspace propagates errors", async () => {
    (loadWorkspace as any).mockRejectedValueOnce(new Error("failed"));
    const store = useWorkspaceStore();

    await expect(store.loadWorkspace("foo" as any)).rejects.toThrow("failed");
    expect(store.lastError).toContain("failed");
  });

  it("saveWorkspace propagates errors", async () => {
    (saveWorkspace as any).mockRejectedValueOnce(new Error("write error"));
    const store = useWorkspaceStore();

    await expect(store.saveWorkspace("/tmp/workspace.json" as any)).rejects.toThrow("write error");
    expect(store.lastError).toContain("write error");
  });
});
