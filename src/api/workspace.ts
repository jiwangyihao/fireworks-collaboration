import { invoke } from "./tauri";

export interface RepositoryInfo {
  id: string;
  name: string;
  path: string;
  remoteUrl: string;
  tags: string[];
  enabled: boolean;
}

export interface WorkspaceInfo {
  name: string;
  rootPath: string;
  repositories: RepositoryInfo[];
  createdAt: string;
  updatedAt: string;
  metadata: Record<string, string>;
}

export interface CreateWorkspaceRequest {
  name: string;
  rootPath: string;
  metadata?: Record<string, string>;
}

export interface AddRepositoryRequest {
  id: string;
  name: string;
  path: string;
  remoteUrl: string;
  tags?: string[];
  enabled?: boolean;
}

export interface WorkspaceStatusFilter {
  branch?: string | null;
  nameContains?: string | null;
  tags?: string[] | null;
  hasLocalChanges?: boolean | null;
  syncStates?: SyncState[] | null;
}

export type StatusSortDirection = "asc" | "desc";

export type StatusSortField =
  | "name"
  | "branch"
  | "ahead"
  | "behind"
  | "lastUpdated"
  | "lastCommit"
  | "workingState";

export interface WorkspaceStatusSort {
  field: StatusSortField;
  direction?: StatusSortDirection;
}

export interface WorkspaceStatusQuery {
  repoIds?: string[];
  includeDisabled?: boolean;
  forceRefresh?: boolean;
  filter?: WorkspaceStatusFilter;
  sort?: WorkspaceStatusSort | null;
}

export type WorkingTreeState = "clean" | "dirty" | "missing" | "error";
export type SyncState = "clean" | "ahead" | "behind" | "diverged" | "detached" | "unknown";

export interface RepositoryStatus {
  repoId: string;
  name: string;
  enabled: boolean;
  path: string;
  tags: string[];
  remoteUrl?: string | null;
  currentBranch?: string | null;
  upstreamBranch?: string | null;
  ahead: number;
  behind: number;
  syncState: SyncState;
  workingState: WorkingTreeState;
  staged: number;
  unstaged: number;
  untracked: number;
  conflicts: number;
  lastCommitId?: string | null;
  lastCommitMessage?: string | null;
  lastCommitTime?: string | null;
  lastCommitUnixTime?: number | null;
  statusTimestamp: string;
  statusUnixTime: number;
  isCached: boolean;
  error?: string | null;
}

export interface WorkingStateSummary {
  clean: number;
  dirty: number;
  missing: number;
  error: number;
}

export interface SyncStateSummary {
  clean: number;
  ahead: number;
  behind: number;
  diverged: number;
  detached: number;
  unknown: number;
}

export interface WorkspaceStatusSummary {
  workingStates: WorkingStateSummary;
  syncStates: SyncStateSummary;
  errorCount: number;
  errorRepositories?: string[];
}

export interface WorkspaceStatusResponse {
  generatedAt: string;
  total: number;
  refreshed: number;
  cached: number;
  cacheTtlSecs: number;
  autoRefreshSecs?: number | null;
  queried: number;
  missingRepoIds: string[];
  summary: WorkspaceStatusSummary;
  statuses: RepositoryStatus[];
}

export interface WorkspaceBatchCloneRequest {
  repoIds?: string[];
  includeDisabled?: boolean;
  maxConcurrency?: number;
  depth?: number | null;
  filter?: string;
  strategyOverride?: unknown;
  recurseSubmodules?: boolean;
}

export interface WorkspaceBatchFetchRequest {
  repoIds?: string[];
  includeDisabled?: boolean;
  maxConcurrency?: number;
  preset?: string;
  depth?: number | null;
  filter?: string;
  strategyOverride?: unknown;
}

export interface WorkspaceBatchPushRequest {
  repoIds?: string[];
  includeDisabled?: boolean;
  maxConcurrency?: number;
  remote?: string;
  refspecs?: string[];
  username?: string;
  password?: string;
  strategyOverride?: unknown;
}

export async function createWorkspace(request: CreateWorkspaceRequest): Promise<WorkspaceInfo> {
  return invoke<WorkspaceInfo>("create_workspace", { request });
}

export async function loadWorkspace(path: string): Promise<WorkspaceInfo> {
  return invoke<WorkspaceInfo>("load_workspace", { path });
}

export async function saveWorkspace(path: string): Promise<void> {
  await invoke<void>("save_workspace", { path });
}

export async function getWorkspace(): Promise<WorkspaceInfo> {
  return invoke<WorkspaceInfo>("get_workspace");
}

export async function closeWorkspace(): Promise<void> {
  await invoke<void>("close_workspace");
}

export async function addRepository(request: AddRepositoryRequest): Promise<void> {
  await invoke<void>("add_repository", { request });
}

export async function removeRepository(repoId: string): Promise<void> {
  await invoke<void>("remove_repository", { repoId });
}

export async function toggleRepositoryEnabled(repoId: string): Promise<boolean> {
  return invoke<boolean>("toggle_repository_enabled", { repoId });
}

export async function updateRepositoryTags(repoId: string, tags: string[]): Promise<void> {
  await invoke<void>("update_repository_tags", { repoId, tags });
}

export async function reorderRepositories(order: string[]): Promise<RepositoryInfo[]> {
  return invoke<RepositoryInfo[]>("reorder_repositories", { orderedIds: order });
}

export async function listRepositories(): Promise<RepositoryInfo[]> {
  return invoke<RepositoryInfo[]>("list_repositories");
}

export async function listEnabledRepositories(): Promise<RepositoryInfo[]> {
  return invoke<RepositoryInfo[]>("list_enabled_repositories");
}

export async function getWorkspaceStatuses(query?: WorkspaceStatusQuery): Promise<WorkspaceStatusResponse> {
  const payload = query ? { request: sanitizeStatusQuery(query) } : undefined;
  return invoke<WorkspaceStatusResponse>("get_workspace_statuses", payload);
}

function sanitizeStatusQuery(query: WorkspaceStatusQuery): WorkspaceStatusQuery {
  const normalized: WorkspaceStatusQuery = { ...query };
  if (normalized.repoIds && normalized.repoIds.length === 0) {
    delete normalized.repoIds;
  }
  if (normalized.filter) {
    const cleaned: WorkspaceStatusFilter = { ...normalized.filter };
    for (const key of Object.keys(cleaned) as (keyof WorkspaceStatusFilter)[]) {
      const value = cleaned[key];
      if (value === null || value === undefined) {
        delete cleaned[key];
        continue;
      }
      if (Array.isArray(value) && value.length === 0) {
        delete cleaned[key];
      }
    }
    if (Object.keys(cleaned).length > 0) {
      normalized.filter = cleaned;
    } else {
      delete normalized.filter;
    }
  }
  if (normalized.sort === null) {
    delete normalized.sort;
  }
  return normalized;
}

export async function clearWorkspaceStatusCache(): Promise<void> {
  await invoke<void>("clear_workspace_status_cache");
}

export async function invalidateWorkspaceStatusEntry(repoId: string): Promise<boolean> {
  return invoke<boolean>("invalidate_workspace_status_entry", { repoId });
}

export async function workspaceBatchClone(request: WorkspaceBatchCloneRequest): Promise<string> {
  return invoke<string>("workspace_batch_clone", { request });
}

export async function workspaceBatchFetch(request: WorkspaceBatchFetchRequest): Promise<string> {
  return invoke<string>("workspace_batch_fetch", { request });
}

export async function workspaceBatchPush(request: WorkspaceBatchPushRequest): Promise<string> {
  return invoke<string>("workspace_batch_push", { request });
}
