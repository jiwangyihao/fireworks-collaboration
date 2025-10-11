import { invoke } from "./tauri";
import { listen } from "@tauri-apps/api/event";
import { useTasksStore } from "../stores/tasks";
import { useLogsStore } from "../stores/logs";

export interface TaskStateEventPayload {
  taskId: string;
  kind: string;
  state: string;
  createdAt: number;
}
export interface TaskProgressEventPayload {
  taskId: string;
  kind: string;
  phase: string;
  percent: number;
  // P0.6 扩展：可选的 Git 进度指标（前端当前不消费，仅为兼容）
  objects?: number;
  bytes?: number;
  total_hint?: number;
}

let unsubs: (() => void)[] = [];

export async function initTaskEvents() {
  const store = useTasksStore();
  const logs = useLogsStore();
  // state events
  const un1 = await listen<TaskStateEventPayload>("task://state", (e) => {
    const p = e.payload;
    store.upsert({
      id: p.taskId,
      kind: (p.kind as any) ?? "Unknown",
      state: (p.state as any) ?? "pending",
      createdAt: p.createdAt ?? Date.now(),
    });
  });
  const un2 = await listen<TaskProgressEventPayload>("task://progress", (e) => {
    const p = e.payload;
    const totalHint = (p as any).total_hint ?? (p as any).totalHint;
    store.updateProgress({
      taskId: p.taskId,
      percent: p.percent ?? 0,
      phase: p.phase,
      objects: p.objects,
      bytes: p.bytes,
      total_hint: totalHint,
    });
  });
  // MP1.5: error events
  const un3 = await listen<{ taskId: string; kind: string; category: string; code?: string; message: string; retried_times?: number }>(
    "task://error",
    (e) => {
      const p = e.payload;
      const rt = (p as any).retried_times ?? (p as any).retriedTimes;
      store.setLastError(p.taskId, {
        category: p.category,
        message: p.message,
        retried_times: rt,
        // 同时提供 camelCase 便于内部归一
        // @ts-ignore allow extra field for compatibility
        retriedTimes: rt,
        code: p.code,
      });
      // 也写入全局日志，便于提示
      const prefix = p.kind ? `${p.kind}` : "Task";
      logs.push("error", `${prefix} ${p.category}: ${p.message}`);
    },
  );
  unsubs.push(un1, un2, un3);
}

export function disposeTaskEvents() {
  unsubs.forEach((u) => u());
  unsubs = [];
}

export async function listTasks() {
  return invoke<any[]>("task_list");
}

export async function startSleepTask(ms: number) {
  return invoke<string>("task_start_sleep", { ms });
}

export async function cancelTask(id: string) {
  return invoke<boolean>("task_cancel", { id });
}

// P0.6：启动 Git Clone 任务，返回 taskId
export interface StrategyOverride {
  http?: { followRedirects?: boolean; maxRedirects?: number };
  retry?: { max?: number; baseMs?: number; factor?: number; jitter?: boolean };
  // 未来可扩展其它子集（保持前端宽松，后端忽略未知并触发护栏事件）
}

export async function startGitClone(repo: string, dest: string, opts?: { depth?: number; filter?: string; strategyOverride?: StrategyOverride }) {
  const args: Record<string, unknown> = { repo, dest };
  if (opts) {
    if (opts.depth !== undefined) args.depth = opts.depth;
    if (opts.filter !== undefined) args.filter = opts.filter;
    if (opts.strategyOverride) args.strategy_override = opts.strategyOverride; // snake_case for backend
  }
  return invoke<string>("git_clone", args);
}

// P1.1：启动 Git Fetch 任务，返回 taskId
export async function startGitFetch(
  repo: string,
  dest: string,
  options?: { preset?: "remote" | "branches" | "branches+tags" | "tags"; depth?: number; filter?: string; strategyOverride?: StrategyOverride } |
    ("remote" | "branches" | "branches+tags" | "tags"),
) {
  const args: Record<string, unknown> = { repo, dest };
  // 兼容旧签名：第三参数若是字符串则视为 preset
  if (typeof options === "string") {
    if (options !== "remote") args.preset = options;
  } else if (options) {
    const { preset, depth, filter, strategyOverride } = options;
    if (preset && preset !== "remote") args.preset = preset;
    if (depth !== undefined) args.depth = depth;
    if (filter !== undefined) args.filter = filter;
    if (strategyOverride) args.strategy_override = strategyOverride;
  }
  return invoke<string>("git_fetch", args);
}

