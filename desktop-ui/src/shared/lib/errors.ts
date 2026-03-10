import type { ApiError } from "@shared/types";

/** Narrow an unknown catch value to a structured ApiError. */
export function parseApiError(e: unknown): ApiError {
  if (typeof e === "object" && e !== null && "code" in e && "message" in e) {
    return e as ApiError;
  }
  return { code: "UNKNOWN_ERROR", message: String(e) };
}
