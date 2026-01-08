import { describe, it, expect, vi } from "vitest";
import { render } from "@testing-library/react";

// Heavily mock ALL external dependencies to isolate the component
vi.mock("@uiw/react-codemirror", () => ({
  __esModule: true,
  default: () => <div data-testid="codemirror-mock">CodeMirror Mock</div>,
}));

vi.mock("@blocknote/react", () => ({
  createReactBlockSpec: vi.fn(),
}));

vi.mock("@codemirror/view", () => ({
  EditorView: {
    theme: vi.fn(() => []),
    lineWrapping: [],
    updateListener: { of: vi.fn(() => []) },
  },
  Decoration: { line: vi.fn() },
  ViewPlugin: { fromClass: vi.fn(() => []) },
}));

vi.mock("@codemirror/state", () => ({
  RangeSetBuilder: class {
    add() {}
    finish() {
      return [];
    }
  },
  EditorState: { create: vi.fn(() => ({ languageDataAt: vi.fn(() => []) })) },
}));

vi.mock("@codemirror/lang-javascript", () => ({ javascript: vi.fn(() => []) }));
vi.mock("@codemirror/lang-html", () => ({ html: vi.fn(() => []) }));
vi.mock("@codemirror/lang-css", () => ({ css: vi.fn(() => []) }));
vi.mock("@codemirror/lang-json", () => ({ json: vi.fn(() => []) }));
vi.mock("@codemirror/lang-markdown", () => ({ markdown: vi.fn(() => []) }));
vi.mock("@codemirror/lang-python", () => ({ python: vi.fn(() => []) }));
vi.mock("@codemirror/lang-rust", () => ({ rust: vi.fn(() => []) }));
vi.mock("@uiw/codemirror-theme-github", () => ({ githubLight: [] }));

vi.mock("../../ContentRegistry", () => ({
  contentRegistry: {
    register: vi.fn(),
    registerExecutor: vi.fn(),
    unregisterExecutors: vi.fn(),
    notify: vi.fn(),
    focusBlock: vi.fn(),
  },
  iconify: vi.fn(() => null),
}));

vi.mock("@iconify/react", () => ({
  Icon: () => null,
}));

vi.mock("../../menu", () => ({
  DropdownMenu: () => null,
  MenuItem: () => null,
  BaseMenu: () => null,
}));

// Import AFTER mocks are set up
import { ShikiCodeBlockContent } from "../ShikiCodeBlock";

describe("ShikiCodeBlockContent", () => {
  const mockEditor = {
    updateBlock: vi.fn(),
  };

  const mockBlock = {
    id: "test-block-id",
    props: {
      code: "console.log('hello')",
      language: "js",
      filename: "test.js",
      highlightLines: "",
      showLineNumbers: true,
      tabs: "[]",
      activeTabIndex: 0,
    },
  };

  it("should render without crashing (smoke test)", () => {
    // This is a smoke test to ensure the component can be instantiated
    // with all its dependencies mocked.
    const { container } = render(
      <ShikiCodeBlockContent editor={mockEditor} block={mockBlock} />
    );
    expect(container).toBeDefined();
  });
});