// MP1.1：启动 Git Push 任务，返回 taskId
// 参数：
// - dest: 本地仓库路径
// - remote: 远程名（默认 origin）
// - refspecs: 需要推送的 refspec 列表，例如 ["refs/heads/main:refs/heads/main"]；不传则使用默认
// - auth: 可选凭证；仅 token 时可使用 { username: "x-access-token", password: token }
// - useStoredCredential: 是否使用已存储的凭证（P6.4）
export async function startGitPush(params: {
  dest: string;
  remote?: string;
  refspecs?: string[];
  username?: string;
  password?: string;
  useStoredCredential?: boolean;
  strategyOverride?: StrategyOverride;
}) {
  const { dest, remote, refspecs, username, password, useStoredCredential, strategyOverride } = params;
  const args: Record<string, unknown> = { dest };
  if (remote) args.remote = remote;
  if (refspecs && refspecs.length > 0) args.refspecs = refspecs;
  if (username) args.username = username;
  if (password) args.password = password;
  if (useStoredCredential !== undefined) args.use_stored_credential = useStoredCredential; // snake_case for Tauri invoke
  if (strategyOverride) args.strategy_override = strategyOverride; // snake_case for Tauri invoke
  return invoke<string>("git_push", args);
}

// P2.1a: 启动 Git Init 任务
export async function startGitInit(dest: string) {
  return invoke<string>("git_init", { dest });
}

// P2.1a: 启动 Git Add 任务
export async function startGitAdd(dest: string, paths: string[]) {
  return invoke<string>("git_add", { dest, paths });
}

// P2.1b: 启动 Git Commit 任务
export async function startGitCommit(params: { dest: string; message: string; allowEmpty?: boolean; authorName?: string; authorEmail?: string }) {
  const { dest, message, allowEmpty, authorName, authorEmail } = params;
  const args: Record<string, unknown> = { dest, message };
  if (allowEmpty !== undefined) args.allow_empty = allowEmpty; // snake_case 兼容
  if (authorName) args.author_name = authorName;
  if (authorEmail) args.author_email = authorEmail;
  return invoke<string>("git_commit", args);
}

// P2.1c: 启动 Git Branch 任务
export async function startGitBranch(params: { dest: string; name: string; checkout?: boolean; force?: boolean }) {
  const { dest, name, checkout, force } = params;
  const args: Record<string, unknown> = { dest, name };
  if (checkout !== undefined) args.checkout = checkout;
  if (force !== undefined) args.force = force;
  return invoke<string>("git_branch", args);
}

// P2.1c: 启动 Git Checkout 任务
export async function startGitCheckout(params: { dest: string; reference: string; create?: boolean }) {
  const { dest, reference, create } = params;
  const args: Record<string, unknown> = { dest, reference };
  if (create !== undefined) args.create = create;
  return invoke<string>("git_checkout", args);
}

// P2.1d: 启动 Git Tag 任务
export async function startGitTag(params: { dest: string; name: string; message?: string; annotated?: boolean; force?: boolean }) {
  const { dest, name, message, annotated, force } = params;
  const args: Record<string, unknown> = { dest, name };
  if (message !== undefined) args.message = message;
  if (annotated !== undefined) args.annotated = annotated;
  if (force !== undefined) args.force = force;
  return invoke<string>("git_tag", args);
}

// P2.1d: 启动 Git Remote Set 任务
export async function startGitRemoteSet(params: { dest: string; name: string; url: string }) {
  const { dest, name, url } = params;
  return invoke<string>("git_remote_set", { dest, name, url });
}

// P2.1d: 启动 Git Remote Add 任务
export async function startGitRemoteAdd(params: { dest: string; name: string; url: string }) {
  const { dest, name, url } = params;
  return invoke<string>("git_remote_add", { dest, name, url });
}

// P2.1d: 启动 Git Remote Remove 任务
export async function startGitRemoteRemove(params: { dest: string; name: string }) {
  const { dest, name } = params;
  return invoke<string>("git_remote_remove", { dest, name });
}
