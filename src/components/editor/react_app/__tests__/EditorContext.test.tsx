/**
 * EditorContext.test.tsx - EditorContext 逻辑测试
 *
 * 测试 Global Store Hack 模式下的上下文同步机制
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, act } from "@testing-library/react";
import {
  updateGlobalContext,
  useGlobalEditorContext,
  EditorContext,
} from "../EditorContext";
import React from "react";

describe("EditorContext Global Store", () => {
  // 重置 globalContext 状态
  beforeEach(() => {
    act(() => {
      updateGlobalContext({});
    });
  });

  it("updateGlobalContext 应通知监听器", () => {
    const listener = vi.fn();
    // 模拟内部监听机制
    const { result } = renderHook(() => useGlobalEditorContext());

    // 触发更新
    act(() => {
      updateGlobalContext({ filePath: "/test.md" });
    });

    expect(result.current.filePath).toBe("/test.md");
  });

  it("useGlobalEditorContext 应优先使用 React Context", () => {
    const wrapper = ({ children }: { children: React.ReactNode }) => (
      <EditorContext.Provider value={{ filePath: "/context.md" }}>
        {children}
      </EditorContext.Provider>
    );

    const { result } = renderHook(() => useGlobalEditorContext(), { wrapper });
    expect(result.current.filePath).toBe("/context.md");
  });

  it("global store 更新应覆盖初始状态", () => {
    const { result } = renderHook(() => useGlobalEditorContext());

    act(() => {
      updateGlobalContext({ projectRoot: "/root" });
    });

    expect(result.current.projectRoot).toBe("/root");
  });

  it("多组件应同步更新", () => {
    const { result: hook1 } = renderHook(() => useGlobalEditorContext());
    const { result: hook2 } = renderHook(() => useGlobalEditorContext());

    act(() => {
      updateGlobalContext({ devServerPort: 3000 });
    });

    expect(hook1.current.devServerPort).toBe(3000);
    expect(hook2.current.devServerPort).toBe(3000);
  });
});
