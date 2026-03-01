import { invoke } from '@tauri-apps/api/core';

/**
 * Typed wrapper around Tauri invoke.
 * Usage: const tasks = await ipc<Task[]>("task_list")
 * Not called yet — frontend uses mock data directly.
 */
export async function ipc<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  return invoke<T>(cmd, args);
}
