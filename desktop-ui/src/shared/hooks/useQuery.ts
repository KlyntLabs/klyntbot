import { isTauri } from "@shared/lib/utils";
import type { ApiError } from "@shared/types";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useCallback, useEffect, useRef, useState } from "react";
import { parseApiError } from "../lib/errors";
import { ipc } from "./useIpc";

interface QueryResult<T> {
  data: T;
  loading: boolean;
  error: ApiError | null;
  refetch: () => void;
}

interface UseQueryOptions {
  /** Stale time in ms before auto-refetch. Default 30s. */
  staleTime?: number;
  /** Event names that trigger cache invalidation and refetch. */
  invalidateOn?: string[];
  /** Only invalidate if the event payload passes this filter. */
  invalidateFilter?: (payload: unknown) => boolean;
}

interface CacheEntry {
  data: unknown;
  timestamp: number;
  promise?: Promise<unknown>;
}

const cache = new Map<string, CacheEntry>();
const DEFAULT_STALE_TIME = 30_000;
const MAX_CACHE_ENTRIES = 50;
const CACHE_TTL = 60_000; // 1 minute

/** Evict stale/excess entries. Only does work when cache is over the high-water mark. */
function evictCache() {
  if (cache.size <= MAX_CACHE_ENTRIES) return;

  const now = Date.now();
  for (const [key, entry] of cache) {
    if (now - entry.timestamp > CACHE_TTL) cache.delete(key);
  }
  if (cache.size <= MAX_CACHE_ENTRIES) return;

  // Still over limit — evict oldest down to 75% capacity to avoid sorting on every insert
  const target = Math.floor(MAX_CACHE_ENTRIES * 0.75);
  const sorted = [...cache.entries()].sort((a, b) => a[1].timestamp - b[1].timestamp);
  const toRemove = cache.size - target;
  for (let i = 0; i < toRemove; i++) {
    cache.delete(sorted[i][0]);
  }
}

// Listener set for cache invalidation — hooks subscribe to get notified when their cache entry is cleared.
const invalidationListeners = new Set<(prefix: string) => void>();

// Per-key subscribers for live data push (optimistic updates).
const dataSubscribers = new Map<string, Set<(data: unknown) => void>>();

/**
 * Imperatively update cache entries whose key starts with `cmdPrefix` and
 * notify all mounted hooks reading that key. Use for optimistic updates.
 */
export function updateQueryCache<T>(cmdPrefix: string, updater: (data: T) => T) {
  for (const [k, entry] of cache) {
    if (!k.startsWith(cmdPrefix) || entry.data === undefined) continue;
    const next = updater(entry.data as T);
    if (next === entry.data) continue;
    cache.set(k, { ...entry, data: next, timestamp: Date.now() });
    dataSubscribers.get(k)?.forEach((cb) => cb(next));
  }
}

function cacheKey(cmd: string, args?: Record<string, unknown> | null): string | null {
  if (args === null) return null;
  return args === undefined ? cmd : `${cmd}:${JSON.stringify(args)}`;
}

/**
 * Listen for multiple events and call handler on any match.
 * Debounces via requestAnimationFrame to coalesce batch mutations.
 */
function useEventInvalidation(events: string[] | undefined, handler: (payload: unknown) => void) {
  const handlerRef = useRef(handler);
  handlerRef.current = handler;
  const rafRef = useRef(0);

  const debouncedHandler = useCallback((payload: unknown) => {
    cancelAnimationFrame(rafRef.current);
    rafRef.current = requestAnimationFrame(() => {
      handlerRef.current(payload);
    });
  }, []);

  const eventsRef = useRef(events);
  eventsRef.current = events;
  // Re-subscribe when event names actually change (not on every render's new array reference)
  const eventsKey = events?.join(",") ?? "";

  // biome-ignore lint/correctness/useExhaustiveDependencies: eventsKey tracks content changes; eventsRef avoids array identity churn
  useEffect(() => {
    const ev = eventsRef.current;
    if (!ev || ev.length === 0) return;

    const listeners: (() => void)[] = [];

    if (isTauri) {
      for (const event of ev) {
        let cancelled = false;
        let unlisten: UnlistenFn | undefined;
        listen(event, (e: { payload: unknown }) => {
          if (!cancelled) debouncedHandler(e.payload);
        }).then((fn) => {
          if (cancelled) fn();
          else unlisten = fn;
        });
        listeners.push(() => {
          cancelled = true;
          unlisten?.();
        });
      }
    } else {
      for (const event of ev) {
        const onCustom = (e: Event) => {
          debouncedHandler((e as CustomEvent).detail);
        };
        window.addEventListener(event, onCustom);
        listeners.push(() => window.removeEventListener(event, onCustom));
      }
    }

    return () => {
      cancelAnimationFrame(rafRef.current);
      for (const cleanup of listeners) cleanup();
    };
  }, [debouncedHandler, eventsKey]);
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
  options?: UseQueryOptions | number,
): QueryResult<T> {
  // Normalize options — number = legacy staleTime parameter
  const opts: UseQueryOptions =
    typeof options === "number" ? { staleTime: options } : (options ?? {});
  const staleTime = opts.staleTime ?? DEFAULT_STALE_TIME;

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
          const prev = cache.get(k)?.data;
          cache.set(k, { data: result, timestamp: Date.now() });
          evictCache();
          // Skip setState if the refetched value is reference-identical to the
          // cached value (e.g. after an optimistic update wrote the same shape).
          if (result !== prev) setData(result);
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

  // Subscribe to imperative cache updates (optimistic updates)
  useEffect(() => {
    const k = argsKey;
    if (!k) return;
    const listener = (d: unknown) => setData(d as T);
    let set = dataSubscribers.get(k);
    if (!set) {
      set = new Set();
      dataSubscribers.set(k, set);
    }
    set.add(listener);
    return () => {
      set?.delete(listener);
      if (set?.size === 0) dataSubscribers.delete(k);
    };
  }, [argsKey]);

  // Re-fetch when cache is externally invalidated for this command
  useEffect(() => {
    const listener = (prefix: string) => {
      if (cmd.startsWith(prefix)) doFetch(true);
    };
    invalidationListeners.add(listener);
    return () => {
      invalidationListeners.delete(listener);
    };
  }, [cmd, doFetch]);

  // Event-driven invalidation — refetch when specified events fire
  const invalidateFilterRef = useRef(opts.invalidateFilter);
  invalidateFilterRef.current = opts.invalidateFilter;

  useEventInvalidation(opts.invalidateOn, (payload: unknown) => {
    if (invalidateFilterRef.current && !invalidateFilterRef.current(payload)) return;
    doFetch(true);
  });

  return { data, loading, error, refetch };
}

/** Invalidate all cache entries matching a command prefix and notify mounted hooks. */
export function invalidateQueries(cmdPrefix: string) {
  for (const k of cache.keys()) {
    if (k.startsWith(cmdPrefix)) cache.delete(k);
  }
  for (const listener of invalidationListeners) {
    listener(cmdPrefix);
  }
}
