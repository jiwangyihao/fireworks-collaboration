import { fetch as tauriFetch } from "../api/tauri-fetch";
import { loadAccessToken } from "./github-auth";

// GitHub API 基础配置
const GITHUB_API_BASE = "https://api.github.com";

// 获取认证头
async function getAuthHeaders(): Promise<HeadersInit> {
  const token = await loadAccessToken();
  if (!token) {
    throw new Error("未找到访问令牌，请先登录");
  }

  return {
    Authorization: `Bearer ${token}`,
    Accept: "application/vnd.github.v3+json",
    "Content-Type": "application/json",
  };
}

// Fork 仓库
export async function forkRepository(
  owner: string,
  repo: string,
): Promise<any> {
  try {
    const headers = await getAuthHeaders();
  const response = await tauriFetch(
      `${GITHUB_API_BASE}/repos/${owner}/${repo}/forks`,
      {
        method: "POST",
        headers,
      },
    );

    if (!response.ok) {
      const errorData = await response.json();
      throw new Error(
        `Fork 仓库失败: ${errorData.message || response.statusText}`,
      );
    }

    return await response.json();
  } catch (error) {
    throw new Error(`Fork 仓库失败: ${error}`);
  }
}

// 创建 Pull Request
export async function createPullRequest(
  owner: string,
  repo: string,
  data: {
    title: string;
    body?: string;
    head: string; // 源分支，格式: "username:branch"
    base: string; // 目标分支，通常是 "main" 或 "master"
    draft?: boolean;
  },
): Promise<any> {
  try {
    const headers = await getAuthHeaders();
  const response = await tauriFetch(
      `${GITHUB_API_BASE}/repos/${owner}/${repo}/pulls`,
      {
        method: "POST",
        headers,
        body: JSON.stringify(data),
      },
    );

    if (!response.ok) {
      const errorData = await response.json();
      throw new Error(
        `创建 PR 失败: ${errorData.message || response.statusText}`,
      );
    }

    return await response.json();
  } catch (error) {
    throw new Error(`创建 PR 失败: ${error}`);
  }
}

// 获取用户的SSH密钥列表
export async function listSSHKeys(): Promise<any[]> {
  try {
    const headers = await getAuthHeaders();
  const response = await tauriFetch(`${GITHUB_API_BASE}/user/keys`, {
      headers,
    });

    if (!response.ok) {
      const errorData = await response.json();
      throw new Error(
        `获取SSH密钥失败: ${errorData.message || response.statusText}`,
      );
    }

    return await response.json();
  } catch (error) {
    throw new Error(`获取SSH密钥失败: ${error}`);
  }
}

// 添加SSH密钥
export async function addSSHKey(title: string, key: string): Promise<any> {
  try {
    const headers = await getAuthHeaders();
  const response = await tauriFetch(`${GITHUB_API_BASE}/user/keys`, {
      method: "POST",
      headers,
      body: JSON.stringify({
        title,
        key,
      }),
    });

    if (!response.ok) {
      const errorData = await response.json();
      throw new Error(
        `添加SSH密钥失败: ${errorData.message || response.statusText}`,
      );
    }

    return await response.json();
  } catch (error) {
    throw new Error(`添加SSH密钥失败: ${error}`);
  }
}

// 删除SSH密钥
export async function deleteSSHKey(keyId: number): Promise<void> {
  try {
    const headers = await getAuthHeaders();
  const response = await tauriFetch(`${GITHUB_API_BASE}/user/keys/${keyId}`, {
      method: "DELETE",
      headers,
    });

    if (!response.ok) {
      const errorData = await response.json();
      throw new Error(
        `删除SSH密钥失败: ${errorData.message || response.statusText}`,
      );
    }
  } catch (error) {
    throw new Error(`删除SSH密钥失败: ${error}`);
  }
}

// 获取仓库信息
export async function getRepository(owner: string, repo: string): Promise<any> {
  try {
    const headers = await getAuthHeaders();
  const response = await tauriFetch(`${GITHUB_API_BASE}/repos/${owner}/${repo}`, {
      headers,
    });

    if (!response.ok) {
      const errorData = await response.json();
      throw new Error(
        `获取仓库信息失败: ${errorData.message || response.statusText}`,
      );
    }

    return await response.json();
  } catch (error) {
    throw new Error(`获取仓库信息失败: ${error}`);
  }
}

