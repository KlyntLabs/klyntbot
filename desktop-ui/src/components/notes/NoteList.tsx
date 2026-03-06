import { Plus, Search } from "lucide-react";
import { useMemo, useState } from "react";
import type { Note } from "../../lib/types";
import { NoteCard } from "./NoteCard";

interface NoteListProps {
  notes: Note[];
  selectedId: string | null;
  onSelect: (id: string) => void;
  onCreate: () => void;
}

export function NoteList({ notes, selectedId, onSelect, onCreate }: NoteListProps) {
  const [search, setSearch] = useState("");

  const sorted = useMemo(() => {
    const q = search.toLowerCase();
    const filtered = q
      ? notes.filter((n) => n.title.toLowerCase().includes(q) || n.body.toLowerCase().includes(q))
      : notes;

    return [...filtered].sort((a, b) => {
      if (a.pinned !== b.pinned) return a.pinned ? -1 : 1;
      return b.updatedAt > a.updatedAt ? 1 : b.updatedAt < a.updatedAt ? -1 : 0;
    });
  }, [notes, search]);

  return (
    <div className="w-64 glass-panel rounded-2xl p-3 flex flex-col gap-2">
      {/* Header */}
      <div className="flex items-center justify-between">
        <h2 className="text-xs font-medium text-muted uppercase tracking-wider">
          Notes ({notes.length})
        </h2>
        <button
          type="button"
          onClick={onCreate}
          className="w-6 h-6 rounded-lg flex items-center justify-center text-muted hover:text-primary hover:bg-white/[0.06] transition-colors"
          aria-label="New note"
        >
          <Plus className="w-4 h-4" />
        </button>
      </div>

      {/* Search */}
      <div className="relative">
        <Search className="absolute left-2 top-1/2 -translate-y-1/2 w-3.5 h-3.5 text-dim" />
        <input
          type="text"
          placeholder="Search notes..."
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          className="w-full text-xs bg-white/[0.04] border border-border rounded-lg pl-7 pr-2 py-1.5 text-primary placeholder:text-dim focus:outline-none focus:ring-1 focus:ring-brand/30"
        />
      </div>

      {/* Note list */}
      <div className="flex-1 overflow-y-auto flex flex-col gap-0.5 min-h-0">
        {sorted.map((note) => (
          <NoteCard
            key={note.id}
            note={note}
            selected={note.id === selectedId}
            onSelect={onSelect}
          />
        ))}
        {sorted.length === 0 && (
          <div className="text-xs text-dim text-center py-4">
            {search ? "No matching notes" : "No notes yet"}
          </div>
        )}
      </div>
    </div>
  );
}
