import { invoke as rawInvoke } from "@tauri-apps/api/core";

export async function invoke<T>(
  cmd: string,
  args?: Record<string, unknown>,
): Promise<T> {
  return rawInvoke<T>(cmd, args);
}
