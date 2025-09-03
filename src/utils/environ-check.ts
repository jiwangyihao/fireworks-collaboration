import { Command } from "@tauri-apps/plugin-shell";

export interface Status {
  id: number;
  type: "success" | "warning" | "error";
  message: string;
}

export async function* checkGit(): AsyncGenerator<Partial<Status>> {
  try {
    let result = await Command.create("cmd", ["/c", "git", "-v"]).execute();
    if (!/git version 2\.\d+/.test(result.stdout)) {
      throw "Git 版本过低，请安装 Git 2.x 及以上版本";
    }
    yield {
      type: "success",
      message: `Git 通过，当前版本：v${result.stdout
        .replace(/git version/g, "")
        .trim()}`,
    };
  } catch (error) {
    yield {
      type: "error",
      message: `Git 未通过，请先安装 Git：${error}`,
    };
  }
}

export async function* checkNode(): AsyncGenerator<Partial<Status>> {
  try {
    let result = await Command.create("cmd", ["/c", "node", "-v"]).execute();
    if (parseInt(result.stdout.replace(/v/g, "").split(".")[0]) < 24) {
      throw `Node 版本过低，需求版本 24.x 及以上，当前版本：${result.stdout}`;
    }
    yield {
      type: "success",
      message: `Node 通过，当前版本：${result.stdout.trim()}`,
    };
  } catch (error) {
    yield {
      type: "error",
      message: `Node 未通过，请先安装 Node：${error}`,
    };
  }
}

export async function* checkPnpm(): AsyncGenerator<Partial<Status>> {
  try {
    let result = await Command.create("cmd", ["/c", "pnpm", "-v"]).execute();
    if (parseInt(result.stdout.split(".")[0]) < 10) {
      throw `pnpm 版本过低，需求版本 10.x 及以上，当前版本：v${result.stdout}`;
    }
    yield {
      type: "success",
      message: `pnpm 通过，当前版本：v${result.stdout.trim()}`,
    };
  } catch (error) {
    yield {
      type: "error",
      message: `pnpm 未通过，请先安装 pnpm：${error}`,
    };
  }
}
