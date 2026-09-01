import type { ApiError } from "@shared/types";

/** Narrow an unknown catch value to a structured ApiError. */
export function parseApiError(e: unknown): ApiError {
  if (typeof e === "object" && e !== null && "code" in e && "message" in e) {
    return e as ApiError;
  }
  return { code: "UNKNOWN_ERROR", message: String(e) };
}

/** Extract a human-readable message from an unknown catch value. */
export function toErrorMessage(e: unknown): string {
  if (e instanceof Error) return e.message;
  if (typeof e === "object" && e !== null) return JSON.stringify(e);
  return String(e);
}
