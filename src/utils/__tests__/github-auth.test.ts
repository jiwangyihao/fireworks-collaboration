import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";

vi.mock("../../api/tauri-fetch", () => ({
  fetch: vi.fn(),
}));
vi.mock("@tauri-apps/plugin-opener", () => ({
  openPath: vi.fn(),
}));

import { fetch as tauriFetch } from "../../api/tauri-fetch";
import { openPath } from "@tauri-apps/plugin-opener";
import {
  generateAuthUrl,
  exchangeCodeForToken,
  saveAccessToken,
  loadAccessToken,
  validateToken,
  getUserInfo,
  removeAccessToken,
  startOAuthFlow,
} from "../github-auth";

function mockResponse({
  ok,
  status = 200,
  statusText = "OK",
  body,
}: {
  ok: boolean;
  status?: number;
  statusText?: string;
  body: any;
}) {
  return {
    ok,
    status,
    statusText,
    text: async () => (typeof body === "string" ? body : JSON.stringify(body)),
    json: async () => (typeof body === "string" ? JSON.parse(body) : body),
  } as any;
}

describe("utils/github-auth", () => {
  beforeEach(() => {
    (tauriFetch as any).mockReset();
    (openPath as any).mockReset();
    localStorage.clear();
  });
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("generateAuthUrl: 生成包含必要参数的 URL", () => {
    const { url, codeVerifier, state } = generateAuthUrl(3429);
    expect(url).toContain("client_id=");
    expect(url).toContain("code_challenge_method=S256");
    expect(url).toContain("response_type=code");
    expect(url).toMatch(/localhost%3A3429|localhost:3429/);
    expect(codeVerifier.length).toBeGreaterThanOrEqual(64);
    expect(state.length).toBeGreaterThanOrEqual(16);
  });

  it("exchangeCodeForToken: 成功返回 access_token", async () => {
    (tauriFetch as any).mockResolvedValueOnce(
      mockResponse({ ok: true, body: { access_token: "tok" } })
    );
    const token = await exchangeCodeForToken("code", "ver", 3429);
    expect(token).toBe("tok");
  });

  it("exchangeCodeForToken: 非200应抛错并包含状态码", async () => {
    (tauriFetch as any).mockResolvedValueOnce(
      mockResponse({
        ok: false,
        status: 400,
        statusText: "Bad Request",
        body: "boom",
      })
    );
    await expect(exchangeCodeForToken("c", "v", 3429)).rejects.toThrow(/400/);
  });

  it("exchangeCodeForToken: 非法 JSON 抛解析错误", async () => {
    (tauriFetch as any).mockResolvedValueOnce(
      mockResponse({ ok: true, body: "{not-json}" })
    );
    await expect(exchangeCodeForToken("c", "v", 3429)).rejects.toThrow(
      /解析 GitHub OAuth 响应失败/
    );
  });

  it("exchangeCodeForToken: 响应包含 error 字段抛错", async () => {
    (tauriFetch as any).mockResolvedValueOnce(
      mockResponse({
        ok: true,
        body: { error: "invalid_grant", error_description: "bad" },
      })
    );
    await expect(exchangeCodeForToken("c", "v", 3429)).rejects.toThrow(
      /GitHub OAuth 错误/
    );
  });

  it("exchangeCodeForToken: 缺少 access_token 抛错", async () => {
    (tauriFetch as any).mockResolvedValueOnce(
      mockResponse({ ok: true, body: {} })
    );
    await expect(exchangeCodeForToken("c", "v", 3429)).rejects.toThrow(
      /缺少访问令牌/
    );
  });

  it("token 存取与移除: save/load/remove/trim", async () => {
    await saveAccessToken(" abc ");
    expect(await loadAccessToken()).toBe("abc");
    await removeAccessToken();
    expect(await loadAccessToken()).toBeNull();
  });

  it("validateToken: 根据响应 ok 返回布尔值", async () => {
    (tauriFetch as any).mockResolvedValueOnce(
      mockResponse({ ok: true, body: {} })
    );
    expect(await validateToken("t")).toBe(true);
    (tauriFetch as any).mockResolvedValueOnce(
      mockResponse({ ok: false, body: {} })
    );
    expect(await validateToken("t")).toBe(false);
  });

  it("getUserInfo: 成功返回用户信息", async () => {
    (tauriFetch as any).mockResolvedValueOnce(
      mockResponse({ ok: true, body: { login: "u" } })
    );
    const info = await getUserInfo("t");
    expect(info.login).toBe("u");
  });

  it("getUserInfo: 非200抛错", async () => {
    (tauriFetch as any).mockResolvedValueOnce(
      mockResponse({ ok: false, body: {} })
    );
    await expect(getUserInfo("t")).rejects.toThrow(/获取用户信息失败/);
  });

  it("startOAuthFlow: 打开授权地址并返回 verifier 与 state", async () => {
    (openPath as any).mockResolvedValueOnce(undefined);
    const { codeVerifier, state, port } = await startOAuthFlow(3429);
    expect((openPath as any).mock.calls[0][0]).toContain(
      "https://github.com/login/oauth/authorize"
    );
    expect((openPath as any).mock.calls[0][0]).toMatch(
      /localhost%3A3429|localhost:3429/
    );
    expect(codeVerifier.length).toBeGreaterThan(10);
    expect(state.length).toBeGreaterThan(10);
    expect(port).toBe(3429);
  });
});
