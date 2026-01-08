import { beforeEach, describe, expect, it, vi } from "vitest";

const { mockInvoke } = vi.hoisted(() => ({
  mockInvoke: vi.fn(),
}));

vi.mock("../tauri", () => ({
  invoke: mockInvoke,
}));

import { fetch as tauriFetch } from "../tauri-fetch";
import type { HttpResponseOutput } from "../http";

function createResponse(
  overrides: Partial<HttpResponseOutput>
): HttpResponseOutput {
  return {
    ok: true,
    status: 200,
    headers: {
      "content-type": "application/json",
    },
    bodyBase64: "e30=",
    usedFakeSni: false,
    ip: null,
    timing: {
      connectMs: 1,
      tlsMs: 1,
      firstByteMs: 1,
      totalMs: 1,
    },
    redirects: [],
    bodySize: 2,
    ...overrides,
  };
}

function toBase64(value: string): string {
  const BufferCtor = (
    globalThis as unknown as {
      Buffer?: {
        from: (...args: any[]) => { toString: (encoding: string) => string };
      };
    }
  ).Buffer;
  if (BufferCtor) {
    return BufferCtor.from(value, "utf8").toString("base64");
  }
  const encoder = new TextEncoder();
  const bytes = encoder.encode(value);
  let binary = "";
  for (const byte of bytes) {
    binary += String.fromCharCode(byte);
  }
  return btoa(binary);
}

describe("tauriFetch", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
  });

  it("serializes request payloads and decodes json responses", async () => {
    const payload = { ok: true };
    mockInvoke.mockResolvedValueOnce(
      createResponse({ bodyBase64: toBase64(JSON.stringify(payload)) })
    );

    const response = await tauriFetch("https://example.com/data", {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
      },
      body: JSON.stringify({ message: "hello" }),
      timeoutMs: 5_000,
      maxRedirects: 4,
    });

    expect(response.status).toBe(200);
    expect(response.ok).toBe(true);
    expect(response.redirected).toBe(false);
    expect(await response.json()).toEqual(payload);

    const args = mockInvoke.mock.calls[0][1] as {
      input: { [key: string]: unknown };
    };
    expect(args.input.method).toBe("POST");
    expect(args.input.followRedirects).toBe(true);
    expect(args.input.timeoutMs).toBe(5_000);
    expect(args.input.maxRedirects).toBe(4);
    expect(args.input.bodyBase64).toBe(
      toBase64(JSON.stringify({ message: "hello" }))
    );
    expect((args.input.headers as Record<string, string>)["user-agent"]).toBe(
      "fireworks-collaboration/tauri-fetch"
    );
  });

  it("honors redirect=manual by disabling followRedirects", async () => {
    mockInvoke.mockResolvedValueOnce(createResponse({}));
    await tauriFetch("https://example.com/", { redirect: "manual" });
    const args = mockInvoke.mock.calls[0][1] as {
      input: { followRedirects: boolean };
    };
    expect(args.input.followRedirects).toBe(false);
  });

  it("throws when redirect mode is error and backend returns redirect", async () => {
    mockInvoke.mockResolvedValueOnce(
      createResponse({
        status: 302,
        redirects: [
          { status: 302, location: "https://example.com/next", count: 1 },
        ],
      })
    );
    await expect(
      tauriFetch("https://example.com/", { redirect: "error" })
    ).rejects.toThrow(/redirect was blocked/);
  });

  it("marks responses as redirected when backend followed redirects", async () => {
    mockInvoke.mockResolvedValueOnce(
      createResponse({
        redirects: [
          { status: 301, location: "https://example.com/final", count: 1 },
        ],
      })
    );
    const response = await tauriFetch("https://example.com/start", {});
    expect(response.redirected).toBe(true);
    expect(response.url).toBe("https://example.com/final");
  });

  it("aborts immediately when signal is already aborted", async () => {
    const controller = new AbortController();
    controller.abort();
    // jsdom 和 happy-dom 对 AbortSignal 的实现不同，需要兼容两种错误消息
    await expect(
      tauriFetch("https://example.com/", { signal: controller.signal })
    ).rejects.toThrow(/操作已中止|abort|AbortSignal/i);
  });
});
