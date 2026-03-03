import { useState, useEffect, useCallback, useRef } from 'react';
import { ipc } from './useIpc';
import { isTauri, parseApiError } from '../lib/utils';
import type { ApiError } from '../lib/types';

interface QueryResult<T> {
  data: T;
  loading: boolean;
  error: ApiError | null;
  refetch: () => void;
}

/**
 * Fetches data from a Tauri command. Falls back to `fallback` in browser dev mode.
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
  const [loading, setLoading] = useState(isTauri && args !== null);
  const [error, setError] = useState<ApiError | null>(null);
  const argsRef = useRef(args);
  argsRef.current = args;
  const fallbackRef = useRef(fallback);
  fallbackRef.current = fallback;

  const fetch = useCallback(() => {
    if (!isTauri || argsRef.current === null) return;

    setLoading(true);
    setError(null);

    ipc<T>(cmd, argsRef.current)
      .then(setData)
      .catch((e) => setError(parseApiError(e)))
      .finally(() => setLoading(false));
  }, [cmd]);

  // Re-fetch when cmd or args change. null args = skip + reset to fallback.
  const argsKey = args === null ? null : args === undefined ? '' : JSON.stringify(args);

  useEffect(() => {
    if (argsKey === null) {
      setData(fallbackRef.current as T);
      setLoading(false);
      return;
    }
    fetch();
  }, [fetch, argsKey]);

  return { data, loading, error, refetch: fetch };
}
