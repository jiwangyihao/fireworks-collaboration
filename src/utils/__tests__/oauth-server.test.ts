import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

import { invoke } from "@tauri-apps/api/core";
import { createCallbackServer } from "../oauth-server";

describe("utils/oauth-server", () => {
  beforeEach(() => {
    (invoke as any).mockReset();
    vi.useFakeTimers();
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it("getCallbackData: 轮询两次后拿到数据，并在 close 时清理状态", async () => {
    let pollCount = 0;
    (invoke as any).mockImplementation((cmd: string) => {
      if (cmd === "start_oauth_server") return Promise.resolve();
      if (cmd === "clear_oauth_state") return Promise.resolve();
      if (cmd === "get_oauth_callback_data") {
        pollCount++;
        if (pollCount < 3) return Promise.resolve(null);
        return Promise.resolve({ code: "abc", state: "st" });
      }
      return Promise.resolve(undefined);
    });

    const { server, getCallbackData } = await createCallbackServer();

    const p = getCallbackData();
    await vi.advanceTimersByTimeAsync(1000);
    await vi.advanceTimersByTimeAsync(1000);
    await vi.advanceTimersByTimeAsync(1000);

    const data = await p;
    expect(data).toEqual({ code: "abc", state: "st" });

    await server.close();
    expect(invoke as any).toHaveBeenCalledWith("clear_oauth_state");
  });

  it("getCallbackData: 碰到 UTF-8 错误应立刻拒绝并停止轮询", async () => {
    (invoke as any).mockImplementation((cmd: string) => {
      if (cmd === "start_oauth_server") return Promise.resolve();
      if (cmd === "get_oauth_callback_data") {
        return Promise.reject(new Error("utf-8 invalid byte sequence"));
      }
      return Promise.resolve(undefined);
    });

    const { getCallbackData } = await createCallbackServer();
    const p = getCallbackData();
    // 立即挂载 catch，避免未处理拒绝告警
    const handled = p.catch((e) => e);

    await vi.advanceTimersByTimeAsync(1000);
    const err = await handled;
    expect(String(err)).toMatch(/UTF-8 编码错误/);
  });

  it("getCallbackData: 超时返回 timeout 错误描述", async () => {
    (invoke as any).mockImplementation((cmd: string) => {
      if (cmd === "start_oauth_server") return Promise.resolve();
      if (cmd === "get_oauth_callback_data") return Promise.resolve(null);
      return Promise.resolve(undefined);
    });

    const { getCallbackData } = await createCallbackServer();
    const p = getCallbackData();

    await vi.advanceTimersByTimeAsync(30000);
    const data = await p;
    expect(data).toEqual({
      error: "timeout",
      error_description: "授权超时，请重试",
    });
  });
});
