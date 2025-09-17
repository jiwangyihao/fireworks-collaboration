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
export async function startGitClone(repo: string, dest: string) {
  return invoke<string>("git_clone", { repo, dest });
}

// P1.1：启动 Git Fetch 任务，返回 taskId
export async function startGitFetch(repo: string, dest: string, preset?: "remote" | "branches" | "branches+tags" | "tags") {
  const args: Record<string, unknown> = { repo, dest };
  if (preset && preset !== "remote") args.preset = preset;
  return invoke<string>("git_fetch", args);
}

// MP1.1：启动 Git Push 任务，返回 taskId
// 参数：
// - dest: 本地仓库路径
// - remote: 远程名（默认 origin）
// - refspecs: 需要推送的 refspec 列表，例如 ["refs/heads/main:refs/heads/main"]；不传则使用默认
// - auth: 可选凭证；仅 token 时可使用 { username: "x-access-token", password: token }
export async function startGitPush(params: {
  dest: string;
  remote?: string;
  refspecs?: string[];
  username?: string;
  password?: string;
}) {
  const { dest, remote, refspecs, username, password } = params;
  const args: Record<string, unknown> = { dest };
  if (remote) args.remote = remote;
  if (refspecs && refspecs.length > 0) args.refspecs = refspecs;
  if (username) args.username = username;
  if (password) args.password = password;
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
