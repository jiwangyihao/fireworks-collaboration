import { invoke } from "@tauri-apps/api/core";

interface OAuthCallbackData {
  code?: string;
  state?: string;
  error?: string;
  error_description?: string;
}

// 创建 OAuth 回调服务器
export async function createCallbackServer(): Promise<{
  server: { close: () => void };
  getCallbackData: () => Promise<OAuthCallbackData>;
}> {
  // 启动 Tauri 后端的 OAuth 服务器
  await invoke("start_oauth_server");

  return {
    server: {
      close: () => {
        // 清除 OAuth 状态
        invoke("clear_oauth_state").catch(() => {});
      },
    },
    getCallbackData: async () => {
      // 轮询获取 OAuth 回调数据
      return new Promise<OAuthCallbackData>((resolve, reject) => {
        const pollInterval = setInterval(async () => {
          try {
            let data;
            try {
              data = await invoke<OAuthCallbackData | null>(
                "get_oauth_callback_data",
              );
            } catch (invokeError) {
              // 添加更详细的错误处理
              const errorString = invokeError?.toString() || "";
              if (
                errorString.includes("utf-8") ||
                errorString.includes("UTF-8")
              ) {
                clearInterval(pollInterval);
                reject(new Error(`UTF-8 编码错误: ${errorString}`));
                return;
              }

              // 如果不是 UTF-8 错误，重新抛出以便外层 catch 处理
              throw invokeError;
            }

            if (data) {
              clearInterval(pollInterval);
              resolve(data);
            }
          } catch (error) {
            // 如果是 UTF-8 错误，立即返回错误而不是继续轮询
            if (error && error.toString().includes("utf-8")) {
              clearInterval(pollInterval);
              reject(new Error(`UTF-8 编码错误: ${error}`));
              return;
            }
          }
        }, 1000); // 每秒检查一次

        // 30秒超时
        setTimeout(() => {
          clearInterval(pollInterval);
          resolve({
            error: "timeout",
            error_description: "授权超时，请重试",
          });
        }, 30000);
      });
    },
  };
}
