/**
 * Credential management API
 *
 * Provides TypeScript interfaces and Tauri command wrappers for credential storage operations.
 */

import { invoke } from "@tauri-apps/api/core";

/**
 * Credential information (with masked password for display)
 */
export interface CredentialInfo {
  host: string;
  username: string;
  maskedPassword: string;
  createdAt: number; // Unix timestamp in seconds
  expiresAt?: number;
  lastUsedAt?: number;
  isExpired: boolean;
}

/**
 * Request to add a new credential
 */
export interface AddCredentialRequest {
  host: string;
  username: string;
  passwordOrToken: string;
  expiresInDays?: number;
}

/**
 * Request to update an existing credential
 */
export interface UpdateCredentialRequest {
  host: string;
  username: string;
  newPassword: string;
  expiresInDays?: number;
}

/**
 * Credential storage configuration
 */
export interface CredentialConfig {
  storage: "system" | "file" | "memory";
  filePath?: string;
  masterPassword?: string;
  defaultTtlSeconds?: number;
  keyCacheTtlSeconds?: number;
  auditMode?: boolean;
}

/**
 * Add a new credential to the store
 */
export async function addCredential(
  request: AddCredentialRequest
): Promise<void> {
  await invoke("add_credential", { request });
}

/**
 * Get a credential from the store
 */
export async function getCredential(
  host: string,
  username?: string
): Promise<CredentialInfo | null> {
  return await invoke("get_credential", { host, username });
}

/**
 * Update an existing credential
 */
export async function updateCredential(
  request: UpdateCredentialRequest
): Promise<void> {
  await invoke("update_credential", { request });
}

/**
 * Delete a credential from the store
 */
export async function deleteCredential(
  host: string,
  username: string
): Promise<void> {
  await invoke("delete_credential", { host, username });
}

/**
 * List all credentials in the store
 */
export async function listCredentials(): Promise<CredentialInfo[]> {
  return await invoke("list_credentials");
}

/**
 * Set master password for encrypted file storage
 */
export async function setMasterPassword(
  password: string,
  config: CredentialConfig
): Promise<void> {
  await invoke("set_master_password", { password, config });
}

/**
 * Unlock credential store with master password
 */
export async function unlockStore(
  password: string,
  config: CredentialConfig
): Promise<void> {
  await invoke("unlock_store", { password, config });
}

/**
 * Export audit log as JSON
 */
export async function exportAuditLog(): Promise<string> {
  return await invoke("export_audit_log");
}

/**
 * Clean up expired credentials
 *
 * Removes all credentials that have passed their expiration time.
 *
 * @returns The number of credentials removed
 */
export async function cleanupExpiredCredentials(): Promise<number> {
  return await invoke("cleanup_expired_credentials");
}

/**
 * Format timestamp to readable date string
 */
export function formatTimestamp(timestamp?: number): string {
  if (!timestamp) return "N/A";
  return new Date(timestamp * 1000).toLocaleString();
}

/**
 * Check if a credential is expiring soon (within 7 days)
 */
export function isExpiringSoon(expiresAt?: number): boolean {
  if (!expiresAt) return false;
  const now = Date.now() / 1000; // Current time in seconds
  const sevenDaysInSeconds = 7 * 24 * 60 * 60;
  return expiresAt - now < sevenDaysInSeconds && expiresAt > now;
}
