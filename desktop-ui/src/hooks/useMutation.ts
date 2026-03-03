import { useState, useCallback } from 'react';
import { ipc } from './useIpc';
import { isTauri, parseApiError } from '../lib/utils';
import type { ApiError } from '../lib/types';

interface MutationResult<T, P> {
  mutate: (params: P) => Promise<T | undefined>;
  loading: boolean;
  error: ApiError | null;
}

/**
 * Wraps a Tauri command for write operations.
 * Returns `undefined` in browser dev mode.
 */
export function useMutation<T = void, P = Record<string, unknown>>(
  cmd: string,
): MutationResult<T, P> {
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<ApiError | null>(null);

  const mutate = useCallback(
    async (params: P): Promise<T | undefined> => {
      if (!isTauri) return undefined;

      setLoading(true);
      setError(null);
      try {
        const result = await ipc<T>(cmd, params as Record<string, unknown>);
        return result;
      } catch (e) {
        setError(parseApiError(e));
        return undefined;
      } finally {
        setLoading(false);
      }
    },
    [cmd],
  );

  return { mutate, loading, error };
}
