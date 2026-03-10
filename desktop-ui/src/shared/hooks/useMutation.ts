import { useCallback, useMemo, useRef, useState } from "react";
import type { ApiError } from "@shared/types";
import { parseApiError } from "../lib/errors";
import { ipc } from "./useIpc";

interface MutationResult<T, P> {
  mutate: (params: P) => Promise<T | undefined>;
  loading: boolean;
  error: ApiError | null;
}

/**
 * Wraps a Tauri command (or dev HTTP server) for write operations.
 *
 * When `wrapKey` is provided, params are nested under that key before invoking.
 * This is required for Tauri commands that accept a struct parameter (e.g.
 * `fn task_update(params: TaskUpdateParams)` needs `{ params: { ... } }`).
 */
export function useMutation<T = void, P = Record<string, unknown>>(
  cmd: string,
  wrapKey?: string,
): MutationResult<T, P> {
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<ApiError | null>(null);

  const cmdRef = useRef(cmd);
  cmdRef.current = cmd;
  const wrapKeyRef = useRef(wrapKey);
  wrapKeyRef.current = wrapKey;

  const mutate = useCallback(async (params: P): Promise<T | undefined> => {
    setLoading(true);
    setError(null);
    try {
      const args = wrapKeyRef.current
        ? { [wrapKeyRef.current]: params }
        : (params as Record<string, unknown>);
      const result = await ipc<T>(cmdRef.current, args);
      return result;
    } catch (e) {
      setError(parseApiError(e));
      return undefined;
    } finally {
      setLoading(false);
    }
  }, []);

  return useMemo(() => ({ mutate, loading, error }), [mutate, loading, error]);
}
