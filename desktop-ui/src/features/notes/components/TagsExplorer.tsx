import { tagBgColor, tagColor } from "@shared/lib/tagColor";
import type { Note } from "@shared/types";
import { useMemo } from "react";

interface TagsExplorerProps {
  notes: Note[];
  activeTags: string[];
  selectedNoteId: string | null;
  onToggleTag: (tag: string, additive: boolean) => void;
  onClearTags: () => void;
  onSelectNote: (id: string) => void;
}

export function TagsExplorer({
  notes,
  activeTags,
  selectedNoteId,
  onToggleTag,
  onClearTags,
  onSelectNote,
}: TagsExplorerProps) {
  // Compute tag counts from notes
  const tagCounts = useMemo(() => {
    const counts = new Map<string, number>();
    for (const note of notes) {
      for (const tag of note.tags) {
        counts.set(tag, (counts.get(tag) || 0) + 1);
      }
    }
    // Sort by count descending, then alphabetically
    return Array.from(counts.entries()).sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0]));
  }, [notes]);

  // Filter notes by active tags (AND logic)
  const filteredNotes = useMemo(() => {
    if (activeTags.length === 0) return [];
    return notes.filter((n) => activeTags.every((t) => n.tags.includes(t)));
  }, [notes, activeTags]);

  if (tagCounts.length === 0) return null;

  return (
    <div className="flex flex-col gap-1">
      <div className="flex items-center justify-between px-2 py-1">
        <span className="text-[10px] uppercase tracking-wider text-dim">Tags</span>
        {activeTags.length > 0 && (
          <button
            type="button"
            onClick={onClearTags}
            className="text-[10px] text-muted hover:text-secondary"
          >
            Clear
          </button>
        )}
      </div>

      <div className="flex flex-wrap gap-1 px-2">
        {tagCounts.map(([tag, count]) => {
          const isActive = activeTags.includes(tag);
          return (
            <button
              type="button"
              key={tag}
              onClick={(e) => onToggleTag(tag, e.metaKey || e.ctrlKey)}
              className="inline-flex items-center gap-1 px-1.5 py-0.5 rounded-full text-[11px] transition-opacity"
              style={{
                color: tagColor(tag),
                backgroundColor: tagBgColor(tag),
                opacity: activeTags.length > 0 && !isActive ? 0.4 : 1,
                outline: isActive ? `1px solid ${tagColor(tag)}` : "none",
              }}
              title={`${tag} (${count})`}
            >
              <span className="truncate max-w-[100px]">{tag}</span>
              <span className="opacity-60">{count}</span>
            </button>
          );
        })}
      </div>

      {/* Filtered notes list */}
      {filteredNotes.length > 0 && (
        <div className="flex flex-col gap-0.5 mt-1">
          <div className="h-px bg-white/[0.04] mx-2" />
          {filteredNotes.map((note) => (
            <button
              type="button"
              key={note.id}
              onClick={() => onSelectNote(note.id)}
              className={`px-2 py-1 rounded text-sm truncate text-left w-full transition-colors ${
                note.id === selectedNoteId
                  ? "bg-white/[0.08] text-primary"
                  : "text-secondary hover:bg-white/[0.04]"
              }`}
            >
              {note.title || "Untitled"}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
