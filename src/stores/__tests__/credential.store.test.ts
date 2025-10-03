import { describe, it, expect, vi, beforeEach } from "vitest";
import { setActivePinia, createPinia } from "pinia";

// Mock the API module
vi.mock("../../api/credential", () => ({
  listCredentials: vi.fn(),
  addCredential: vi.fn(),
  updateCredential: vi.fn(),
  deleteCredential: vi.fn(),
  getCredential: vi.fn(),
  setMasterPassword: vi.fn(),
  unlockStore: vi.fn(),
  exportAuditLog: vi.fn(),
  formatTimestamp: (ts: number) => new Date(ts).toLocaleString(),
  isExpiringSoon: (expiresAt: number) => {
    const now = Date.now();
    const days = (expiresAt - now) / (1000 * 60 * 60 * 24);
    return days > 0 && days <= 7;
  },
}));

import { useCredentialStore } from "../credential";
import {
  listCredentials,
  addCredential,
  updateCredential,
  deleteCredential,
  getCredential,
  setMasterPassword,
  unlockStore,
  exportAuditLog,
} from "../../api/credential";
import type { CredentialInfo, CredentialConfig } from "../../api/credential";

const mockCredential: CredentialInfo = {
  host: "github.com",
  username: "testuser",
  maskedPassword: "gh********************",
  createdAt: Date.now() - 1000 * 60 * 60 * 24, // 1 day ago
  expiresAt: Date.now() + 1000 * 60 * 60 * 24 * 30, // 30 days from now
  lastUsedAt: Date.now() - 1000 * 60 * 60, // 1 hour ago
  isExpired: false,
};

const mockExpiredCredential: CredentialInfo = {
  host: "gitlab.com",
  username: "testuser",
  maskedPassword: "gl********************",
  createdAt: Date.now() - 1000 * 60 * 60 * 24 * 100, // 100 days ago
  expiresAt: Date.now() - 1000 * 60 * 60 * 24, // 1 day ago (expired)
  lastUsedAt: Date.now() - 1000 * 60 * 60 * 24 * 10, // 10 days ago
  isExpired: true,
};

