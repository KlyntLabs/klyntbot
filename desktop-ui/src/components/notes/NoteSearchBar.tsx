import { Search, X } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { ipc } from "../../hooks/useIpc";
import type { Note } from "../../lib/types";

interface NoteSearchBarProps {
  onResults: (results: Note[] | null) => void;
}

export function NoteSearchBar({ onResults }: NoteSearchBarProps) {
  const [query, setQuery] = useState("");
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const doSearch = useCallback(
    (q: string) => {
      if (debounceRef.current) clearTimeout(debounceRef.current);
      if (!q.trim()) {
        onResults(null);
        return;
      }
      debounceRef.current = setTimeout(async () => {
        try {
          const results = await ipc<Note[]>("note_search", { query: q.trim() });
          onResults(results);
        } catch {
          onResults(null);
        }
      }, 200);
    },
    [onResults],
  );

  useEffect(() => {
    doSearch(query);
    return () => {
      if (debounceRef.current) clearTimeout(debounceRef.current);
    };
  }, [query, doSearch]);

  const handleClear = useCallback(() => {
    setQuery("");
    onResults(null);
  }, [onResults]);

  return (
    <div className="relative">
      <Search className="absolute left-2.5 top-1/2 -translate-y-1/2 w-3.5 h-3.5 text-dim" />
      <input
        type="text"
        placeholder="Search all notes..."
        value={query}
        onChange={(e) => setQuery(e.target.value)}
        className="w-full text-xs bg-white/[0.04] border border-border rounded-lg pl-8 pr-7 py-1.5 text-primary placeholder:text-dim focus:outline-none focus:ring-1 focus:ring-brand/30"
      />
      {query && (
        <button
          type="button"
          onClick={handleClear}
          className="absolute right-2 top-1/2 -translate-y-1/2 text-dim hover:text-secondary"
          aria-label="Clear search"
        >
          <X className="w-3.5 h-3.5" />
        </button>
      )}
    </div>
  );
}
