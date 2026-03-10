import type { ApiError } from "@shared/types";
import { useCallback, useEffect, useRef, useState } from "react";
import { parseApiError } from "../lib/errors";
import { ipc } from "./useIpc";

interface QueryResult<T> {
  data: T;
  loading: boolean;
  error: ApiError | null;
  refetch: () => void;
}

interface CacheEntry {
  data: unknown;
  timestamp: number;
  promise?: Promise<unknown>;
}

const cache = new Map<string, CacheEntry>();
const DEFAULT_STALE_TIME = 30_000;

function cacheKey(cmd: string, args?: Record<string, unknown> | null): string | null {
  if (args === null) return null;
  return args === undefined ? cmd : `${cmd}:${JSON.stringify(args)}`;
}

/**
 * Fetches data from a Tauri command with SWR caching and request dedup.
 *
 * Pass `null` for `args` to skip fetching.
 * Pass `undefined` for commands that take no arguments.
 */
export function useQuery<T>(
  cmd: string,
  args?: Record<string, unknown> | null,
  fallback?: T,
  staleTime = DEFAULT_STALE_TIME,
): QueryResult<T> {
  const key = cacheKey(cmd, args);
  const cached = key ? cache.get(key) : undefined;

  const [data, setData] = useState<T>(() => (cached?.data as T) ?? (fallback as T));
  const [loading, setLoading] = useState(() => args !== null && !cached);
  const [error, setError] = useState<ApiError | null>(null);

  const argsRef = useRef(args);
  argsRef.current = args;
  const fallbackRef = useRef(fallback);
  fallbackRef.current = fallback;
  const keyRef = useRef(key);
  keyRef.current = key;
  const staleTimeRef = useRef(staleTime);
  staleTimeRef.current = staleTime;

  const doFetch = useCallback(
    (force = false) => {
      const k = keyRef.current;
      if (k === null) return;

      const existing = cache.get(k);

      // Dedup: reuse in-flight promise
      if (!force && existing?.promise) {
        existing.promise
          .then((result) => setData(result as T))
          .catch((e) => setError(parseApiError(e)));
        return;
      }

      setError(null);
      if (!existing?.data) setLoading(true);

      const promise = ipc<T>(cmd, argsRef.current ?? undefined);
      cache.set(k, { ...(existing ?? { data: undefined, timestamp: 0 }), promise });

      promise
        .then((result) => {
          cache.set(k, { data: result, timestamp: Date.now() });
          setData(result);
        })
        .catch((e) => {
          // Clear failed promise but keep stale data
          if (existing) cache.set(k, { data: existing.data, timestamp: existing.timestamp });
          else cache.delete(k);
          setError(parseApiError(e));
        })
        .finally(() => setLoading(false));
    },
    [cmd],
  );

  const refetch = useCallback(() => doFetch(true), [doFetch]);

  // Use the already-computed cache key as the effect dependency (avoids double stringify)
  const argsKey = key;

  useEffect(() => {
    if (argsKey === null) {
      setData(fallbackRef.current as T);
      setLoading(false);
      return;
    }
    // Serve cached data immediately, then fetch if stale
    const k = keyRef.current;
    const entry = k ? cache.get(k) : undefined;
    if (entry?.data !== undefined) setData(entry.data as T);
    const isStale = !entry || Date.now() - entry.timestamp > staleTimeRef.current;
    if (isStale) doFetch();
  }, [doFetch, argsKey]);

  return { data, loading, error, refetch };
}

/** Invalidate all cache entries matching a command prefix. */
export function invalidateQueries(cmdPrefix: string) {
  for (const k of cache.keys()) {
    if (k.startsWith(cmdPrefix)) cache.delete(k);
  }
}
