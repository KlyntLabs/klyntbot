import { type ClassValue, clsx } from "clsx";
import { twMerge } from "tailwind-merge";

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

export const isTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

/**
 * Vite's http-proxy buffers SSE responses, so we connect directly to the
 * dev server on :3456 (which already sets CORS headers for localhost:1420).
 */
export const DEV_SSE_BASE = "http://127.0.0.1:3456";

export { parseApiError } from "./errors";
export { formatCost, formatDuration, formatTokens, qualifiedToolName } from "./format";
export { groupBy } from "./group-by";
