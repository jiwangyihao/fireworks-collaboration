import { describe, it, expect, vi, beforeEach } from "vitest";
import * as vitepressApi from "../vitepress";
import { invoke } from "../tauri";

// Mock the invoke function
vi.mock("../tauri", () => ({
  invoke: vi.fn(),
}));

describe("VitePress API", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe("detectVitePressProject", () => {
    it("should call backend with correct arguments", async () => {
      const path = "/test/path";
      const mockResult = {
        isVitepress: true,
        configPath: "/test/path/.vitepress/config.mts",
        contentRoot: ".",
        projectName: "Test Project",
      };

      vi.mocked(invoke).mockResolvedValue(mockResult);

      const result = await vitepressApi.detectVitePressProject(path);

      expect(invoke).toHaveBeenCalledWith("vitepress_detect_project", { path });
      expect(result).toEqual(mockResult);
    });
  });

  describe("checkDependencies", () => {
    it("should call backend with correct arguments", async () => {
      const projectPath = "/test/path";
      const mockResult = {
        installed: true,
        pnpmLockExists: true,
        nodeModulesExists: true,
        pnpmStoreExists: true,
        outdated: false,
        packageManager: "pnpm",
      };

      vi.mocked(invoke).mockResolvedValue(mockResult);

      const result = await vitepressApi.checkDependencies(projectPath);

      expect(invoke).toHaveBeenCalledWith("vitepress_check_dependencies", {
        projectPath,
      });
      expect(result).toEqual(mockResult);
    });
  });

  describe("getDocTree", () => {
    it("should call backend with correct arguments", async () => {
      const projectPath = "/test/path";
      const contentRoot = "src";
      const mockResult = {
        name: "root",
        path: "/test/path",
        nodeType: "folder",
        children: [],
      };

      vi.mocked(invoke).mockResolvedValue(mockResult);

      const result = await vitepressApi.getDocTree(projectPath, contentRoot);

      expect(invoke).toHaveBeenCalledWith("vitepress_get_doc_tree", {
        projectPath,
        contentRoot,
      });
      expect(result).toEqual(mockResult);
    });
  });

  describe("CRUD operations", () => {
    it("createDocument should call backend", async () => {
      vi.mocked(invoke).mockResolvedValue("/path/new.md");
      await vitepressApi.createDocument("/dir", "new.md", "template");
      expect(invoke).toHaveBeenCalledWith("vitepress_create_document", {
        dir: "/dir",
        name: "new.md",
        template: "template",
      });
    });

    it("createFolder should call backend", async () => {
      vi.mocked(invoke).mockResolvedValue("/parent/new-folder");
      await vitepressApi.createFolder("/parent", "new-folder");
      expect(invoke).toHaveBeenCalledWith("vitepress_create_folder", {
        parent: "/parent",
        name: "new-folder",
      });
    });

    it("renameItem should call backend", async () => {
      vi.mocked(invoke).mockResolvedValue("/new/path");
      await vitepressApi.renameItem("/old/path", "new-name");
      expect(invoke).toHaveBeenCalledWith("vitepress_rename", {
        oldPath: "/old/path",
        newName: "new-name",
      });
    });

    it("deleteItem should call backend", async () => {
      vi.mocked(invoke).mockResolvedValue(true);
      await vitepressApi.deleteItem("/path/to/delete");
      expect(invoke).toHaveBeenCalledWith("vitepress_delete", {
        path: "/path/to/delete",
      });
    });
  });
});
