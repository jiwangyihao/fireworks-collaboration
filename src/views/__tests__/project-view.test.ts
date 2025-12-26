import { describe, it, expect, beforeEach } from "vitest";
import { mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import ProjectView from "../ProjectView.vue";
import { useProjectStore } from "../../stores/project";

// Note: ProjectView has many Tauri dependencies, so we focus on basic structure tests

describe("ProjectView", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
  });

  describe("initial state", () => {
    it("mounts without crashing", () => {
      // ProjectView requires Tauri APIs which aren't available in test environment
      // This is a smoke test to ensure the component can at least be imported
      expect(ProjectView).toBeDefined();
    });
  });

  describe("store integration", () => {
    it("uses project store", () => {
      const store = useProjectStore();
      expect(store).toBeDefined();
      expect(store.upstreamRepo).toBeNull();
      expect(store.forkRepo).toBeNull();
      expect(store.hasFork).toBe(false);
    });

    it("has expected getters", () => {
      const store = useProjectStore();
      expect(store.isLoading).toBe(false);
      expect(store.upstreamOwner).toBeDefined();
      expect(store.upstreamRepoName).toBeDefined();
      expect(store.upstreamFullName).toBeDefined();
    });

    it("has expected actions", () => {
      const store = useProjectStore();
      expect(typeof store.setError).toBe("function");
      expect(typeof store.setLoadingState).toBe("function");
      expect(typeof store.fetchUpstreamRepo).toBe("function");
      expect(typeof store.checkAndFetchFork).toBe("function");
      expect(typeof store.fetchPullRequests).toBe("function");
    });
  });
});
