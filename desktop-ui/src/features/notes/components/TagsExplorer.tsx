import { tagBgColor, tagColor } from "@shared/lib/tagColor";
import type { NoteListItem } from "@shared/types";
import { useMemo } from "react";

interface TagsExplorerProps {
  notes: NoteListItem[];
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
        <span className="text-ui-xs uppercase tracking-wider text-fg-dim">Tags</span>
        {activeTags.length > 0 && (
          <button
            type="button"
            onClick={onClearTags}
            className="text-ui-xs text-fg-secondary hover:text-fg"
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
              className="inline-flex items-center gap-1 px-1.5 py-0.5 rounded-full text-ui-xs transition-opacity"
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
          <div className="h-px bg-bg-elevated mx-2" />
          {filteredNotes.map((note) => (
            <button
              type="button"
              key={note.id}
              onClick={() => onSelectNote(note.id)}
              className={`px-2 py-1 rounded text-sm truncate text-left w-full transition-colors ${
                note.id === selectedNoteId
                  ? "bg-control-hover text-fg"
                  : "text-fg-secondary hover:bg-bg-elevated"
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
