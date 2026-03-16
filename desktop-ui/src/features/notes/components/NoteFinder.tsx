import { ipc } from "@shared/hooks/useIpc";
import { formatDate } from "@shared/lib/dates";
import type { Note } from "@shared/types";
import { FileText, Search } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";

interface HybridSearchResult {
  exact: Note[];
  related: Note[];
}

interface NoteFinderProps {
  isOpen: boolean;
  onClose: () => void;
  onSelectNote: (id: string) => void;
  notes: Note[];
}

export function NoteFinder({ isOpen, onClose, onSelectNote, notes }: NoteFinderProps) {
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<Note[]>(notes);
  const [selectedIndex, setSelectedIndex] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLDivElement>(null);
  const debounceRef = useRef<ReturnType<typeof setTimeout>>();

  useEffect(() => {
    if (isOpen) {
      setQuery("");
      setResults(notes);
      setSelectedIndex(0);
      requestAnimationFrame(() => inputRef.current?.focus());
    }
  }, [isOpen, notes]);

  useEffect(() => {
    if (!isOpen) return;
    if (debounceRef.current) clearTimeout(debounceRef.current);

    if (!query.trim()) {
      setResults(notes);
      setSelectedIndex(0);
      return;
    }

    debounceRef.current = setTimeout(async () => {
      try {
        const res = await ipc<HybridSearchResult>("note_search_hybrid", {
          query: query.trim(),
        });
        const seenIds = new Set(res.exact.map((n) => n.id));
        const unique = [...res.exact, ...res.related.filter((n) => !seenIds.has(n.id))];
        setResults(unique);
        setSelectedIndex(0);
      } catch {
        setResults([]);
      }
    }, 200);

    return () => {
      if (debounceRef.current) clearTimeout(debounceRef.current);
    };
  }, [query, isOpen, notes]);

  useEffect(() => {
    if (!listRef.current) return;
    const items = listRef.current.children;
    if (items[selectedIndex]) {
      (items[selectedIndex] as HTMLElement).scrollIntoView({ block: "nearest" });
    }
  }, [selectedIndex]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if ((e.key === "j" && e.ctrlKey) || e.key === "ArrowDown") {
        e.preventDefault();
        setSelectedIndex((prev) => Math.min(prev + 1, results.length - 1));
        return;
      }
      if ((e.key === "k" && e.ctrlKey) || e.key === "ArrowUp") {
        e.preventDefault();
        setSelectedIndex((prev) => Math.max(prev - 1, 0));
        return;
      }
      if (e.key === "Enter" && results.length > 0) {
        e.preventDefault();
        onSelectNote(results[selectedIndex].id);
        onClose();
        return;
      }
      if (e.key === "Escape") {
        e.preventDefault();
        onClose();
        return;
      }
    },
    [results, selectedIndex, onSelectNote, onClose],
  );

  if (!isOpen) return null;

  const selectedNote = results[selectedIndex] ?? null;

  return (
    <div
      className="fixed inset-0 z-50 flex items-start justify-center pt-[10vh]"
      style={{ background: "rgba(0, 0, 0, 0.5)" }}
      onClick={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
      onKeyDown={handleKeyDown}
    >
      {/* Modal — glass-floating like launcher/tray */}
      <div className="glass-floating w-[900px] max-w-[92vw] h-[600px] max-h-[75vh] overflow-hidden flex flex-col">
        {/* Search input */}
        <div className="flex items-center gap-2.5 px-4 py-2.5 border-b border-white/[0.08]">
          <Search className="w-4 h-4 text-muted shrink-0" />
          <input
            ref={inputRef}
            type="text"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Search notes..."
            className="flex-1 bg-transparent text-[13px] text-primary placeholder:text-muted/50 focus:outline-none"
          />
          <span className="text-[10px] text-muted shrink-0">
            {results.length}/{notes.length}
          </span>
        </div>

        {/* Two-pane */}
        <div className="flex flex-1 min-h-0">
          {/* Left: compact results list */}
          <div ref={listRef} className="w-[38%] border-r border-white/[0.06] overflow-y-auto">
            {results.length === 0 ? (
              <div className="text-[11px] text-muted text-center py-8">No matches</div>
            ) : (
              results.map((note, i) => (
                <button
                  key={note.id}
                  type="button"
                  onClick={() => {
                    onSelectNote(note.id);
                    onClose();
                  }}
                  onMouseEnter={() => setSelectedIndex(i)}
                  className={`w-full flex items-center gap-2 px-3 py-1.5 text-left transition-colors ${
                    i === selectedIndex
                      ? "bg-white/[0.08] text-primary"
                      : "text-secondary hover:bg-white/[0.03]"
                  }`}
                >
                  <FileText className="w-3 h-3 shrink-0 text-muted/60" />
                  <span className="truncate text-[11px] flex-1">{note.title}</span>
                  <span className="text-[9px] text-muted/40 shrink-0">
                    {note.updatedAt ? formatDate(note.updatedAt.slice(0, 10)) : ""}
                  </span>
                </button>
              ))
            )}
          </div>

          {/* Right: preview */}
          <div className="w-[62%] overflow-y-auto px-5 py-4">
            {selectedNote ? (
              <>
                <div className="text-[12px] font-medium text-primary/90 mb-1">
                  {selectedNote.title}
                </div>
                {selectedNote.tags.length > 0 && (
                  <div className="flex gap-1 flex-wrap mb-2">
                    {selectedNote.tags.map((tag) => (
                      <span
                        key={tag}
                        className="text-[9px] px-1.5 py-0.5 rounded-full bg-white/[0.06] text-muted"
                      >
                        {tag}
                      </span>
                    ))}
                  </div>
                )}
                <div className="text-[11px] text-secondary/80 leading-relaxed whitespace-pre-wrap">
                  {selectedNote.body || "No content"}
                </div>
              </>
            ) : (
              <div className="text-[11px] text-muted text-center py-8">No preview</div>
            )}
          </div>
        </div>

        {/* Footer */}
        <div className="flex items-center gap-4 px-4 py-1.5 border-t border-white/[0.06] text-[9px] text-muted/50">
          <span>
            <kbd className="px-1 py-0.5 rounded bg-white/[0.05] text-[8px]">↑↓</kbd> navigate
          </span>
          <span>
            <kbd className="px-1 py-0.5 rounded bg-white/[0.05] text-[8px]">Enter</kbd> open
          </span>
          <span>
            <kbd className="px-1 py-0.5 rounded bg-white/[0.05] text-[8px]">Esc</kbd> close
          </span>
          <span>
            <kbd className="px-1 py-0.5 rounded bg-white/[0.05] text-[8px]">^J/K</kbd> vim
          </span>
        </div>
      </div>
    </div>
  );
}
