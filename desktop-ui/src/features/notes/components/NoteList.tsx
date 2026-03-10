import type { Note } from "@shared/types";
import { Plus, X } from "lucide-react";
import { useMemo, useState } from "react";
import { NoteCard } from "./NoteCard";

interface NoteListProps {
  notes: Note[];
  selectedId: string | null;
  onSelect: (id: string) => void;
  onCreate: () => void;
  onPin: (id: string, pinned: boolean) => void;
  onDelete: (id: string) => void;
}

export function NoteList({
  notes,
  selectedId,
  onSelect,
  onCreate,
  onPin,
  onDelete,
}: NoteListProps) {
  const [activeTag, setActiveTag] = useState<string | null>(null);

  // Collect all unique tags from notes for the filter chips
  const allTags = useMemo(() => {
    const tagSet = new Set<string>();
    for (const note of notes) {
      for (const tag of note.tags) {
        tagSet.add(tag);
      }
    }
    return Array.from(tagSet).sort();
  }, [notes]);

  const sorted = useMemo(() => {
    const filtered = activeTag ? notes.filter((n) => n.tags.includes(activeTag)) : notes;

    return [...filtered].sort((a, b) => {
      if (a.pinned !== b.pinned) return a.pinned ? -1 : 1;
      return b.updatedAt > a.updatedAt ? 1 : b.updatedAt < a.updatedAt ? -1 : 0;
    });
  }, [notes, activeTag]);

  return (
    <div className="flex-1 glass-panel rounded-2xl p-3 flex flex-col gap-2 min-h-0">
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

      {/* Tag filter */}
      {allTags.length > 0 && (
        <div className="flex gap-1 flex-wrap">
          {allTags.map((tag) => (
            <button
              key={tag}
              type="button"
              onClick={() => setActiveTag(activeTag === tag ? null : tag)}
              className={`tag-pill ${activeTag === tag ? "tag-pill-active" : ""}`}
            >
              {tag}
              {activeTag === tag && <X className="w-2.5 h-2.5 inline ml-0.5" />}
            </button>
          ))}
        </div>
      )}

      {/* Note list */}
      <div
        className="flex-1 overflow-y-auto flex flex-col gap-0.5 min-h-0"
        style={{ contentVisibility: "auto" }}
      >
        {sorted.map((note) => (
          <NoteCard
            key={note.id}
            note={note}
            selected={note.id === selectedId}
            onSelect={onSelect}
            onPin={onPin}
            onDelete={onDelete}
          />
        ))}
        {sorted.length === 0 && (
          <div className="text-xs text-dim text-center py-4">No notes yet</div>
        )}
      </div>
    </div>
  );
}
