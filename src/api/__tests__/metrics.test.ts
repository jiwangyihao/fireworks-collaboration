import { beforeEach, describe, expect, it, vi } from "vitest";

const { mockFetch, mockInvoke } = vi.hoisted(() => ({
  mockFetch: vi.fn(),
  mockInvoke: vi.fn(),
}));

vi.mock("@tauri-apps/plugin-http", () => ({
  fetch: mockFetch,
}));

vi.mock("../tauri", () => ({
  invoke: mockInvoke,
}));

import { fetchMetricsSnapshot, type MetricsRange } from "../metrics";

function createResponse({
  ok,
  status,
  statusText,
  body,
}: {
  ok: boolean;
  status: number;
  statusText: string;
  body: string;
}) {
  return {
    ok,
    status,
    statusText,
    async text() {
      return body;
    },
  };
}

describe("fetchMetricsSnapshot", () => {
  beforeEach(() => {
    mockFetch.mockReset();
    mockInvoke.mockReset();
  });

  it("prefers the HTTP exporter when available", async () => {
    const snapshot = { generatedAtMs: 123, series: [] };
    mockFetch.mockResolvedValueOnce(
      createResponse({
        ok: true,
        status: 200,
        statusText: "OK",
        body: JSON.stringify(snapshot),
      }),
    );

    const result = await fetchMetricsSnapshot({
      names: ["tls_handshake_ms", "git_tasks_total"],
      range: "1h",
      quantiles: [0.5],
      maxSeries: 25,
    });

    expect(result).toEqual(snapshot);
    expect(mockInvoke).not.toHaveBeenCalled();
    expect(mockFetch).toHaveBeenCalledTimes(1);

    const [urlString] = mockFetch.mock.calls[0];
    const url = new URL(urlString);
    expect(url.pathname).toBe("/metrics/snapshot");
    expect(url.searchParams.get("names")).toBe("git_tasks_total,tls_handshake_ms");
    expect(url.searchParams.get("range")).toBe("1h");
    expect(url.searchParams.get("quantiles")).toBe("0.5");
    expect(url.searchParams.get("maxSeries")).toBe("25");
  });

  it("falls back to the Tauri command with sanitized payload", async () => {
    mockFetch.mockRejectedValueOnce(new Error("network down"));

    const fallback = { generatedAtMs: 456, series: [] };
    mockInvoke.mockResolvedValueOnce(fallback);

    const request = {
      names: [" tls_handshake_ms", "git_tasks_total", "git_tasks_total"],
      range: "24h" as MetricsRange,
      quantiles: [0.95, 0.5, 1.2, -1],
      maxSeries: 10,
    };

    const result = await fetchMetricsSnapshot(request);

    expect(result).toEqual(fallback);
    expect(mockFetch).toHaveBeenCalledTimes(1);
    expect(mockInvoke).toHaveBeenCalledTimes(1);

    const [command, args] = mockInvoke.mock.calls[0];
    expect(command).toBe("metrics_snapshot");
    const payload = (args as { options: Record<string, unknown> }).options;
    expect(payload.names).toEqual(["git_tasks_total", "tls_handshake_ms"]);
    expect(payload.range).toBe("24h");
    expect(payload.quantiles).toEqual([0.5, 0.95]);
    expect(payload.maxSeries).toBe(10);
  });
});
