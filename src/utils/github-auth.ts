import CryptoJS from "crypto-js";
import { fetch as tauriFetch } from "../api/tauri-fetch";
import { openPath } from "@tauri-apps/plugin-opener";

const GITHUB_CLIENT_ID = "Ov23liuEyOOy0l1BNyyV";
const REDIRECT_URI = "http://localhost:3429/auth/callback";
const SCOPE = "repo user admin:public_key workflow";
const TOKEN_STORAGE_KEY = "github_access_token";

function generateRandomString(length: number): string {
  const chars =
    "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-._~";
  let result = "";
  for (let i = 0; i < length; i++) {
    result += chars.charAt(Math.floor(Math.random() * chars.length));
  }
  return result;
}

function generatePKCE() {
  const codeVerifier = generateRandomString(128);
  const codeChallenge = CryptoJS.SHA256(codeVerifier).toString(
    CryptoJS.enc.Base64url,
  );
  return { codeVerifier, codeChallenge };
}

export function generateAuthUrl(): {
  url: string;
  codeVerifier: string;
  state: string;
} {
  const { codeVerifier, codeChallenge } = generatePKCE();
  const state = generateRandomString(32);

  const params = new URLSearchParams({
    client_id: GITHUB_CLIENT_ID,
    redirect_uri: REDIRECT_URI,
    scope: SCOPE,
    state: state,
    code_challenge: codeChallenge,
    code_challenge_method: "S256",
    response_type: "code",
  });

  const url = `https://github.com/login/oauth/authorize?${params.toString()}`;

  return { url, codeVerifier, state };
}

export async function exchangeCodeForToken(
  code: string,
  codeVerifier: string,
): Promise<string> {
  try {
    const response = await tauriFetch(
      "https://github.com/login/oauth/access_token",
      {
        method: "POST",
        headers: {
          Accept: "application/json",
          "Content-Type": "application/json",
        },
        body: JSON.stringify({
          client_id: GITHUB_CLIENT_ID,
          client_secret: "fc39bef106d3ace7ec404f799f32818301ec4929",
          code: code,
          code_verifier: codeVerifier,
          redirect_uri: REDIRECT_URI,
        }),
      },
    );

    if (!response.ok) {
      const errorText = await response.text();
      throw new Error(
        `GitHub OAuth 请求失败: ${response.status} ${response.statusText} - ${errorText}`,
      );
    }

    const responseText = await response.text();
    let data;
    try {
      data = JSON.parse(responseText);
    } catch (parseError) {
      throw new Error(`解析 GitHub OAuth 响应失败: ${parseError}`);
    }

    if (data.error) {
      throw new Error(
        `GitHub OAuth 错误: ${data.error_description || data.error}`,
      );
    }

    if (!data.access_token) {
      throw new Error("GitHub OAuth 响应中缺少访问令牌");
    }

    return data.access_token;
  } catch (error) {
    throw error;
  }
}

export async function saveAccessToken(token: string): Promise<void> {
  try {
    localStorage.setItem(TOKEN_STORAGE_KEY, token);
  } catch (error) {
    throw new Error(`保存访问令牌失败: ${error}`);
  }
}

export async function loadAccessToken(): Promise<string | null> {
  try {
    const token = localStorage.getItem(TOKEN_STORAGE_KEY);
    return token ? token.trim() : null;
  } catch (error) {
    return null;
  }
}

export async function validateToken(token: string): Promise<boolean> {
  try {
    const response = await tauriFetch("https://api.github.com/user", {
      headers: {
        Authorization: `Bearer ${token}`,
        Accept: "application/vnd.github.v3+json",
      },
    });

    return response.ok;
  } catch (error) {
    return false;
  }
}

export interface UserInfo {
  login: string;
  id: number;
  avatar_url: string;
  html_url: string;
  name: string;
  company: string | null;
  blog: string;
  location: string | null;
  email: string | null;
  bio: string | null;
  public_repos: number;
  public_gists: number;
  followers: number;
  following: number;
  created_at: string;
  updated_at: string;
}

export async function getUserInfo(token: string): Promise<UserInfo> {
  try {
    const response = await tauriFetch("https://api.github.com/user", {
      headers: {
        Authorization: `Bearer ${token}`,
        Accept: "application/vnd.github.v3+json",
      },
    });

    if (!response.ok) {
      throw new Error("获取用户信息失败");
    }

    return await response.json();
  } catch (error) {
    throw new Error(`获取用户信息失败: ${error}`);
  }
}

export async function removeAccessToken(): Promise<void> {
  try {
    localStorage.removeItem(TOKEN_STORAGE_KEY);
  } catch (error) {
    // 静默处理错误
  }
}

export async function startOAuthFlow(): Promise<{
  codeVerifier: string;
  state: string;
}> {
  const { url, codeVerifier, state } = generateAuthUrl();
  await openPath(url);
  return { codeVerifier, state };
}
