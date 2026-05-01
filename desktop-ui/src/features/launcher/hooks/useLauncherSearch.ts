import { useEffect, useState } from "react";
import { qk, useTauriQuery } from "@/lib/query";
import { useLauncherApi, useLauncherState } from "../store";
import type { LauncherItem } from "../types";

const DEBOUNCE_MS = 16; // one frame for responsive typing

export function useLauncherSearch() {
  const query = useLauncherState((s) => s.query);
  const { setResults, setIsSearching } = useLauncherApi();

  // Debounce the raw query so we don't fire 1 query per keystroke. The
  // queryKey change cancels the in-flight TQ fetch automatically.
  const debounced = useDebounced(query, DEBOUNCE_MS);

  const isEmpty = debounced.trim().length === 0;

  const search = useTauriQuery<LauncherItem[]>({
    queryKey: qk.launcher.search(debounced),
    command: "launcher_search",
    args: { query: debounced },
    fallback: [],
    enabled: !isEmpty,
    // Search results are inherently stale-fast; tighter than the global
    // 30s default so an exact-string repeat within ~5s reuses the cache.
    staleTime: 5_000,
  });

  useEffect(() => {
    setIsSearching(!isEmpty && search.isFetching);
  }, [isEmpty, search.isFetching, setIsSearching]);

  useEffect(() => {
    if (isEmpty) {
      setResults([]);
      return;
    }
    if (search.data) setResults(search.data);
  }, [isEmpty, search.data, setResults]);
}

function useDebounced<T>(value: T, ms: number): T {
  const [v, setV] = useState(value);
  useEffect(() => {
    const t = setTimeout(() => setV(value), ms);
    return () => clearTimeout(t);
  }, [value, ms]);
  return v;
}
