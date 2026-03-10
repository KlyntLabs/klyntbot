import { invoke } from "@tauri-apps/api/core";
import { isTauri } from "@shared/lib/utils";

const DEV_API_BASE = "/api";

/** Typed wrapper around Tauri invoke — falls back to HTTP in browser dev mode. */
export async function ipc<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (isTauri) {
    return invoke<T>(cmd, args);
  }

  // Browser dev mode: call the dev HTTP server via Vite proxy
  const res = await fetch(`${DEV_API_BASE}/${cmd}`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(args ?? {}),
  });

  if (!res.ok) {
    const error = await res.json().catch(() => ({ code: "NETWORK", message: res.statusText }));
    throw error;
  }

  return res.json();
}
