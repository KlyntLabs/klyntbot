import type { Note } from "@shared/types";
import { Clock, Pin } from "lucide-react";
import { useMemo } from "react";

interface QuickAccessListProps {
  notes: Note[];
  selectedNoteId: string | null;
  onSelectNote: (id: string) => void;
  onPinNote: (id: string, pinned: boolean) => void;
}

export function QuickAccessList({
  notes,
  selectedNoteId,
  onSelectNote,
  onPinNote,
}: QuickAccessListProps) {
  const pinnedNotes = useMemo(() => notes.filter((n) => n.pinned), [notes]);

  const recentNotes = useMemo(() => {
    const pinnedIds = new Set(pinnedNotes.map((n) => n.id));
    return notes
      .filter((n) => !pinnedIds.has(n.id) && !n.archived)
      .sort((a, b) => new Date(b.updatedAt).getTime() - new Date(a.updatedAt).getTime())
      .slice(0, 8);
  }, [notes, pinnedNotes]);

  if (pinnedNotes.length === 0 && recentNotes.length === 0) return null;

  return (
    <div className="flex flex-col gap-0.5">
      <div className="text-[10px] uppercase tracking-wider text-dim px-2 py-1">Quick Access</div>

      {pinnedNotes.map((note) => (
        <NoteItem
          key={note.id}
          note={note}
          icon="pin"
          isSelected={note.id === selectedNoteId}
          onSelect={onSelectNote}
          onUnpin={() => onPinNote(note.id, false)}
        />
      ))}

      {recentNotes.length > 0 && pinnedNotes.length > 0 && (
        <div className="h-px bg-white/[0.04] mx-2 my-0.5" />
      )}

      {recentNotes.map((note) => (
        <NoteItem
          key={note.id}
          note={note}
          icon="recent"
          isSelected={note.id === selectedNoteId}
          onSelect={onSelectNote}
        />
      ))}
    </div>
  );
}

function NoteItem({
  note,
  icon,
  isSelected,
  onSelect,
  onUnpin,
}: {
  note: Note;
  icon: "pin" | "recent";
  isSelected: boolean;
  onSelect: (id: string) => void;
  onUnpin?: () => void;
}) {
  return (
    <button
      type="button"
      onClick={() => onSelect(note.id)}
      onDoubleClick={onUnpin}
      className={`flex items-center gap-1.5 px-2 py-1 rounded text-sm truncate text-left w-full transition-colors ${
        isSelected ? "bg-white/[0.08] text-primary" : "text-secondary hover:bg-white/[0.04]"
      }`}
      title={onUnpin ? `${note.title} (double-click to unpin)` : note.title}
    >
      {icon === "pin" ? (
        <Pin className="w-3 h-3 shrink-0 text-dim" />
      ) : (
        <Clock className="w-3 h-3 shrink-0 text-dim" />
      )}
      <span className="truncate">{note.title || "Untitled"}</span>
    </button>
  );
}
