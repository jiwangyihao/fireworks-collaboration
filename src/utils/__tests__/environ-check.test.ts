import { describe, it, expect, vi, beforeEach } from "vitest";

const { execMock, createMock } = vi.hoisted(() => {
  const exec = vi.fn();
  const create = vi.fn(() => ({ execute: exec }));
  return { execMock: exec, createMock: create };
});

vi.mock("@tauri-apps/plugin-shell", () => ({
  Command: { create: createMock },
}));

import { checkGit, checkNode, checkPnpm } from "../environ-check";

function collect<T>(gen: AsyncGenerator<T>): Promise<T[]> {
  return new Promise(async (resolve, reject) => {
    const out: T[] = [];
    try {
      for await (const v of gen) out.push(v);
      resolve(out);
    } catch (e) {
      reject(e);
    }
  });
}

describe("utils/environ-check", () => {
  beforeEach(() => {
    execMock.mockReset();
    createMock.mockClear();
  });

  it("checkGit: 版本符合 2.x 时成功", async () => {
    execMock.mockResolvedValueOnce({
      code: 0,
      stdout: "git version 2.45.2",
      stderr: "",
    });
    const res = await collect(checkGit());
    expect(createMock).toHaveBeenCalled();
    expect(res[0]).toMatchObject({ type: "success" });
    expect(String((res[0] as any).message)).toContain("Git 通过");
  });

  it("checkGit: 版本过低时报错", async () => {
    execMock.mockResolvedValueOnce({
      code: 0,
      stdout: "git version 1.9.0",
      stderr: "",
    });
    const res = await collect(checkGit());
    expect(res[0]).toMatchObject({ type: "error" });
  });

  it("checkNode: 版本 >= v24 成功，低于报错", async () => {
    execMock.mockResolvedValueOnce({ code: 0, stdout: "v24.1.0", stderr: "" });
    let res = await collect(checkNode());
    expect(res[0]).toMatchObject({ type: "success" });

    execMock.mockResolvedValueOnce({ code: 0, stdout: "v18.19.0", stderr: "" });
    res = await collect(checkNode());
    expect(res[0]).toMatchObject({ type: "error" });
  });

  it("checkPnpm: 主版本 >= 10 成功，低于报错", async () => {
    execMock.mockResolvedValueOnce({ code: 0, stdout: "10.15.0", stderr: "" });
    let res = await collect(checkPnpm());
    expect(res[0]).toMatchObject({ type: "success" });

    execMock.mockResolvedValueOnce({ code: 0, stdout: "7.28.0", stderr: "" });
    res = await collect(checkPnpm());
    expect(res[0]).toMatchObject({ type: "error" });
  });
});
