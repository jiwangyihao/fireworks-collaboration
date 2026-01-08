import { describe, it, expect, vi } from "vitest";
import { renderHook } from "@testing-library/react";
import { useFormattingActions } from "../useFormattingActions";

const mockEditor = {
  toggleStyles: vi.fn(),
  removeStyles: vi.fn(),
  createLink: vi.fn(),
  getSelectedLinkUrl: vi.fn(),
  insertInlineContent: vi.fn(),
  // Mock custom/extended methods which might be missing from core type definition
  nestBlock: vi.fn(),
  unnestBlock: vi.fn(),
  canNestBlock: vi.fn().mockReturnValue(true),
  // Add other properties if accessed by the hook
} as any;

describe("useFormattingActions", () => {
  it("should toggle styles", () => {
    const { result } = renderHook(() => useFormattingActions(mockEditor));

    result.current.toggleBold();
    expect(mockEditor.toggleStyles).toHaveBeenCalledWith({ bold: true });

    result.current.toggleItalic();
    expect(mockEditor.toggleStyles).toHaveBeenCalledWith({ italic: true });

    result.current.toggleCode();
    expect(mockEditor.toggleStyles).toHaveBeenCalledWith({ code: true });
  });

  it("should handle links", () => {
    const { result } = renderHook(() => useFormattingActions(mockEditor));

    mockEditor.getSelectedLinkUrl.mockReturnValue("http://example.com");
    expect(result.current.getSelectedLinkUrl()).toBe("http://example.com");

    result.current.createLink("http://test.com");
    expect(mockEditor.createLink).toHaveBeenCalledWith("http://test.com");

    result.current.removeLink();
    expect(mockEditor.removeStyles).toHaveBeenCalledWith({ link: true });
  });

  it("should insert math", () => {
    const { result } = renderHook(() => useFormattingActions(mockEditor));

    result.current.insertMath();
    expect(mockEditor.insertInlineContent).toHaveBeenCalledWith([
      {
        type: "inlineMath",
        props: { formula: "" },
      },
    ]);
  });

  it("should handle nesting", () => {
    const { result } = renderHook(() => useFormattingActions(mockEditor));

    result.current.nestBlock();
    expect(mockEditor.nestBlock).toHaveBeenCalled();

    result.current.unnestBlock();
    expect(mockEditor.unnestBlock).toHaveBeenCalled();
  });
});
