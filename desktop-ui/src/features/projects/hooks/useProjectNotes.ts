import { ipc } from "@shared/hooks/useIpc";
import type { Note } from "@shared/types";
import { useCallback, useEffect, useState } from "react";

/**
 * Fetches notes from multiple notebooks (one call per notebook ID),
 * merges and deduplicates client-side.
 */
export function useProjectNotes(notebookIds: string[]) {
  const [data, setData] = useState<Note[]>([]);
  const [loading, setLoading] = useState(true);

  // Stable key for dependency tracking — avoids stale closure from array reference changes
  const idsKey = notebookIds.join(",");

  const fetchAll = useCallback(async () => {
    const ids = idsKey.split(",").filter(Boolean);
    if (ids.length === 0) {
      setData([]);
      setLoading(false);
      return;
    }
    setLoading(true);
    try {
      const results = await Promise.all(
        ids.map((notebookId) => ipc<Note[]>("note_list", { notebookId })),
      );
      const merged = results.flat();
      const seen = new Set<string>();
      const deduped = merged.filter((n) => {
        if (seen.has(n.id)) return false;
        seen.add(n.id);
        return true;
      });
      deduped.sort((a, b) => (b.updatedAt ?? "").localeCompare(a.updatedAt ?? ""));
      setData(deduped);
    } finally {
      setLoading(false);
    }
  }, [idsKey]);

  useEffect(() => {
    fetchAll();
  }, [fetchAll]);

  return { data, loading, refetch: fetchAll };
}
