import { Command } from "@tauri-apps/plugin-shell";

export interface Status {
  id: number;
  type: "success" | "warning" | "error";
  message: string;
}

export async function* checkGit(): AsyncGenerator<Partial<Status>> {
  try {
    // Git works fine with the shell plugin, no encoding issues
    let result = await Command.create("cmd", ["/c", "git -v"]).execute();
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

// try {
//   let result = await Command.create("cmd", ["/c", "git", "-v"]).execute();
//   if (!/git version 2\.\d+/.test(result.stdout)) {
//     throw "Git 版本过低，请安装 Git 2.x 及以上版本";
//   }
//   statusList.value[statusList.value.length - 1] = {
//     id: 1,
//     type: "success",
//     message: `Git 通过，当前版本：v${result.stdout
//       .replace(/git version/g, "")
//       .trim()}`,
//   };
// } catch (error) {
//   statusList.value[statusList.value.length - 1] = {
//     id: 1,
//     type: "error",
//     message: `Git 未通过，请先安装 Git：${error}`,
//   };
//   try {
//     statusList.value.push({
//       id: statusList.value.length + 1,
//       type: "warning",
//       message: "正在尝试自动安装 Git...",
//     });
//     // 下载 Git 安装包
//     const downloadResult = await Command.create("powershell", [
//       "-Command",
//       "Invoke-WebRequest -Uri https://github.com/git-for-windows/git/releases/download/v2.45.2.windows.1/Git-2.45.2-64-bit.exe -OutFile git-installer.exe",
//     ]).execute();
//
//     if (downloadResult.code !== 0) {
//       throw new Error(`下载 Git 安装包失败: ${downloadResult.stderr}`);
//     }
//
//     // 运行安装程序
//     const installResult = await Command.create("cmd", [
//       "/c",
//       "git-installer.exe",
//       "/SILENT",
//     ]).execute();
//
//     if (installResult.code !== 0) {
//       throw new Error(`安装 Git 失败: ${installResult.stderr}`);
//     }
//
//     statusList.value.push({
//       id: statusList.value.length + 1,
//       type: "success",
//       message: "Git 自动安装完成，请重新启动应用以使更改生效",
//     });
//   } catch (error) {
//     statusList.value[statusList.value.length - 1] = {
//       id: statusList.value.length,
//       type: "error",
//       message: `自动安装 Git 失败，请手动安装 Git：${error}`,
//     };
//   }
//   return;
// }

export async function* checkNode(): AsyncGenerator<Partial<Status>> {
  try {
    // Use Rust backend to handle encoding issues
    const { invoke } = await import("@tauri-apps/api/core");
    const stdout = await invoke<string>("check_tool_version", { tool: "node" });
    if (parseInt(stdout.replace(/v/g, "").split(".")[0]) < 24) {
      throw `Node 版本过低，需求版本 24.x 及以上，当前版本：${stdout}`;
    }
    yield {
      type: "success",
      message: `Node 通过，当前版本：${stdout}`,
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
    // Use Rust backend to handle encoding issues
    const { invoke } = await import("@tauri-apps/api/core");
    const stdout = await invoke<string>("check_tool_version", { tool: "pnpm" });
    if (parseInt(stdout.split(".")[0]) < 10) {
      throw `pnpm 版本过低，需求版本 10.x 及以上，当前版本：v${stdout}`;
    }
    yield {
      type: "success",
      message: `pnpm 通过，当前版本：v${stdout}`,
    };
  } catch (error) {
    yield {
      type: "error",
      message: `pnpm 未通过，请先安装 pnpm：${error}`,
    };
  }
}
