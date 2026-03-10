import { Search, X } from "lucide-react";
import type { Ref } from "react";
import { useCallback, useEffect, useImperativeHandle, useRef, useState } from "react";
import { ipc } from "@shared/hooks/useIpc";
import type { Note } from "@shared/types";

export interface NoteSearchBarHandle {
  focus: () => void;
}

interface NoteSearchBarProps {
  onResults: (results: Note[] | null) => void;
  ref?: Ref<NoteSearchBarHandle>;
}

export function NoteSearchBar({ onResults, ref }: NoteSearchBarProps) {
  const [query, setQuery] = useState("");
  const inputRef = useRef<HTMLInputElement>(null);
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useImperativeHandle(ref, () => ({
    focus: () => inputRef.current?.focus(),
  }));

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
        } catch (e) {
          console.warn("Note search failed:", e);
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
    <div className="relative glass-sidebar">
      <Search className="absolute left-2.5 top-1/2 -translate-y-1/2 w-3.5 h-3.5 text-dim" />
      <input
        ref={inputRef}
        type="text"
        placeholder="Search all notes..."
        value={query}
        onChange={(e) => setQuery(e.target.value)}
        className="w-full text-xs bg-transparent border-none rounded-lg pl-8 pr-7 py-1.5 text-primary placeholder:text-dim focus:outline-none"
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
