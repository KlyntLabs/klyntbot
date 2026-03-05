import { useCallback, useEffect, useRef, useState } from "react";
import type { ApiError } from "../lib/types";
import { parseApiError } from "../lib/utils";
import { ipc } from "./useIpc";

interface QueryResult<T> {
  data: T;
  loading: boolean;
  error: ApiError | null;
  refetch: () => void;
}

/**
 * Fetches data from a Tauri command or the dev HTTP server (in browser mode).
 *
 * Pass `null` for `args` to skip fetching (e.g. when a required param isn't ready yet).
 * Pass `undefined` for commands that take no arguments.
 */
export function useQuery<T>(
  cmd: string,
  args?: Record<string, unknown> | null,
  fallback?: T,
): QueryResult<T> {
  const [data, setData] = useState<T>(fallback as T);
  const [loading, setLoading] = useState(args !== null);
  const [error, setError] = useState<ApiError | null>(null);
  const argsRef = useRef(args);
  argsRef.current = args;
  const fallbackRef = useRef(fallback);
  fallbackRef.current = fallback;

  const doFetch = useCallback(() => {
    if (argsRef.current === null) return;

    setLoading(true);
    setError(null);

    ipc<T>(cmd, argsRef.current)
      .then(setData)
      .catch((e) => setError(parseApiError(e)))
      .finally(() => setLoading(false));
  }, [cmd]);

  // Re-fetch when cmd or args change. null args = skip + reset to fallback.
  const argsKey = args === null ? null : args === undefined ? "" : JSON.stringify(args);

  useEffect(() => {
    if (argsKey === null) {
      setData(fallbackRef.current as T);
      setLoading(false);
      return;
    }
    doFetch();
  }, [doFetch, argsKey]);

  return { data, loading, error, refetch: doFetch };
}
