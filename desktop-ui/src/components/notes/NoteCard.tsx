import { Pin } from "lucide-react";
import { formatDate } from "../../lib/dates";
import type { Note } from "../../lib/types";

interface NoteCardProps {
  note: Note;
  selected: boolean;
  onSelect: (id: string) => void;
}

export function NoteCard({ note, selected, onSelect }: NoteCardProps) {
  const preview = note.body.slice(0, 100) || "Empty note";
  const dateStr = formatDate(note.updatedAt.slice(0, 10));

  return (
    <button
      type="button"
      onClick={() => onSelect(note.id)}
      className={`w-full p-2.5 rounded-xl text-left transition-colors ${
        selected ? "bg-white/[0.08] ring-1 ring-brand/30" : "hover:bg-white/[0.04]"
      }`}
    >
      <div className="flex items-center gap-1">
        {note.pinned && <Pin className="w-3 h-3 text-brand shrink-0" />}
        <span className="text-sm font-medium text-primary truncate">{note.title}</span>
        <span className="ml-auto text-[10px] text-dim shrink-0">{dateStr}</span>
      </div>
      <div className="text-xs text-muted truncate mt-0.5">{preview}</div>
      {note.tags.length > 0 && (
        <div className="flex gap-1 mt-1 flex-wrap">
          {note.tags.slice(0, 3).map((tag) => (
            <span
              key={tag}
              className="text-[10px] px-1.5 py-0.5 rounded-md bg-white/[0.06] text-dim"
            >
              {tag}
            </span>
          ))}
        </div>
      )}
    </button>
  );
}
