import { defineStore } from "pinia";
import { ref, computed } from "vue";
import {
  type VitePressDetection,
  type DependencyStatus,
  type DocTreeNode,
  type VitePressConfig,
  detectVitePressProject,
  checkDependencies,
  installDependencies as apiInstallDependencies,
  getDocTree,
  createDocument as apiCreateDocument,
  createFolder as apiCreateFolder,
  renameItem as apiRenameItem,
  deleteItem as apiDeleteItem,
  parseConfig,
} from "../api/vitepress";

export const useDocumentStore = defineStore("document", () => {
  // ==========================================================================
  // State
  // ==========================================================================

  // Project Info
  const worktreePath = ref<string | null>(null);
  const detection = ref<VitePressDetection | null>(null);
  const config = ref<VitePressConfig | null>(null);

  // Difficulty / Dependencies
  const dependencyStatus = ref<DependencyStatus | null>(null);
  const isInstalling = ref(false);
  const installLogs = ref<string[]>([]);

  // Document Tree
  const docTree = ref<DocTreeNode | null>(null);
  const loadingTree = ref(false);

  // Document Content (E2)
  const currentBlocks = ref<any[]>([]);
  const currentFrontmatter = ref<Record<string, any>>({});
  const isDirty = ref(false);
  const isSaving = ref(false);

  // Selection
  const selectedPath = ref<string | null>(null);
  // Placeholder for E2: content of current document
  // const currentDocument = ref<DocumentContent | null>(null);

  // Generic Loading/Error
  const loadingDetection = ref(false);
  const loadingDependencies = ref(false);
  const loadingContent = ref(false);
  const error = ref<string | null>(null);

  // ==========================================================================
  // Getters
  // ==========================================================================

  const isVitePress = computed(() => detection.value?.isVitepress ?? false);
  const projectName = computed(
    () => detection.value?.projectName || "VitePress 项目"
  );
  const needsInstall = computed(
    () => dependencyStatus.value && !dependencyStatus.value.installed
  );

  const isLoading = computed(
    () =>
      loadingDetection.value ||
      loadingDependencies.value ||
      loadingTree.value ||
      isInstalling.value
  );

  // ==========================================================================
  // Actions
  // ==========================================================================

  /** Initialize/Switch Project */
  async function bindProject(path: string) {
    if (worktreePath.value === path) return;

    resetState();
    worktreePath.value = path;
    await detectProject();
  }

  function resetState() {
    worktreePath.value = null;
    detection.value = null;
    config.value = null;
    dependencyStatus.value = null;
    docTree.value = null;
    selectedPath.value = null;
    currentBlocks.value = [];
    currentFrontmatter.value = {};
    isDirty.value = false;
    isSaving.value = false;
    loadingContent.value = false;
    error.value = null;
    installLogs.value = [];
  }

  /** Detect Project & Config */
  async function detectProject() {
    if (!worktreePath.value) return;

    loadingDetection.value = true;
    error.value = null;

    try {
      detection.value = await detectVitePressProject(worktreePath.value);

      if (!detection.value.isVitepress) {
        error.value = "该目录不是 VitePress 项目";
        return;
      }

      // Load Config (Best effort)
      try {
        config.value = await parseConfig(worktreePath.value);
      } catch (e) {
        console.warn("Failed to parse config:", e);
      }

      // Check Dependencies
      await checkProjectDependencies();
    } catch (e) {
      error.value = `检测失败: ${e}`;
    } finally {
      loadingDetection.value = false;
    }
  }

  /** Check Dependencies */
  async function checkProjectDependencies() {
    if (!worktreePath.value) return;

    loadingDependencies.value = true;
    try {
      dependencyStatus.value = await checkDependencies(worktreePath.value);
      if (dependencyStatus.value.installed) {
        await loadDocTree();
      }
    } catch (e) {
      error.value = `检查依赖失败: ${e}`;
    } finally {
      loadingDependencies.value = false;
    }
  }

  /** Load Document Tree */
  async function loadDocTree() {
    if (!worktreePath.value) return;

    loadingTree.value = true;
    try {
      const contentRoot = detection.value?.contentRoot || ".";
      docTree.value = await getDocTree(worktreePath.value, contentRoot);
      // Ensure selectedPath is still valid or reset it?
      // For now keep it, could be useful to maintain selection across reloads
    } catch (e) {
      error.value = `加载文档树失败: ${e}`;
    } finally {
      loadingTree.value = false;
    }
  }

  /** Install Dependencies (Wrapper needed for event listening in View) */
  async function installProjectDependencies() {
    if (!worktreePath.value) return;
    isInstalling.value = true;
    try {
      await apiInstallDependencies(worktreePath.value);
      isInstalling.value = false;
    } catch (e) {
      isInstalling.value = false; // Reset on error
      throw e; // Let View handle error display or toast
    }
  }

  // CRUD Actions

  async function createDocument(dir: string, name: string) {
    await apiCreateDocument(dir, name);
    await loadDocTree();
  }

  async function createFolder(parent: string, name: string) {
    await apiCreateFolder(parent, name);
    await loadDocTree();
  }

  async function renameItem(path: string, newName: string) {
    await apiRenameItem(path, newName);
    await loadDocTree();

    // If we renamed the selected file, update selectedPath?
    // Complex logic, simpler to deselect or just refresh tree
    if (selectedPath.value === path) {
      // Ideally calculate new path to keep selection
      selectedPath.value = null;
    }
  }

  async function deleteItem(path: string) {
    await apiDeleteItem(path);
    await loadDocTree();
    if (selectedPath.value === path) {
      selectedPath.value = null;
    }
  }

  async function selectDocument(path: string) {
    if (selectedPath.value === path) return;
    selectedPath.value = path;
    await loadDocumentContent(path);
  }

  /** Load document content and convert to blocks (E2) */
  async function loadDocumentContent(path: string) {
    loadingContent.value = true;
    try {
      const { readDocument } = await import("../api/vitepress");
      const { loadMarkdownToEditor } = await import(
        "../utils/blocknote-adapter"
      );

      const doc = await readDocument(path);
      currentFrontmatter.value = doc.frontmatter || {};
      currentBlocks.value = await loadMarkdownToEditor(doc.content);
      isDirty.value = false;
    } catch (e) {
      error.value = `加载文档失败: ${e}`;
    } finally {
      loadingContent.value = false;
    }
  }

  /** Save current blocks back to markdown file (E2) */
  async function saveDocumentContent() {
    if (!selectedPath.value || isSaving.value) return;

    isSaving.value = true;
    try {
      const { saveDocument } = await import("../api/vitepress");
      const { saveEditorToMarkdown } = await import(
        "../utils/blocknote-adapter"
      );

      const markdown = await saveEditorToMarkdown(
        currentBlocks.value,
        currentFrontmatter.value
      );

      await saveDocument(selectedPath.value, markdown);
      isDirty.value = false;
    } catch (e) {
      error.value = `保存文档失败: ${e}`;
      throw e;
    } finally {
      isSaving.value = false;
    }
  }

  function updateEditorBlocks(blocks: any[]) {
    currentBlocks.value = blocks;
    isDirty.value = true;
  }

  return {
    // State
    worktreePath,
    detection,
    config,
    dependencyStatus,
    isInstalling,
    installLogs,
    docTree,
    loadingTree,
    selectedPath,
    currentBlocks,
    currentFrontmatter,
    isDirty,
    isSaving,
    loadingDetection,
    loadingDependencies,
    loadingContent,
    error,

    // Getters
    isVitePress,
    projectName,
    needsInstall,
    isLoading,

    // Actions
    bindProject,
    detectProject,
    checkProjectDependencies,
    loadDocTree,
    installProjectDependencies,
    createDocument,
    createFolder,
    renameItem,
    deleteItem,
    selectDocument,
    loadDocumentContent,
    saveDocumentContent,
    updateEditorBlocks,
    resetState,
  };
});
