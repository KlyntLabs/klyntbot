/**
 * REST data fetching hook.
 *
 * Exposes: data, loading, error, refetch
 * Uses AbortController to cancel in-flight fetches on unmount or refetch.
 * Shows error state after 5 seconds without data.
 */

import { useState, useEffect, useCallback, useRef } from 'react';
import { apiFetch, ApiError, type FetchParams } from '../api';

export interface UseApiOptions {
  params?: FetchParams;
  /** Skip the initial fetch if true */
  skip?: boolean;
}

export interface UseApiResult<T> {
  data: T | undefined;
  loading: boolean;
  error: ApiError | Error | null;
  refetch: () => void;
}

const TIMEOUT_MS = 5000;

export function useApi<T>(
  url: string,
  options: UseApiOptions = {},
): UseApiResult<T> {
  const [data, setData] = useState<T | undefined>(undefined);
  const [loading, setLoading] = useState(!options.skip);
  const [error, setError] = useState<ApiError | Error | null>(null);

  // Stable ref for latest params to avoid effect re-runs on object identity change
  const paramsRef = useRef(options.params);
  paramsRef.current = options.params;

  const fetchCountRef = useRef(0);

  const fetchData = useCallback(
    (abortSignal: AbortSignal) => {
      setLoading(true);

      const timeoutId = setTimeout(() => {
        if (!abortSignal.aborted) {
          setLoading(false);
          setError(new Error('Request timed out after 5 seconds'));
        }
      }, TIMEOUT_MS);

      apiFetch<T>(url, { params: paramsRef.current, signal: abortSignal })
        .then((result) => {
          if (!abortSignal.aborted) {
            setData(result);
            setError(null);
            setLoading(false);
          }
        })
        .catch((err: unknown) => {
          if (!abortSignal.aborted) {
            if (err instanceof Error && err.name === 'AbortError') {
              // Fetch was intentionally aborted — do not update state
              return;
            }
            setError(err instanceof Error ? err : new Error(String(err)));
            setLoading(false);
          }
        })
        .finally(() => {
          clearTimeout(timeoutId);
        });
    },
    [url],
  );

  const refetchTrigger = useRef(0);

  const refetch = useCallback(() => {
    fetchCountRef.current += 1;
    refetchTrigger.current = fetchCountRef.current;
    // Force re-render to trigger the effect
    setLoading(true);
  }, []);

  useEffect(() => {
    if (options.skip) return;

    const controller = new AbortController();
    fetchData(controller.signal);

    return () => {
      controller.abort();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [url, fetchData, refetchTrigger.current, options.skip]);

  return { data, loading, error, refetch };
}