// 检查是否已经fork了仓库并获取同步状态
export async function checkIfForked(
  owner: string,
  repo: string,
  username: string,
): Promise<{
  isForked: boolean;
  syncStatus?: {
    aheadBy: number;
    behindBy: number;
    isSynced: boolean;
  };
  forkData?: any;
}> {
  try {
    const headers = await getAuthHeaders();
  const response = await tauriFetch(
      `${GITHUB_API_BASE}/repos/${username}/${repo}`,
      {
        headers,
      },
    );

    if (response.ok) {
      const repoData = await response.json();
      const isForked =
        repoData.fork && repoData.parent?.full_name === `${owner}/${repo}`;

      if (isForked) {
        // 获取同步状态
        const syncStatus = await getForkSyncStatus(username, repo, owner, repo);
        return {
          isForked: true,
          syncStatus,
          forkData: repoData,
        };
      }
    }

    return { isForked: false };
  } catch (error) {
    console.error("检查fork状态失败:", error);
    return { isForked: false };
  }
}

// 获取Fork与上游的同步状态
export async function getForkSyncStatus(
  forkOwner: string,
  forkRepo: string,
  upstreamOwner: string,
  upstreamRepo: string,
  baseBranch: string = "main",
): Promise<{
  aheadBy: number;
  behindBy: number;
  isSynced: boolean;
}> {
  try {
    const headers = await getAuthHeaders();

    // 比较Fork和上游仓库的主分支
  const response = await tauriFetch(
      `${GITHUB_API_BASE}/repos/${upstreamOwner}/${upstreamRepo}/compare/${baseBranch}...${forkOwner}:${baseBranch}`,
      {
        headers,
      },
    );

    if (!response.ok) {
      // 如果主分支不存在，尝试master分支
      if (baseBranch === "main") {
        return await getForkSyncStatus(
          forkOwner,
          forkRepo,
          upstreamOwner,
          upstreamRepo,
          "master",
        );
      }
      throw new Error(`获取同步状态失败: ${response.statusText}`);
    }

    const compareData = await response.json();

    return {
      aheadBy: compareData.ahead_by || 0,
      behindBy: compareData.behind_by || 0,
      isSynced:
        (compareData.ahead_by || 0) === 0 && (compareData.behind_by || 0) === 0,
    };
  } catch (error) {
    throw new Error(`获取同步状态失败: ${error}`);
  }
}

// 同步Fork仓库
export async function syncFork(
  forkOwner: string,
  forkRepo: string,
  branch: string = "main",
): Promise<any> {
  try {
    const headers = await getAuthHeaders();

    // 使用GitHub的Fork同步API
  const response = await tauriFetch(
      `${GITHUB_API_BASE}/repos/${forkOwner}/${forkRepo}/merge-upstream`,
      {
        method: "POST",
        headers,
        body: JSON.stringify({
          branch: branch,
        }),
      },
    );

    if (!response.ok) {
      const errorData = await response.json();
      // 如果是主分支不存在，尝试master分支
      if (branch === "main" && errorData.message?.includes("branch")) {
        return await syncFork(forkOwner, forkRepo, "master");
      }
      throw new Error(
        `同步Fork失败: ${errorData.message || response.statusText}`,
      );
    }

    return await response.json();
  } catch (error) {
    throw new Error(`同步Fork失败: ${error}`);
  }
}

// 获取Fork仓库的默认分支
export async function getForkDefaultBranch(
  forkOwner: string,
  forkRepo: string,
): Promise<string> {
  try {
    const repoData = await getRepository(forkOwner, forkRepo);
    return repoData.default_branch || "main";
  } catch (error) {
    throw new Error(`获取默认分支失败: ${error}`);
  }
}

