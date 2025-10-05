import { defineStore } from "pinia";
import {
  WorkspaceInfo,
  RepositoryInfo,
  WorkspaceStatusQuery,
  WorkspaceStatusResponse,
  WorkspaceStatusFilter,
  WorkspaceBatchCloneRequest,
  WorkspaceBatchFetchRequest,
  WorkspaceBatchPushRequest,
  createWorkspace as createWorkspaceApi,
  loadWorkspace as loadWorkspaceApi,
  saveWorkspace as saveWorkspaceApi,
  getWorkspace as getWorkspaceApi,
  closeWorkspace as closeWorkspaceApi,
  addRepository as addRepositoryApi,
  removeRepository as removeRepositoryApi,
  toggleRepositoryEnabled as toggleRepositoryEnabledApi,
  updateRepositoryTags as updateRepositoryTagsApi,
  reorderRepositories as reorderRepositoriesApi,
  listRepositories as listRepositoriesApi,
  getWorkspaceStatuses as getWorkspaceStatusesApi,
  clearWorkspaceStatusCache as clearWorkspaceStatusCacheApi,
  invalidateWorkspaceStatusEntry as invalidateWorkspaceStatusEntryApi,
  workspaceBatchClone as workspaceBatchCloneApi,
  workspaceBatchFetch as workspaceBatchFetchApi,
  workspaceBatchPush as workspaceBatchPushApi,
} from "../api/workspace";
import {
  TemplateExportOptions,
  TemplateImportOptions,
  TemplateImportReport,
  exportTeamConfigTemplate,
  importTeamConfigTemplate,
} from "../api/config";

interface WorkspaceState {
  current: WorkspaceInfo | null;
  repositories: RepositoryInfo[];
  loadingWorkspace: boolean;
  loadingStatus: boolean;
  status: WorkspaceStatusResponse | null;
  statusQuery: WorkspaceStatusQuery;
  selectedRepoIds: string[];
  lastError: string | null;
  lastBatchTaskId: string | null;
  lastTemplateReport: TemplateImportReport | null;
  lastBatchOperation: "clone" | "fetch" | "push" | null;
}

const defaultStatusFilter: WorkspaceStatusFilter = {};

