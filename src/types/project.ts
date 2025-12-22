// 项目配置常量
export const PROJECT_CONFIG = {
  UPSTREAM_OWNER: "HIT-Fireworks",
  UPSTREAM_REPO: "fireworks-notes-society",
  get UPSTREAM_FULL_NAME() {
    return `${this.UPSTREAM_OWNER}/${this.UPSTREAM_REPO}`;
  },
  get UPSTREAM_URL() {
    return `https://github.com/${this.UPSTREAM_FULL_NAME}`;
  },
} as const;

// 仓库所有者信息
export interface RepoOwner {
  login: string;
  avatar_url: string;
  html_url: string;
}

// 仓库License信息
export interface RepoLicense {
  key: string;
  name: string;
  spdx_id: string;
  url: string | null;
}

// 仓库基本信息（GitHub API返回的字段子集）
export interface RepoInfo {
  id: number;
  name: string;
  full_name: string;
  description: string | null;
  html_url: string;
  clone_url: string;
  ssh_url: string;
  default_branch: string;
  stargazers_count: number;
  forks_count: number;
  watchers_count: number;
  open_issues_count: number;
  language: string | null;
  topics: string[];
  license: RepoLicense | null;
  created_at: string;
  updated_at: string;
  pushed_at: string;
  owner: RepoOwner;
  // Fork相关字段
  fork: boolean;
  parent?: RepoInfo;
  source?: RepoInfo;
}

// Fork同步状态
export interface ForkSyncStatus {
  aheadBy: number;
  behindBy: number;
  isSynced: boolean;
}

// Fork仓库信息（扩展了同步状态）
export interface ForkInfo extends RepoInfo {
  syncStatus?: ForkSyncStatus;
}

// PR用户信息
export interface PRUser {
  login: string;
  avatar_url: string;
  html_url: string;
}

// PR分支信息
export interface PRBranchRef {
  ref: string;
  sha: string;
  label?: string;
  user?: PRUser;
  repo?: RepoInfo;
}

// Pull Request信息
export interface PullRequestInfo {
  id: number;
  number: number;
  title: string;
  body: string | null;
  state: "open" | "closed";
  merged: boolean;
  html_url: string;
  head: PRBranchRef;
  base: PRBranchRef;
  user: PRUser;
  created_at: string;
  updated_at: string;
  merged_at: string | null;
  closed_at: string | null;
  draft: boolean;
  mergeable?: boolean | null;
  mergeable_state?: string;
  review_comments?: number;
  commits?: number;
  additions?: number;
  deletions?: number;
  changed_files?: number;
}

// 贡献者信息
export interface ContributorInfo {
  login: string;
  avatar_url: string;
  html_url: string;
  contributions: number;
}

// Release信息
export interface ReleaseInfo {
  id: number;
  tag_name: string;
  name: string;
  body: string | null;
  html_url: string;
  published_at: string;
  author: {
    login: string;
    avatar_url: string;
  };
  prerelease: boolean;
  draft: boolean;
}

// 分支信息
export interface BranchInfo {
  name: string;
  commit: {
    sha: string;
    url: string;
  };
  protected: boolean;
}

// Commit信息
export interface CommitInfo {
  sha: string;
  commit: {
    message: string;
    author: {
      name: string;
      email: string;
      date: string;
    };
  };
  author: {
    login: string;
    avatar_url: string;
    html_url: string;
  } | null;
  html_url: string;
}

// Git Worktree信息
export interface WorktreeInfo {
  path: string;
  branch: string;
  isMainWorktree: boolean;
  head?: string;
  locked?: boolean;
  prunable?: boolean;
  // 扩展字段：PR关联
  linkedPR?: number;
  linkedPRUrl?: string;
  linkedPRTitle?: string;
  createdAt?: string;
}

// 本地仓库状态
export interface LocalRepoStatus {
  exists: boolean;
  path: string | null;
  currentBranch: string | null;
  workingTreeClean: boolean;
  staged: number;
  unstaged: number;
  untracked: number;
  ahead: number;
  behind: number;
  worktrees: WorktreeInfo[];
}

// 项目状态枚举
export type ProjectLoadingState =
  | "idle"
  | "loading-upstream"
  | "loading-fork"
  | "loading-local"
  | "loading-prs"
  | "forking"
  | "syncing-fork"
  | "cloning"
  | "creating-worktree";

// 项目Store状态接口
export interface ProjectState {
  // 远端仓库
  upstreamRepo: RepoInfo | null;
  forkRepo: ForkInfo | null;
  hasFork: boolean;

  // 本地仓库
  localStatus: LocalRepoStatus | null;

  // PR列表
  pullRequests: PullRequestInfo[];
  myPullRequests: PullRequestInfo[]; // 当前用户创建的PR

  // 附加信息
  contributors: ContributorInfo[];
  languages: Record<string, number>;
  latestRelease: ReleaseInfo | null;
  branches: BranchInfo[];
  forkBranches: BranchInfo[];
  forkCommits: CommitInfo[];

  // 加载状态
  loadingState: ProjectLoadingState;

  // 错误信息
  lastError: string | null;

  // 当前用户
  currentUser: string | null;
}
