import { describe, it, expect, vi, beforeEach } from "vitest";

// Mock Tauri invoke
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

import { invoke } from "@tauri-apps/api/core";
import {
  listCredentials,
  addCredential,
  updateCredential,
  deleteCredential,
  getCredential,
  setMasterPassword,
  unlockStore,
  exportAuditLog,
  formatTimestamp,
  isExpiringSoon,
} from "../credential";
import type { CredentialInfo } from "../credential";

const mockInvoke = vi.mocked(invoke);

describe("api/credential", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe("listCredentials", () => {
    it("should call list_credentials command", async () => {
      const mockData: CredentialInfo[] = [
        {
          host: "github.com",
          username: "testuser",
          maskedPassword: "gh********************",
          createdAt: 1704067200000,
          expiresAt: 1706659200000,
          lastUsedAt: 1705276800000,
          isExpired: false,
        },
      ];
      mockInvoke.mockResolvedValueOnce(mockData);

      const result = await listCredentials();

      expect(mockInvoke).toHaveBeenCalledWith("list_credentials");
      expect(result).toEqual(mockData);
    });
  });

  describe("addCredential", () => {
    it("should call add_credential command with correct parameters", async () => {
      mockInvoke.mockResolvedValueOnce(undefined);

      await addCredential({
        host: "github.com",
        username: "testuser",
        passwordOrToken: "ghp_test123",
        expiresInDays: 30,
      });

      expect(mockInvoke).toHaveBeenCalledWith("add_credential", {
        request: {
          host: "github.com",
          username: "testuser",
          passwordOrToken: "ghp_test123",
          expiresInDays: 30,
        },
      });
    });

    it("should handle optional expiresInDays", async () => {
      mockInvoke.mockResolvedValueOnce(undefined);

      await addCredential({
        host: "github.com",
        username: "testuser",
        passwordOrToken: "ghp_test123",
      });

      expect(mockInvoke).toHaveBeenCalledWith("add_credential", {
        request: {
          host: "github.com",
          username: "testuser",
          passwordOrToken: "ghp_test123",
        },
      });
    });
  });

  describe("updateCredential", () => {
    it("should call update_credential command", async () => {
      mockInvoke.mockResolvedValueOnce(undefined);

      await updateCredential({
        host: "github.com",
        username: "testuser",
        newPassword: "ghp_new123",
        expiresInDays: 60,
      });

      expect(mockInvoke).toHaveBeenCalledWith("update_credential", {
        request: {
          host: "github.com",
          username: "testuser",
          newPassword: "ghp_new123",
          expiresInDays: 60,
        },
      });
    });
  });

  describe("deleteCredential", () => {
    it("should call delete_credential command with host and username", async () => {
      mockInvoke.mockResolvedValueOnce(undefined);

      await deleteCredential("github.com", "testuser");

      expect(mockInvoke).toHaveBeenCalledWith("delete_credential", {
        host: "github.com",
        username: "testuser",
      });
    });
  });

  describe("getCredential", () => {
    it("should call get_credential command with host and username", async () => {
      const mockCred: CredentialInfo = {
        host: "github.com",
        username: "testuser",
        maskedPassword: "gh********************",
        createdAt: 1704067200000,
        expiresAt: 1706659200000,
        lastUsedAt: 1705276800000,
        isExpired: false,
      };
      mockInvoke.mockResolvedValueOnce(mockCred);

      const result = await getCredential("github.com", "testuser");

      expect(mockInvoke).toHaveBeenCalledWith("get_credential", {
        host: "github.com",
        username: "testuser",
      });
      expect(result).toEqual(mockCred);
    });

    it("should handle optional username", async () => {
      const mockCred: CredentialInfo = {
        host: "github.com",
        username: "testuser",
        maskedPassword: "gh********************",
        createdAt: 1704067200000,
        expiresAt: 1706659200000,
        lastUsedAt: 1705276800000,
        isExpired: false,
      };
      mockInvoke.mockResolvedValueOnce(mockCred);

      const result = await getCredential("github.com");

      expect(mockInvoke).toHaveBeenCalledWith("get_credential", {
        host: "github.com",
        username: undefined,
      });
      expect(result).toEqual(mockCred);
    });
  });

  describe("setMasterPassword", () => {
    it("should call set_master_password command", async () => {
      mockInvoke.mockResolvedValueOnce(undefined);

      await setMasterPassword("strongPassword123", {
        storage: "file",
        auditMode: true,
      });

      expect(mockInvoke).toHaveBeenCalledWith("set_master_password", {
        password: "strongPassword123",
        config: {
          storage: "file",
          auditMode: true,
        },
      });
    });
  });

  describe("unlockStore", () => {
    it("should call unlock_credential_store command", async () => {
      mockInvoke.mockResolvedValueOnce(undefined);

      await unlockStore("masterPassword", {
        storage: "file",
        auditMode: true,
        masterPassword: "masterPassword",
      });

      expect(mockInvoke).toHaveBeenCalledWith("unlock_store", {
        password: "masterPassword",
        config: {
          storage: "file",
          auditMode: true,
          masterPassword: "masterPassword",
        },
      });
    });
  });

  describe("exportAuditLog", () => {
    it("should call export_credential_audit_log command", async () => {
      const mockLog = JSON.stringify([{ event: "test" }]);
      mockInvoke.mockResolvedValueOnce(mockLog);

      const result = await exportAuditLog();

      expect(mockInvoke).toHaveBeenCalledWith("export_audit_log");
      expect(result).toBe(mockLog);
    });
  });

  describe("formatTimestamp", () => {
    it("should format timestamp as locale string", () => {
      const timestamp = 1704067200000; // 2024-01-01 00:00:00 UTC
      const result = formatTimestamp(timestamp);

      // Just check it returns a string (exact format depends on locale)
      expect(typeof result).toBe("string");
      expect(result.length).toBeGreaterThan(0);
    });

    it("should handle 0 timestamp", () => {
      const result = formatTimestamp(0);

      expect(typeof result).toBe("string");
      expect(result).toBe("N/A");
    });
  });

  describe("isExpiringSoon", () => {
    it("should return true for expiry within 7 days", () => {
      const now = Date.now() / 1000; // Convert to seconds
      const expiresIn5Days = now + 5 * 24 * 60 * 60;

      expect(isExpiringSoon(expiresIn5Days)).toBe(true);
    });

    it("should return false for expiry beyond 7 days", () => {
      const now = Date.now();
      const expiresIn30Days = now + 30 * 24 * 60 * 60 * 1000;

      expect(isExpiringSoon(expiresIn30Days)).toBe(false);
    });

    it("should return false for already expired", () => {
      const now = Date.now();
      const expiredYesterday = now - 24 * 60 * 60 * 1000;

      expect(isExpiringSoon(expiredYesterday)).toBe(false);
    });

    it("should return true for expiry exactly 7 days away", () => {
      const now = Date.now() / 1000; // Convert to seconds
      const expiresIn7Days = now + 7 * 24 * 60 * 60 - 1; // Just under 7 days

      expect(isExpiringSoon(expiresIn7Days)).toBe(true);
    });

    it("should return false for expiry exactly 8 days away", () => {
      const now = Date.now();
      const expiresIn8Days = now + 8 * 24 * 60 * 60 * 1000;

      expect(isExpiringSoon(expiresIn8Days)).toBe(false);
    });
  });
});
