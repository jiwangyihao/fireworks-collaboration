import { setActivePinia, createPinia } from "pinia";
import { describe, it, expect, beforeEach, vi } from "vitest";
import { useDocumentStore } from "../document";
import * as api from "../../api/vitepress";

// Mock the API module with correct export names
vi.mock("../../api/vitepress", () => ({
  detectVitePressProject: vi.fn(),
  checkDependencies: vi.fn(),
  installDependencies: vi.fn(),
  getDocTree: vi.fn(),
  createDocument: vi.fn(),
  createFolder: vi.fn(),
  renameItem: vi.fn(),
  deleteItem: vi.fn(),
  parseConfig: vi.fn(),
}));

describe("Document Store", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.clearAllMocks();
  });

  it("detectProject should update state on success", async () => {
    const store = useDocumentStore();
    store.worktreePath = "/project/root"; // Set worktreePath!
    const mockDetection = {
      isVitepress: true, // Note: camelCase matches interface in api/vitepress.ts
      configPath: "/path/to/config.mts",
      package_json_path: "/path/to/package.json",
      projectName: "Test Project",
    };

    vi.mocked(api.detectVitePressProject).mockResolvedValue(
      mockDetection as any
    );
    vi.mocked(api.checkDependencies).mockResolvedValue({
      installed: true,
      pnpmLockExists: false,
      nodeModulesExists: true,
      pnpmStoreExists: false,
      outdated: false,
      packageManager: "npm",
    });

    // Mock getDocTree because checkProjectDependencies calls it if installed=true
    vi.mocked(api.getDocTree).mockResolvedValue({
      name: "root",
      path: "/",
      nodeType: "folder",
      children: [],
    } as any);

    await store.detectProject();

    expect(store.isVitePress).toBe(true);
    expect(store.detection).toEqual(mockDetection);
    expect(store.dependencyStatus?.installed).toBe(true);
  });

  it("loadDocTree should update tree state", async () => {
    const store = useDocumentStore();
    const mockTree = {
      name: "root",
      path: "/",
      nodeType: "folder",
      children: [],
    };

    // Setup initial state
    store.worktreePath = "/project/root";
    // Mock successful detection to ensure loadDocTree has context if needed
    vi.mocked(api.getDocTree).mockResolvedValue(mockTree as any);

    await store.loadDocTree();

    expect(store.docTree).toEqual(mockTree);
    expect(api.getDocTree).toHaveBeenCalledWith("/project/root", ".");
  });

  it("createDocument should call API and refresh tree", async () => {
    const store = useDocumentStore();
    store.worktreePath = "/project/root";
    // Mock successful creation
    vi.mocked(api.createDocument).mockResolvedValue("Document created");
    // Mock tree refresh
    const mockTree = {
      name: "root",
      path: "/",
      nodeType: "folder",
      children: [],
    };
    vi.mocked(api.getDocTree).mockResolvedValue(mockTree as any);

    await store.createDocument("/project/root/docs", "new-doc");

    // Expect validation: only 2 args passed
    expect(api.createDocument).toHaveBeenCalledWith(
      "/project/root/docs",
      "new-doc"
    );
    expect(api.getDocTree).toHaveBeenCalled();
  });

  it("deleteItem should call API and refresh tree", async () => {
    const store = useDocumentStore();
    store.worktreePath = "/project/root";
    vi.mocked(api.deleteItem).mockResolvedValue(true);
    vi.mocked(api.getDocTree).mockResolvedValue({} as any);

    await store.deleteItem("/project/root/docs/old.md");

    expect(api.deleteItem).toHaveBeenCalledWith("/project/root/docs/old.md");
    expect(api.getDocTree).toHaveBeenCalled();
  });

  it("installDependencies should set installing state", async () => {
    const store = useDocumentStore();
    store.worktreePath = "/project/root";

    // Simulate long running installation
    let resolveInstall: (value: any) => void;
    const installPromise = new Promise((resolve) => {
      resolveInstall = resolve;
    });

    vi.mocked(api.installDependencies).mockReturnValue(
      installPromise as Promise<void>
    );

    const actionPromise = store.installProjectDependencies();

    expect(store.isInstalling).toBe(true);
    expect(api.installDependencies).toHaveBeenCalledWith("/project/root");

    // Complete installation (API returns)
    resolveInstall!(undefined);
    await actionPromise;

    // 注意：isInstalling 不会在这里重置，它由 View 层的 vitepress://install-finish 事件监听器负责
    // Store 只负责启动安装任务，保持 isInstalling=true 直到 View 层收到完成事件
    expect(store.isInstalling).toBe(true);
  });
});