// 检查Fork是否可以同步（是否有权限）
export async function canSyncFork(
  forkOwner: string,
  forkRepo: string,
): Promise<boolean> {
  try {
    const headers = await getAuthHeaders();
  const response = await tauriFetch(
      `${GITHUB_API_BASE}/repos/${forkOwner}/${forkRepo}`,
      {
        headers,
      },
    );

    if (response.ok) {
      const repoData = await response.json();
      // 检查是否是Fork且用户有推送权限
      return repoData.fork && repoData.permissions?.push === true;
    }

    return false;
  } catch (error) {
    console.error("检查同步权限失败:", error);
    return false;
  }
}

// 获取用户的仓库列表
export async function listUserRepositories(username?: string): Promise<any[]> {
  try {
    const headers = await getAuthHeaders();
    const url = username
      ? `${GITHUB_API_BASE}/users/${username}/repos`
      : `${GITHUB_API_BASE}/user/repos`;

  const response = await tauriFetch(url, {
      headers,
    });

    if (!response.ok) {
      const errorData = await response.json();
      throw new Error(
        `获取仓库列表失败: ${errorData.message || response.statusText}`,
      );
    }

    return await response.json();
  } catch (error) {
    throw new Error(`获取仓库列表失败: ${error}`);
  }
}

// 创建分支
export async function createBranch(
  owner: string,
  repo: string,
  branchName: string,
  fromSha: string,
): Promise<any> {
  try {
    const headers = await getAuthHeaders();
  const response = await tauriFetch(
      `${GITHUB_API_BASE}/repos/${owner}/${repo}/git/refs`,
      {
        method: "POST",
        headers,
        body: JSON.stringify({
          ref: `refs/heads/${branchName}`,
          sha: fromSha,
        }),
      },
    );

    if (!response.ok) {
      const errorData = await response.json();
      throw new Error(
        `创建分支失败: ${errorData.message || response.statusText}`,
      );
    }

    return await response.json();
  } catch (error) {
    throw new Error(`创建分支失败: ${error}`);
  }
}

// 获取分支信息
export async function getBranch(
  owner: string,
  repo: string,
  branch: string,
): Promise<any> {
  try {
    const headers = await getAuthHeaders();
  const response = await tauriFetch(
      `${GITHUB_API_BASE}/repos/${owner}/${repo}/branches/${branch}`,
      {
        headers,
      },
    );

    if (!response.ok) {
      const errorData = await response.json();
      throw new Error(
        `获取分支信息失败: ${errorData.message || response.statusText}`,
      );
    }

    return await response.json();
  } catch (error) {
    throw new Error(`获取分支信息失败: ${error}`);
  }
}

// 获取文件内容
export async function getFileContent(
  owner: string,
  repo: string,
  path: string,
  ref?: string,
): Promise<any> {
  try {
    const headers = await getAuthHeaders();
    const url = new URL(
      `${GITHUB_API_BASE}/repos/${owner}/${repo}/contents/${path}`,
    );
    if (ref) {
      url.searchParams.set("ref", ref);
    }

  const response = await tauriFetch(url.toString(), {
      headers,
    });

    if (!response.ok) {
      const errorData = await response.json();
      throw new Error(
        `获取文件内容失败: ${errorData.message || response.statusText}`,
      );
    }

    return await response.json();
  } catch (error) {
    throw new Error(`获取文件内容失败: ${error}`);
  }
}

// 创建或更新文件
export async function createOrUpdateFile(
  owner: string,
  repo: string,
  path: string,
  data: {
    message: string;
    content: string; // Base64 编码的内容
    sha?: string; // 如果是更新文件，需要提供现有文件的 SHA
    branch?: string;
  },
): Promise<any> {
  try {
    const headers = await getAuthHeaders();
  const response = await tauriFetch(
      `${GITHUB_API_BASE}/repos/${owner}/${repo}/contents/${path}`,
      {
        method: "PUT",
        headers,
        body: JSON.stringify(data),
      },
    );

    if (!response.ok) {
      const errorData = await response.json();
      throw new Error(
        `创建/更新文件失败: ${errorData.message || response.statusText}`,
      );
    }

    return await response.json();
  } catch (error) {
    throw new Error(`创建/更新文件失败: ${error}`);
  }
}
