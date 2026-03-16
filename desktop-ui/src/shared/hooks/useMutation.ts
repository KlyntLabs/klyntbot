import type { ApiError } from "@shared/types";
import { useCallback, useMemo, useRef, useState } from "react";
import { parseApiError } from "../lib/errors";
import { ipc } from "./useIpc";

interface MutationResult<T, P> {
  mutate: (params: P) => Promise<T | undefined>;
  loading: boolean;
  error: ApiError | null;
}

/** Map command prefix to entity kind for automatic event dispatch. */
function inferEntityKind(cmd: string): string | null {
  if (cmd.startsWith("note_")) return "note";
  if (cmd.startsWith("notebook_")) return "notebook";
  if (cmd.startsWith("inbox_")) return "inbox";
  if (cmd.startsWith("task_")) return "task";
  if (cmd.startsWith("project_")) return "project";
  return null;
}

/** Mutating commands — emit entity:updated after success so listeners react. */
const MUTATING_VERBS = ["create", "update", "delete", "archive", "unarchive", "restore", "move"];

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
      // Emit browser-side entity event so useEvent listeners fire in dev mode
      const entityKind = inferEntityKind(cmdRef.current);
      if (entityKind && MUTATING_VERBS.some((v) => cmdRef.current.includes(v))) {
        window.dispatchEvent(new CustomEvent("entity:updated", { detail: { entityKind } }));
      }
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