export const useWorkspaceStore = defineStore("workspace", {
  state: (): WorkspaceState => ({
    current: null,
    repositories: [],
    loadingWorkspace: false,
    loadingStatus: false,
    status: null,
    statusQuery: { filter: { ...defaultStatusFilter }, includeDisabled: false },
    selectedRepoIds: [],
    lastError: null,
    lastBatchTaskId: null,
    lastTemplateReport: null,
    lastBatchOperation: null,
  }),
  getters: {
    hasWorkspace: (state) => Boolean(state.current),
    selectedRepositories(state): RepositoryInfo[] {
      return state.repositories.filter((repo) => state.selectedRepoIds.includes(repo.id));
    },
  },
  actions: {
    setError(message: string | null) {
      this.lastError = message;
    },
    async initialize() {
      this.loadingWorkspace = true;
      try {
        const workspace = await getWorkspaceApi();
        this.applyWorkspace(workspace);
      } catch (error: any) {
        this.current = null;
        this.repositories = [];
        if (error?.toString && !String(error).includes("No workspace")) {
          this.lastError = String(error);
        }
      } finally {
        this.loadingWorkspace = false;
      }
    },
    applyWorkspace(workspace: WorkspaceInfo) {
      this.current = workspace;
      this.repositories = workspace.repositories.slice();
      const selected = this.selectedRepoIds.filter((id) => this.repositories.some((repo) => repo.id === id));
      this.selectedRepoIds = selected;
    },
    async createWorkspace(payload: { name: string; rootPath: string; metadata?: Record<string, string> }) {
      this.loadingWorkspace = true;
      try {
        const ws = await createWorkspaceApi(payload);
        this.applyWorkspace(ws);
      } catch (error: any) {
        this.lastError = String(error);
        throw error;
      } finally {
        this.loadingWorkspace = false;
      }
    },
    async loadWorkspace(path: string) {
      this.loadingWorkspace = true;
      try {
        const ws = await loadWorkspaceApi(path);
        this.applyWorkspace(ws);
      } catch (error: any) {
        this.lastError = String(error);
        throw error;
      } finally {
        this.loadingWorkspace = false;
      }
    },
    async saveWorkspace(path: string) {
      this.loadingWorkspace = true;
      try {
        await saveWorkspaceApi(path);
      } catch (error: any) {
        this.lastError = String(error);
        throw error;
      } finally {
        this.loadingWorkspace = false;
      }
    },
    async closeWorkspace() {
      try {
        await closeWorkspaceApi();
      } finally {
        this.current = null;
        this.repositories = [];
        this.selectedRepoIds = [];
        this.status = null;
        this.lastBatchTaskId = null;
        this.lastBatchOperation = null;
      }
    },
    async refreshRepositories() {
      if (!this.hasWorkspace) return;
      const repos = await listRepositoriesApi();
      this.repositories = repos;
      this.selectedRepoIds = this.selectedRepoIds.filter((id) => repos.some((repo) => repo.id === id));
    },
    async addRepository(request: { id: string; name: string; path: string; remoteUrl: string; tags?: string[]; enabled?: boolean }) {
      await addRepositoryApi(request);
      await this.refreshRepositories();
    },
    async removeRepository(id: string) {
      await removeRepositoryApi(id);
      await this.refreshRepositories();
    },
    async toggleRepositoryEnabled(id: string) {
      const enabled = await toggleRepositoryEnabledApi(id);
      const repo = this.repositories.find((r) => r.id === id);
      if (repo) repo.enabled = enabled;
      if (!enabled) {
        this.selectedRepoIds = this.selectedRepoIds.filter((rid) => rid !== id);
      }
    },
    async updateRepositoryTags(id: string, tags: string[]) {
      await updateRepositoryTagsApi(id, tags);
      const repo = this.repositories.find((r) => r.id === id);
      if (repo) repo.tags = tags.slice();
    },
    async reorderRepositories(order: string[]) {
      const updated = await reorderRepositoriesApi(order);
      this.repositories = updated;
    },
    setStatusQuery(query: WorkspaceStatusQuery) {
      this.statusQuery = { ...this.statusQuery, ...query };
    },
    setStatusFilter(update: Partial<WorkspaceStatusFilter>) {
      const base = { ...this.statusQuery.filter, ...update } as WorkspaceStatusFilter;
      this.statusQuery = { ...this.statusQuery, filter: base };
    },
    async fetchStatuses(options?: WorkspaceStatusQuery) {
      if (!this.hasWorkspace) {
        this.status = null;
        return;
      }
      this.loadingStatus = true;
      try {
        const merged: WorkspaceStatusQuery = {
          ...this.statusQuery,
          ...(options ?? {}),
        };
        const response = await getWorkspaceStatusesApi(merged);
        this.status = response;
      } catch (error: any) {
        this.lastError = String(error);
        throw error;
      } finally {
        this.loadingStatus = false;
      }
    },
    async forceRefreshStatuses() {
      await this.fetchStatuses({ ...this.statusQuery, forceRefresh: true });
    },
    async clearStatusCache() {
      await clearWorkspaceStatusCacheApi();
      await this.forceRefreshStatuses();
    },
    async invalidateStatusEntry(repoId: string) {
      await invalidateWorkspaceStatusEntryApi(repoId);
      await this.fetchStatuses({ ...this.statusQuery, forceRefresh: true });
    },
    selectRepositories(ids: string[]) {
      this.selectedRepoIds = ids;
    },
    toggleRepositorySelection(id: string) {
      if (this.selectedRepoIds.includes(id)) {
        this.selectedRepoIds = this.selectedRepoIds.filter((rid) => rid !== id);
      } else {
        this.selectedRepoIds = [...this.selectedRepoIds, id];
      }
    },
    selectAll(select: boolean) {
      if (select) {
        this.selectedRepoIds = this.repositories.map((repo) => repo.id);
      } else {
        this.selectedRepoIds = [];
      }
    },
    async startBatchClone(request: WorkspaceBatchCloneRequest) {
      const taskId = await workspaceBatchCloneApi(request);
      this.lastBatchTaskId = taskId;
       this.lastBatchOperation = "clone";
      return taskId;
    },
    async startBatchFetch(request: WorkspaceBatchFetchRequest) {
      const taskId = await workspaceBatchFetchApi(request);
      this.lastBatchTaskId = taskId;
       this.lastBatchOperation = "fetch";
      return taskId;
    },
    async startBatchPush(request: WorkspaceBatchPushRequest) {
      const taskId = await workspaceBatchPushApi(request);
      this.lastBatchTaskId = taskId;
       this.lastBatchOperation = "push";
      return taskId;
    },
    async exportTeamConfig(destination?: string, options?: TemplateExportOptions) {
      const path = await exportTeamConfigTemplate(destination, options);
      return path;
    },
    async importTeamConfig(source?: string, options?: TemplateImportOptions) {
      const report = await importTeamConfigTemplate(source, options);
      this.lastTemplateReport = report;
      return report;
    },
  },
});
