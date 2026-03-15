import { ipc } from "@shared/hooks/useIpc";
import { useEffect, useRef } from "react";
import { useLauncherStore } from "../stores/launcherStore";
import type { LauncherItem } from "../types";

export function useLauncherSearch() {
  const query = useLauncherStore((s) => s.query);
  const setResults = useLauncherStore((s) => s.setResults);
  const timerRef = useRef<ReturnType<typeof setTimeout>>(undefined);

  useEffect(() => {
    if (!query.trim()) {
      setResults([]);
      return;
    }

    clearTimeout(timerRef.current);
    timerRef.current = setTimeout(async () => {
      try {
        const results = await ipc<LauncherItem[]>("launcher_search", { query });
        setResults(results);
      } catch (e) {
        console.error("Launcher search failed:", e);
      }
    }, 100);

    return () => clearTimeout(timerRef.current);
  }, [query, setResults]);
}
