import { describe, it, expect, vi, beforeEach } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

import { invoke } from "@tauri-apps/api/core";
import { startGitClone } from "../tasks";

describe("api/git clone 调用", () => {
  beforeEach(() => {
    (invoke as any).mockReset();
  });

  it("startGitClone 会调用 git_clone 并返回 taskId", async () => {
    (invoke as any).mockResolvedValueOnce("task-xyz");
    const id = await startGitClone("https://github.com/rust-lang/log", "C:/tmp/log");
    expect(invoke).toHaveBeenCalledWith("git_clone", {
      repo: "https://github.com/rust-lang/log",
      dest: "C:/tmp/log",
    });
    expect(id).toBe("task-xyz");
  });
});
