import { useState, useCallback } from 'react';
import { ipc } from './useIpc';

interface MutationResult<T, P> {
  mutate: (params: P) => Promise<T | undefined>;
  loading: boolean;
  error: string | null;
}

/**
 * Wraps a Tauri command for write operations.
 * Returns `undefined` in browser dev mode.
 */
export function useMutation<T = void, P = Record<string, unknown>>(
  cmd: string,
): MutationResult<T, P> {
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const isTauri = typeof window !== 'undefined' && '__TAURI__' in window;

  const mutate = useCallback(
    async (params: P): Promise<T | undefined> => {
      if (!isTauri) return undefined;

      setLoading(true);
      setError(null);
      try {
        const result = await ipc<T>(cmd, params as Record<string, unknown>);
        return result;
      } catch (e) {
        const msg = String(e);
        setError(msg);
        return undefined;
      } finally {
        setLoading(false);
      }
    },
    [cmd, isTauri],
  );

  return { mutate, loading, error };
}
