/**
 * Credential management store using Pinia
 */

import { defineStore } from "pinia";
import type {
  CredentialInfo,
  AddCredentialRequest,
  UpdateCredentialRequest,
  CredentialConfig,
} from "../api/credential";
import {
  listCredentials,
  addCredential,
  updateCredential,
  deleteCredential,
  getCredential,
  setMasterPassword,
  unlockStore,
  exportAuditLog,
} from "../api/credential";

export const useCredentialStore = defineStore("credential", {
  state: () => ({
    /**
     * List of credentials (with masked passwords)
     */
    credentials: [] as CredentialInfo[],

    /**
     * Loading state
     */
    loading: false,

    /**
     * Error message
     */
    error: null as string | null,

    /**
     * Master password unlock state
     */
    isUnlocked: false,

    /**
     * Configuration (cached from app config)
     */
    config: null as CredentialConfig | null,
  }),

  getters: {
    /**
     * Get credentials sorted by creation time (newest first)
     */
    sortedCredentials: (state) => {
      return [...state.credentials].sort(
        (a, b) => b.createdAt - a.createdAt
      );
    },

    /**
     * Get expired credentials
     */
    expiredCredentials: (state) => {
      return state.credentials.filter((c) => c.isExpired);
    },

    /**
     * Get active (non-expired) credentials
     */
    activeCredentials: (state) => {
      return state.credentials.filter((c) => !c.isExpired);
    },

    /**
     * Count of credentials
     */
    credentialCount: (state) => state.credentials.length,

    /**
     * Check if store needs unlocking (file storage mode)
     */
    needsUnlock: (state) => {
      return (
        state.config?.storage === "file" &&
        !state.isUnlocked &&
        state.credentials.length === 0
      );
    },
  },

  actions: {
    /**
     * Refresh credential list from backend
     */
    async refresh() {
      this.loading = true;
      this.error = null;
      try {
        this.credentials = await listCredentials();
      } catch (e: any) {
        this.error = String(e);
        console.error("Failed to refresh credentials:", e);
      } finally {
        this.loading = false;
      }
    },

    /**
     * Add a new credential
     */
    async add(request: AddCredentialRequest) {
      this.loading = true;
      this.error = null;
      try {
        await addCredential(request);
        // Refresh list to get the new credential
        await this.refresh();
      } catch (e: any) {
        this.error = String(e);
        console.error("Failed to add credential:", e);
        throw e;
      } finally {
        this.loading = false;
      }
    },

    /**
     * Update an existing credential
     */
    async update(request: UpdateCredentialRequest) {
      this.loading = true;
      this.error = null;
      try {
        await updateCredential(request);
        // Refresh list to get updated credential
        await this.refresh();
      } catch (e: any) {
        this.error = String(e);
        console.error("Failed to update credential:", e);
        throw e;
      } finally {
        this.loading = false;
      }
    },

    /**
     * Delete a credential
     */
    async delete(host: string, username: string) {
      this.loading = true;
      this.error = null;
      try {
        await deleteCredential(host, username);
        // Remove from local list
        this.credentials = this.credentials.filter(
          (c) => !(c.host === host && c.username === username)
        );
      } catch (e: any) {
        this.error = String(e);
        console.error("Failed to delete credential:", e);
        throw e;
      } finally {
        this.loading = false;
      }
    },

    /**
     * Get a specific credential
     */
    async get(host: string, username?: string): Promise<CredentialInfo | null> {
      this.loading = true;
      this.error = null;
      try {
        return await getCredential(host, username);
      } catch (e: any) {
        this.error = String(e);
        console.error("Failed to get credential:", e);
        return null;
      } finally {
        this.loading = false;
      }
    },

    /**
     * Unlock store with master password
     */
    async unlock(password: string) {
      if (!this.config) {
        throw new Error("Credential config not set");
      }

      this.loading = true;
      this.error = null;
      try {
        const configWithPassword = {
          ...this.config,
          masterPassword: password,
        };
        await unlockStore(password, configWithPassword);
        this.isUnlocked = true;
        // Refresh credentials after unlocking
        await this.refresh();
      } catch (e: any) {
        this.error = String(e);
        console.error("Failed to unlock store:", e);
        throw e;
      } finally {
        this.loading = false;
      }
    },

    /**
     * Set master password for first-time setup
     */
    async setPassword(password: string) {
      if (!this.config) {
        throw new Error("Credential config not set");
      }

      this.loading = true;
      this.error = null;
      try {
        const configWithPassword = {
          ...this.config,
          masterPassword: password,
        };
        await setMasterPassword(password, configWithPassword);
        this.isUnlocked = true;
        await this.refresh();
      } catch (e: any) {
        this.error = String(e);
        console.error("Failed to set master password:", e);
        throw e;
      } finally {
        this.loading = false;
      }
    },

    /**
     * Export audit log
     */
    async exportLog(): Promise<string> {
      this.loading = true;
      this.error = null;
      try {
        return await exportAuditLog();
      } catch (e: any) {
        this.error = String(e);
        console.error("Failed to export audit log:", e);
        throw e;
      } finally {
        this.loading = false;
      }
    },

    /**
     * Set configuration (should be called during app initialization)
     */
    setConfig(config: CredentialConfig) {
      this.config = config;
      // If using system or memory storage, mark as unlocked
      if (config.storage === "system" || config.storage === "memory") {
        this.isUnlocked = true;
      }
    },

    /**
     * Clear error message
     */
    clearError() {
      this.error = null;
    },
  },
});