describe("stores/credential", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.clearAllMocks();
  });

  describe("refresh", () => {
    it("should load credentials successfully", async () => {
      (listCredentials as any).mockResolvedValueOnce([mockCredential]);
      const store = useCredentialStore();

      await store.refresh();

      expect(store.credentials).toEqual([mockCredential]);
      expect(store.error).toBeNull();
      expect(store.loading).toBe(false);
    });

    it("should handle errors", async () => {
      (listCredentials as any).mockRejectedValueOnce(new Error("Failed to list"));
      const store = useCredentialStore();

      await store.refresh();

      expect(store.credentials).toEqual([]);
      expect(store.error).toContain("Failed to list");
      expect(store.loading).toBe(false);
    });
  });

  describe("add", () => {
    it("should add credential and refresh list", async () => {
      (addCredential as any).mockResolvedValueOnce(undefined);
      (listCredentials as any).mockResolvedValueOnce([mockCredential]);
      const store = useCredentialStore();

      await store.add({
        host: "github.com",
        username: "testuser",
        passwordOrToken: "ghp_test123",
        expiresInDays: 30,
      });

      expect(addCredential).toHaveBeenCalledWith({
        host: "github.com",
        username: "testuser",
        passwordOrToken: "ghp_test123",
        expiresInDays: 30,
      });
      expect(store.credentials).toEqual([mockCredential]);
      expect(store.error).toBeNull();
      expect(store.loading).toBe(false);
    });

    it("should handle add errors", async () => {
      (addCredential as any).mockRejectedValueOnce(new Error("Add failed"));
      const store = useCredentialStore();

      await expect(
        store.add({
          host: "github.com",
          username: "testuser",
          passwordOrToken: "ghp_test123",
        })
      ).rejects.toThrow("Add failed");

      expect(store.error).toContain("Add failed");
      expect(store.loading).toBe(false);
    });
  });

  describe("update", () => {
    it("should update credential and refresh list", async () => {
      (updateCredential as any).mockResolvedValueOnce(undefined);
      (listCredentials as any).mockResolvedValueOnce([mockCredential]);
      const store = useCredentialStore();

      await store.update({
        host: "github.com",
        username: "testuser",
        newPassword: "ghp_new123",
        expiresInDays: 60,
      });

      expect(updateCredential).toHaveBeenCalledWith({
        host: "github.com",
        username: "testuser",
        newPassword: "ghp_new123",
        expiresInDays: 60,
      });
      expect(store.credentials).toEqual([mockCredential]);
      expect(store.error).toBeNull();
      expect(store.loading).toBe(false);
    });

    it("should handle update errors", async () => {
      (updateCredential as any).mockRejectedValueOnce(new Error("Update failed"));
      const store = useCredentialStore();

      await expect(
        store.update({
          host: "github.com",
          username: "testuser",
          newPassword: "ghp_new123",
        })
      ).rejects.toThrow("Update failed");

      expect(store.error).toContain("Update failed");
    });
  });

  describe("delete", () => {
    it("should delete credential from store", async () => {
      (deleteCredential as any).mockResolvedValueOnce(undefined);
      const store = useCredentialStore();
      store.credentials = [mockCredential];

      await store.delete("github.com", "testuser");

      expect(deleteCredential).toHaveBeenCalledWith("github.com", "testuser");
      expect(store.credentials).toEqual([]);
      expect(store.error).toBeNull();
      expect(store.loading).toBe(false);
    });

    it("should handle delete errors", async () => {
      (deleteCredential as any).mockRejectedValueOnce(new Error("Delete failed"));
      const store = useCredentialStore();

      await expect(store.delete("github.com", "testuser")).rejects.toThrow(
        "Delete failed"
      );

      expect(store.error).toContain("Delete failed");
    });
  });

  describe("get", () => {
    it("should get specific credential", async () => {
      (getCredential as any).mockResolvedValueOnce(mockCredential);
      const store = useCredentialStore();

      const result = await store.get("github.com", "testuser");

      expect(getCredential).toHaveBeenCalledWith("github.com", "testuser");
      expect(result).toEqual(mockCredential);
      expect(store.error).toBeNull();
    });

    it("should return null on error", async () => {
      (getCredential as any).mockRejectedValueOnce(new Error("Not found"));
      const store = useCredentialStore();

      const result = await store.get("github.com", "testuser");

      expect(result).toBeNull();
      expect(store.error).toContain("Not found");
    });
  });

  describe("unlock", () => {
    it("should unlock store with password", async () => {
      (unlockStore as any).mockResolvedValueOnce(undefined);
      (listCredentials as any).mockResolvedValueOnce([mockCredential]);
      const store = useCredentialStore();
      const config: CredentialConfig = { storage: "file", auditMode: true };
      store.setConfig(config);

      await store.unlock("masterPassword123");

      expect(unlockStore).toHaveBeenCalledWith("masterPassword123", {
        ...config,
        masterPassword: "masterPassword123",
      });
      expect(store.isUnlocked).toBe(true);
      expect(store.credentials).toEqual([mockCredential]);
    });

    it("should throw error if config not set", async () => {
      const store = useCredentialStore();

      await expect(store.unlock("password")).rejects.toThrow(
        "Credential config not set"
      );
    });

    it("should handle unlock errors", async () => {
      (unlockStore as any).mockRejectedValueOnce(new Error("Wrong password"));
      const store = useCredentialStore();
      store.setConfig({ storage: "file", auditMode: true });

      await expect(store.unlock("wrongPassword")).rejects.toThrow("Wrong password");

      expect(store.error).toContain("Wrong password");
      expect(store.isUnlocked).toBe(false);
    });
  });

  describe("setPassword", () => {
    it("should set master password", async () => {
      (setMasterPassword as any).mockResolvedValueOnce(undefined);
      (listCredentials as any).mockResolvedValueOnce([]);
      const store = useCredentialStore();
      const config: CredentialConfig = { storage: "file", auditMode: true };
      store.setConfig(config);

      await store.setPassword("newMasterPassword");

      expect(setMasterPassword).toHaveBeenCalledWith("newMasterPassword", {
        ...config,
        masterPassword: "newMasterPassword",
      });
      expect(store.isUnlocked).toBe(true);
    });

    it("should throw error if config not set", async () => {
      const store = useCredentialStore();

      await expect(store.setPassword("password")).rejects.toThrow(
        "Credential config not set"
      );
    });
  });

  describe("exportLog", () => {
    it("should export audit log", async () => {
      const mockLog = JSON.stringify([{ event: "test" }]);
      (exportAuditLog as any).mockResolvedValueOnce(mockLog);
      const store = useCredentialStore();

      const result = await store.exportLog();

      expect(exportAuditLog).toHaveBeenCalled();
      expect(result).toBe(mockLog);
      expect(store.error).toBeNull();
    });

    it("should handle export errors", async () => {
      (exportAuditLog as any).mockRejectedValueOnce(new Error("Export failed"));
      const store = useCredentialStore();

      await expect(store.exportLog()).rejects.toThrow("Export failed");

      expect(store.error).toContain("Export failed");
    });
  });

  describe("setConfig", () => {
    it("should set config and mark unlocked for system storage", () => {
      const store = useCredentialStore();
      const config: CredentialConfig = { storage: "system", auditMode: true };

      store.setConfig(config);

      expect(store.config).toEqual(config);
      expect(store.isUnlocked).toBe(true);
    });

    it("should set config and mark unlocked for memory storage", () => {
      const store = useCredentialStore();
      const config: CredentialConfig = { storage: "memory", auditMode: true };

      store.setConfig(config);

      expect(store.config).toEqual(config);
      expect(store.isUnlocked).toBe(true);
    });

    it("should set config and not mark unlocked for file storage", () => {
      const store = useCredentialStore();
      const config: CredentialConfig = { storage: "file", auditMode: true };

      store.setConfig(config);

      expect(store.config).toEqual(config);
      expect(store.isUnlocked).toBe(false);
    });
  });

  describe("getters", () => {
    it("sortedCredentials should sort by createdAt descending", () => {
      const store = useCredentialStore();
      const older = { ...mockCredential, createdAt: 1000 };
      const newer = { ...mockCredential, createdAt: 2000 };
      store.credentials = [older, newer];

      expect(store.sortedCredentials).toEqual([newer, older]);
    });

    it("expiredCredentials should filter expired credentials", () => {
      const store = useCredentialStore();
      store.credentials = [mockCredential, mockExpiredCredential];

      expect(store.expiredCredentials).toEqual([mockExpiredCredential]);
    });

    it("activeCredentials should filter non-expired credentials", () => {
      const store = useCredentialStore();
      store.credentials = [mockCredential, mockExpiredCredential];

      expect(store.activeCredentials).toEqual([mockCredential]);
    });

    it("credentialCount should return count", () => {
      const store = useCredentialStore();
      store.credentials = [mockCredential, mockExpiredCredential];

      expect(store.credentialCount).toBe(2);
    });

    it("needsUnlock should return true for file storage with no credentials", () => {
      const store = useCredentialStore();
      store.setConfig({ storage: "file", auditMode: true });
      store.isUnlocked = false;
      store.credentials = [];

      expect(store.needsUnlock).toBe(true);
    });

    it("needsUnlock should return false for file storage with credentials", () => {
      const store = useCredentialStore();
      store.setConfig({ storage: "file", auditMode: true });
      store.isUnlocked = false;
      store.credentials = [mockCredential];

      expect(store.needsUnlock).toBe(false);
    });

    it("needsUnlock should return false for system storage", () => {
      const store = useCredentialStore();
      store.setConfig({ storage: "system", auditMode: true });
      store.isUnlocked = true;
      store.credentials = [];

      expect(store.needsUnlock).toBe(false);
    });
  });

  describe("clearError", () => {
    it("should clear error message", () => {
      const store = useCredentialStore();
      store.error = "Some error";

      store.clearError();

      expect(store.error).toBeNull();
    });
  });
});
