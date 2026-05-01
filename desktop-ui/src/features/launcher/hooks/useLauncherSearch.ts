import { useQueryClient } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import { qk, useTauriQuery } from "@/lib/query";
import { isTauri } from "@/utils/tauri-bridge";
import { useLauncherApi, useLauncherState } from "../store";
import type { LauncherItem } from "../types";

const DEBOUNCE_MS = 16; // one frame for responsive typing

export function useLauncherSearch() {
  const query = useLauncherState((s) => s.query);
  const { setResults, setIsSearching } = useLauncherApi();
  const queryClient = useQueryClient();

  const debounced = useDebounced(query, DEBOUNCE_MS);

  const search = useTauriQuery<LauncherItem[]>({
    queryKey: qk.launcher.search(debounced),
    command: "launcher_search",
    args: { query: debounced },
    fallback: [],
    staleTime: 5_000,
  });

  useEffect(() => {
    setIsSearching(search.isFetching);
  }, [search.isFetching, setIsSearching]);

  useEffect(() => {
    if (search.data) setResults(search.data);
  }, [search.data, setResults]);

  // Refetch frequents on every launcher show — first-open may race against
  // backend index population (AppIndex, attention table), and stale empty
  // results would otherwise stick until the cache TTL expires.
  useEffect(() => {
    if (!isTauri()) return;
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    (async () => {
      const { getCurrentWindow } = await import("@tauri-apps/api/window");
      const win = getCurrentWindow();
      const off = await win.listen("window-shown", () => {
        // The store's results are cleared on hide via reset(); restore from the
        // React Query cache synchronously so frequents reappear immediately.
        const cached = queryClient.getQueryData<LauncherItem[]>(
          qk.launcher.search(""),
        );
        if (cached && cached.length > 0) setResults(cached);
        queryClient.invalidateQueries({ queryKey: qk.launcher.search("") });
      });
      if (cancelled) off();
      else unlisten = off;
    })();
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [queryClient, setResults]);
}

function useDebounced<T>(value: T, ms: number): T {
  const [v, setV] = useState(value);
  useEffect(() => {
    const t = setTimeout(() => setV(value), ms);
    return () => clearTimeout(t);
  }, [value, ms]);
  return v;
}
