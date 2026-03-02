import { invoke } from '@tauri-apps/api/core';

/** Typed wrapper around Tauri invoke. */
export async function ipc<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  return invoke<T>(cmd, args);
}
