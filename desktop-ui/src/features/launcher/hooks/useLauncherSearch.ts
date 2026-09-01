import { ipc } from "@shared/hooks/useIpc";
import { useEffect, useRef } from "react";
import { useLauncherStore } from "../stores/launcherStore";
import type { LauncherItem } from "../types";

export function useLauncherSearch() {
  const query = useLauncherStore((s) => s.query);
  const setResults = useLauncherStore((s) => s.setResults);
  const setIsSearching = useLauncherStore((s) => s.setIsSearching);
  const timerRef = useRef<ReturnType<typeof setTimeout>>(undefined);
  const versionRef = useRef(0);

  // Fetch default view on mount (empty query = top frecency items)
  useEffect(() => {
    ipc<LauncherItem[]>("launcher_search", { query: "" })
      .then(setResults)
      .catch(() => {});
  }, [setResults]);

  useEffect(() => {
    if (!query.trim()) {
      // Refetch defaults when query is cleared
      const v = ++versionRef.current;
      ipc<LauncherItem[]>("launcher_search", { query: "" })
        .then((results) => {
          if (versionRef.current === v) setResults(results);
        })
        .catch(() => {});
      return;
    }

    setIsSearching(true);
    clearTimeout(timerRef.current);

    // Cancel-on-keystroke: increment version so stale responses are discarded
    const version = ++versionRef.current;

    timerRef.current = setTimeout(async () => {
      try {
        const results = await ipc<LauncherItem[]>("launcher_search", { query });
        if (versionRef.current === version) {
          setResults(results);
          setIsSearching(false);
        }
      } catch (e) {
        if (versionRef.current === version) {
          console.error("Launcher search failed:", e);
          setResults([]);
          setIsSearching(false);
        }
      }
    }, 30);

    return () => clearTimeout(timerRef.current);
  }, [query, setResults, setIsSearching]);
}
