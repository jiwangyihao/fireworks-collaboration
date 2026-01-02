import { defineStore } from "pinia";
import { appDataDir } from "@tauri-apps/api/path";
import { exists, mkdir } from "@tauri-apps/plugin-fs";
import type {
  ProjectState,
  RepoInfo,
  ForkInfo,
  PullRequestInfo,
  ContributorInfo,
  ReleaseInfo,
  BranchInfo,
  ProjectLoadingState,
  ForkSyncStatus,
} from "../types/project";
import { PROJECT_CONFIG } from "../types/project";
import {
  getRepository,
  checkIfForked,
  forkRepository,
  syncFork,
  forceSyncFork,
  getForkSyncStatus,
  listPullRequests,
  listBranches,
  listContributors,
  getLanguages,
  getLatestRelease,
  createPullRequest,
  listCommits,
} from "../utils/github-api";
import { loadAccessToken, getUserInfo } from "../utils/github-auth";

export const useProjectStore = defineStore("project", {
  state: (): ProjectState => ({
    // 远端仓库
    upstreamRepo: null,
    forkRepo: null,
    hasFork: false,

    // 本地仓库
    localStatus: null,

    // PR列表
    pullRequests: [],
    myPullRequests: [],

    // 附加信息
    contributors: [],
    languages: {},
    latestRelease: null,
    branches: [],
    forkBranches: [],
    forkCommits: [],

    // 加载状态
    loadingState: "idle",

    // 错误信息
    lastError: null,

    // 当前用户
    currentUser: null,
  }),

  getters: {
    isLoading: (state) => state.loadingState !== "idle",

    upstreamOwner: () => PROJECT_CONFIG.UPSTREAM_OWNER,
    upstreamRepoName: () => PROJECT_CONFIG.UPSTREAM_REPO,
    upstreamFullName: () => PROJECT_CONFIG.UPSTREAM_FULL_NAME,

    forkFullName: (state) => {
      if (!state.currentUser) return null;
      return `${state.currentUser}/${PROJECT_CONFIG.UPSTREAM_REPO}`;
    },

    canFork: (state) => !state.hasFork && state.upstreamRepo !== null,
    canSync: (state) =>
      state.hasFork &&
      state.forkRepo?.syncStatus &&
      !state.forkRepo.syncStatus.isSynced,

    localRepoExists: (state) => state.localStatus?.exists ?? false,
    canClone: (state) => state.hasFork && !(state.localStatus?.exists ?? false),

    // 语言统计转换为百分比
    languagePercentages: (state) => {
      const total = Object.values(state.languages).reduce(
        (sum, val) => sum + val,
        0
      );
      if (total === 0) return {};
      return Object.fromEntries(
        Object.entries(state.languages).map(([lang, bytes]) => [
          lang,
          Math.round((bytes / total) * 1000) / 10, // 保留一位小数
        ])
      );
    },

    // 获取fork同步状态描述
    syncStatusText: (state) => {
      if (!state.forkRepo?.syncStatus) return "未知";
      const { aheadBy, behindBy, isSynced } = state.forkRepo.syncStatus;
      if (isSynced) return "已同步";
      const parts = [];
      if (aheadBy > 0) parts.push(`领先 ${aheadBy} 个提交`);
      if (behindBy > 0) parts.push(`落后 ${behindBy} 个提交`);
      return parts.join("，");
    },
  },

  actions: {
    setError(message: string | null) {
      this.lastError = message;
    },

    setLoadingState(state: ProjectLoadingState) {
      this.loadingState = state;
    },

    // 初始化：加载当前用户信息
    async initCurrentUser() {
      try {
        const token = await loadAccessToken();
        if (token) {
          const userInfo = await getUserInfo(token);
          this.currentUser = userInfo.login;
        }
      } catch (error) {
        console.error("获取当前用户信息失败:", error);
      }
    },

    // 加载上游仓库信息
    async fetchUpstreamRepo() {
      this.loadingState = "loading-upstream";
      this.lastError = null;

      try {
        const repo = await getRepository(
          PROJECT_CONFIG.UPSTREAM_OWNER,
          PROJECT_CONFIG.UPSTREAM_REPO
        );
        this.upstreamRepo = repo as RepoInfo;
      } catch (error: any) {
        this.lastError = `加载上游仓库失败: ${error.message || error}`;
        throw error;
      } finally {
        this.loadingState = "idle";
      }
    },

    // 检查并加载Fork仓库
    async checkAndFetchFork() {
      if (!this.currentUser) {
        await this.initCurrentUser();
      }

      if (!this.currentUser) {
        this.hasFork = false;
        return;
      }

      this.loadingState = "loading-fork";
      this.lastError = null;

      try {
        const result = await checkIfForked(
          PROJECT_CONFIG.UPSTREAM_OWNER,
          PROJECT_CONFIG.UPSTREAM_REPO,
          this.currentUser
        );

        this.hasFork = result.isForked;

        if (result.isForked && result.forkData) {
          this.forkRepo = {
            ...result.forkData,
            syncStatus: result.syncStatus,
          } as ForkInfo;
        } else {
          this.forkRepo = null;
        }
      } catch (error: any) {
        this.lastError = `检查Fork状态失败: ${error.message || error}`;
      } finally {
        this.loadingState = "idle";
      }
    },

    // 获取Fork仓库的分支列表
    async fetchForkBranches() {
      if (!this.currentUser || !this.hasFork) {
        this.forkBranches = [];
        return;
      }

      try {
        const branches = await listBranches(
          this.currentUser,
          PROJECT_CONFIG.UPSTREAM_REPO,
          { per_page: 30 }
        );
        this.forkBranches = branches;
      } catch (error: any) {
        console.error("获取Fork分支列表失败:", error);
        this.lastError = `获取Fork分支列表失败: ${error.message || error}`;
        this.forkBranches = [];
      }
    },

    // 获取Fork仓库的最近提交
    async fetchForkCommits() {
      if (!this.currentUser || !this.hasFork) {
        this.forkCommits = [];
        return;
      }

      try {
        const commits = await listCommits(
          this.currentUser,
          PROJECT_CONFIG.UPSTREAM_REPO,
          { per_page: 5 }
        );
        this.forkCommits = commits;
      } catch (error: any) {
        console.error("获取Fork提交列表失败:", error);
        this.lastError = `获取Fork提交列表失败: ${error.message || error}`;
        this.forkCommits = [];
      }
    },

    // Fork上游仓库
    async forkUpstream() {
      this.loadingState = "forking";
      this.lastError = null;

      try {
        const result = await forkRepository(
          PROJECT_CONFIG.UPSTREAM_OWNER,
          PROJECT_CONFIG.UPSTREAM_REPO
        );

        // Fork完成后重新加载Fork信息
        await this.checkAndFetchFork();

        return result;
      } catch (error: any) {
        this.lastError = `Fork仓库失败: ${error.message || error}`;
        throw error;
      } finally {
        this.loadingState = "idle";
      }
    },

    // 同步Fork仓库
    async syncForkRepo() {
      if (!this.currentUser || !this.forkRepo) {
        throw new Error("没有Fork仓库可同步");
      }

      this.loadingState = "syncing-fork";
      this.lastError = null;

      try {
        const defaultBranch = this.forkRepo.default_branch || "main";
        await syncFork(
          this.currentUser,
          PROJECT_CONFIG.UPSTREAM_REPO,
          defaultBranch
        );

        // 重新获取同步状态
        const syncStatus = await getForkSyncStatus(
          this.currentUser,
          PROJECT_CONFIG.UPSTREAM_REPO,
          PROJECT_CONFIG.UPSTREAM_OWNER,
          PROJECT_CONFIG.UPSTREAM_REPO,
          defaultBranch
        );

        if (this.forkRepo) {
          this.forkRepo.syncStatus = syncStatus as ForkSyncStatus;
        }

        // Fork 同步成功后，自动 fetch 本地仓库以更新远程跟踪分支
        if (this.localStatus?.exists && this.localStatus?.path) {
          try {
            await this.fetchLocalRepo();
          } catch (e) {
            console.warn("Fork 同步后本地 fetch 失败:", e);
          }
        }
      } catch (error: any) {
        this.lastError = `同步Fork失败: ${error.message || error}`;
        throw error;
      } finally {
        this.loadingState = "idle";
      }
    },

    // Fetch 本地仓库（更新远程跟踪分支）
    async fetchLocalRepo(): Promise<string | null> {
      if (!this.localStatus?.path || !this.forkRepo) {
        return null;
      }

      const { startGitFetch } = await import("../api/tasks");
      const cloneUrl =
        this.forkRepo.clone_url || this.forkRepo.html_url + ".git";
      return startGitFetch(cloneUrl, this.localStatus.path);
    },

    // 强制同步Fork仓库（丢弃所有本地变更）
    async forceSyncForkRepo() {
      if (!this.currentUser || !this.forkRepo) {
        throw new Error("没有Fork仓库可同步");
      }

      this.loadingState = "syncing-fork";
      this.lastError = null;

      try {
        const defaultBranch = this.forkRepo.default_branch || "main";
        await forceSyncFork(
          this.currentUser,
          PROJECT_CONFIG.UPSTREAM_REPO,
          PROJECT_CONFIG.UPSTREAM_OWNER,
          PROJECT_CONFIG.UPSTREAM_REPO,
          defaultBranch
        );

        // 重新获取同步状态
        const syncStatus = await getForkSyncStatus(
          this.currentUser,
          PROJECT_CONFIG.UPSTREAM_REPO,
          PROJECT_CONFIG.UPSTREAM_OWNER,
          PROJECT_CONFIG.UPSTREAM_REPO,
          defaultBranch
        );

        if (this.forkRepo) {
          this.forkRepo.syncStatus = syncStatus as ForkSyncStatus;
        }

        // 重新获取commits
        await this.fetchForkCommits();
      } catch (error: any) {
        this.lastError = `强制同步Fork失败: ${error.message || error}`;
        throw error;
      } finally {
        this.loadingState = "idle";
      }
    },

    // 加载PR列表
    async fetchPullRequests() {
      this.loadingState = "loading-prs";

      try {
        // 获取上游仓库的PR列表
        const prs = await listPullRequests(
          PROJECT_CONFIG.UPSTREAM_OWNER,
          PROJECT_CONFIG.UPSTREAM_REPO,
          { state: "open", per_page: 30 }
        );

        this.pullRequests = prs as PullRequestInfo[];

        // 过滤出当前用户创建的PR
        if (this.currentUser) {
          this.myPullRequests = this.pullRequests.filter(
            (pr) => pr.user.login === this.currentUser
          );
        }
      } catch (error: any) {
        console.error("加载PR列表失败:", error);
        this.lastError = `加载PR列表失败: ${error.message || error}`;
      } finally {
        this.loadingState = "idle";
      }
    },

    // 加载附加信息（贡献者、语言、Release等）
    async fetchAdditionalInfo() {
      try {
        const [contributors, languages, release, branches] =
          await Promise.allSettled([
            listContributors(
              PROJECT_CONFIG.UPSTREAM_OWNER,
              PROJECT_CONFIG.UPSTREAM_REPO,
              { per_page: 10 }
            ),
            getLanguages(
              PROJECT_CONFIG.UPSTREAM_OWNER,
              PROJECT_CONFIG.UPSTREAM_REPO
            ),
            getLatestRelease(
              PROJECT_CONFIG.UPSTREAM_OWNER,
              PROJECT_CONFIG.UPSTREAM_REPO
            ),
            listBranches(
              PROJECT_CONFIG.UPSTREAM_OWNER,
              PROJECT_CONFIG.UPSTREAM_REPO,
              { per_page: 30 }
            ),
          ]);

        if (contributors.status === "fulfilled") {
          this.contributors = contributors.value as ContributorInfo[];
        }
        if (languages.status === "fulfilled") {
          this.languages = languages.value;
        }
        if (release.status === "fulfilled") {
          this.latestRelease = release.value as ReleaseInfo | null;
        }
        if (branches.status === "fulfilled") {
          this.branches = branches.value as BranchInfo[];
        }
      } catch (error) {
        console.error("加载附加信息失败:", error);
      }
    },

    // 检查本地仓库状态
    async checkLocalRepo() {
      this.loadingState = "loading-local";

      try {
        const dataDir = await appDataDir();
        const repoPath = `${dataDir}/repository`;
        const repoExists = await exists(repoPath);

        if (repoExists) {
          // 调用后端获取实际仓库状态和worktree列表
          const { getGitRepoStatus, getWorktrees } = await import(
            "../api/tasks"
          );
          const [status, wtList] = await Promise.all([
            getGitRepoStatus(repoPath),
            getWorktrees(repoPath),
          ]);

          // 转换worktree列表格式
          const worktrees: import("../types/project").WorktreeInfo[] =
            wtList.map((wt) => ({
              path: wt.path,
              branch: wt.branch || "(detached)",
              isMainWorktree: wt.isMain,
              head: wt.head,
              trackingBranch: wt.trackingBranch,
              ahead: wt.ahead,
              behind: wt.behind,
              locked: wt.locked,
              prunable: wt.prunable,
              isDetached: wt.isDetached,
            }));

          this.localStatus = {
            exists: true,
            path: repoPath,
            currentBranch: status.currentBranch,
            workingTreeClean: status.isClean,
            staged: status.staged,
            unstaged: status.unstaged,
            untracked: status.untracked,
            ahead: status.ahead,
            behind: status.behind,
            trackingBranch: status.trackingBranch,
            worktrees,
          };
        } else {
          this.localStatus = {
            exists: false,
            path: null,
            currentBranch: null,
            workingTreeClean: true,
            staged: 0,
            unstaged: 0,
            untracked: 0,
            ahead: 0,
            behind: 0,
            trackingBranch: null,
            worktrees: [],
          };
        }
      } catch (error: any) {
        console.error("检查本地仓库失败:", error);
        this.lastError = `检查本地仓库失败: ${error.message || error}`;
        this.localStatus = {
          exists: false,
          path: null,
          currentBranch: null,
          workingTreeClean: true,
          staged: 0,
          unstaged: 0,
          untracked: 0,
          ahead: 0,
          behind: 0,
          trackingBranch: null,
          worktrees: [],
        };
      } finally {
        this.loadingState = "idle";
      }
    },

    // 获取Clone目标路径
    async getClonePath(): Promise<string> {
      const dataDir = await appDataDir();
      // 确保目录存在
      const repoDir = `${dataDir}/repository`;
      try {
        if (!(await exists(dataDir))) {
          await mkdir(dataDir, { recursive: true });
        }
      } catch (e) {
        // 目录可能已存在
      }
      return repoDir;
    },

    // 创建PR
    async createPR(data: {
      title: string;
      body?: string;
      head: string;
      base: string;
      draft?: boolean;
    }) {
      if (!this.currentUser) {
        throw new Error("请先登录");
      }

      try {
        const headRef = `${this.currentUser}:${data.head}`;
        const result = await createPullRequest(
          PROJECT_CONFIG.UPSTREAM_OWNER,
          PROJECT_CONFIG.UPSTREAM_REPO,
          {
            ...data,
            head: headRef,
          }
        );

        // 刷新PR列表
        await this.fetchPullRequests();

        return result;
      } catch (error: any) {
        this.lastError = `创建PR失败: ${error.message || error}`;
        throw error;
      }
    },

    // 加载所有项目数据
    async loadAllData() {
      await this.initCurrentUser();

      await Promise.all([
        this.fetchUpstreamRepo(),
        this.checkAndFetchFork(),
        this.checkLocalRepo(),
      ]);

      // 并行加载附加信息
      await Promise.all([
        this.fetchPullRequests(),
        this.fetchAdditionalInfo(),
        this.fetchForkBranches(),
        this.fetchForkCommits(),
      ]);
    },

    // 刷新所有数据
    async refresh() {
      this.lastError = null;
      await this.loadAllData();
    },
  },
});
