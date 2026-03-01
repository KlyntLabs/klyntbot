import { useState, useEffect, useCallback, useRef } from 'react';
import { ipc } from './useIpc';

interface QueryResult<T> {
  data: T;
  loading: boolean;
  error: string | null;
  refetch: () => void;
}

const isTauri = typeof window !== 'undefined' && '__TAURI__' in window;

/**
 * Fetches data from a Tauri command. Falls back to `fallback` in browser dev mode.
 */
export function useQuery<T>(
  cmd: string,
  args?: Record<string, unknown>,
  fallback?: T,
): QueryResult<T> {
  const [data, setData] = useState<T>(fallback as T);
  const [loading, setLoading] = useState(isTauri);
  const [error, setError] = useState<string | null>(null);
  const argsRef = useRef(args);
  argsRef.current = args;
  const fallbackRef = useRef(fallback);
  fallbackRef.current = fallback;

  const fetch = useCallback(() => {
    if (!isTauri) return;

    setLoading(true);
    setError(null);

    ipc<T>(cmd, argsRef.current)
      .then(setData)
      .catch((e) => setError(String(e)))
      .finally(() => setLoading(false));
  }, [cmd]);

  useEffect(() => {
    fetch();
  }, [fetch]);

  return { data, loading, error, refetch: fetch };
}
