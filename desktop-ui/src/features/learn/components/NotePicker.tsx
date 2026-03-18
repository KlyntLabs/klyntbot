import { ipc } from "@shared/hooks/useIpc";
import type { Note } from "@shared/types";
import { FileText, Search } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";

interface NotePickerProps {
  onSelect: (note: { id: string; title: string }) => void;
  onCancel: () => void;
}

export function NotePicker({ onSelect, onCancel }: NotePickerProps) {
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<Note[]>([]);
  const [loading, setLoading] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        onCancel();
      }
    };
    document.addEventListener("keydown", handler);
    return () => document.removeEventListener("keydown", handler);
  }, [onCancel]);

  const requestIdRef = useRef(0);
  const search = useCallback(async (q: string) => {
    if (q.trim().length < 2) {
      setResults([]);
      return;
    }
    const id = ++requestIdRef.current;
    setLoading(true);
    try {
      const notes = await ipc<Note[]>("note_search", { query: q });
      if (id !== requestIdRef.current) return; // stale response
      setResults(notes.slice(0, 10));
    } catch {
      if (id !== requestIdRef.current) return;
      setResults([]);
    } finally {
      if (id === requestIdRef.current) setLoading(false);
    }
  }, []);

  useEffect(() => {
    const timer = setTimeout(() => search(query), 200);
    return () => clearTimeout(timer);
  }, [query, search]);

  return (
    <div className="space-y-2">
      <div className="relative">
        <Search
          size={14}
          className="absolute left-2.5 top-1/2 -translate-y-1/2 text-muted-foreground"
        />
        <input
          ref={inputRef}
          type="text"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder="Search notes..."
          className="w-full bg-muted/50 rounded-lg pl-8 pr-3 py-2 text-sm text-foreground placeholder:text-dim"
        />
      </div>

      {results.length > 0 && (
        <div className="max-h-48 overflow-y-auto space-y-0.5">
          {results.map((note) => (
            <button
              key={note.id}
              type="button"
              onClick={() => onSelect({ id: note.id, title: note.title })}
              className="w-full flex items-center gap-2 px-3 py-2 rounded-lg text-left text-sm text-foreground hover:bg-accent transition-colors"
            >
              <FileText size={14} className="text-muted-foreground flex-shrink-0" />
              <span className="truncate">{note.title}</span>
            </button>
          ))}
        </div>
      )}

      {query.length >= 2 && !loading && results.length === 0 && (
        <p className="text-[12px] text-muted-foreground text-center py-2">No notes found</p>
      )}
    </div>
  );
}
