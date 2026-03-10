import { useClickOutside } from "@shared/hooks/useClickOutside";
import { formatDate } from "@shared/lib/dates";
import type { Note } from "@shared/types";
import { Pin, PinOff, Trash2 } from "lucide-react";
import { memo, useRef, useState } from "react";
import { createPortal } from "react-dom";

interface NoteCardProps {
  note: Note;
  selected: boolean;
  onSelect: (id: string) => void;
  onPin: (id: string, pinned: boolean) => void;
  onDelete: (id: string) => void;
}

export const NoteCard = memo(function NoteCard({
  note,
  selected,
  onSelect,
  onPin,
  onDelete,
}: NoteCardProps) {
  const preview = note.body.slice(0, 100) || "Empty note";
  const dateStr = formatDate(note.updatedAt.slice(0, 10));
  const [menu, setMenu] = useState<{ x: number; y: number } | null>(null);
  const menuRef = useRef<HTMLDivElement>(null);
  useClickOutside(menuRef, () => setMenu(null), menu !== null);

  return (
    <div className="relative">
      <button
        type="button"
        onClick={() => onSelect(note.id)}
        onContextMenu={(e) => {
          e.preventDefault();
          setMenu({ x: e.clientX, y: e.clientY });
        }}
        className={`w-full p-2.5 note-card text-left ${
          selected ? "note-card-active ring-1 ring-brand/30" : ""
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
              <span key={tag} className="tag-pill">
                {tag}
              </span>
            ))}
          </div>
        )}
      </button>

      {menu && (
        <NoteContextMenu
          ref={menuRef}
          x={menu.x}
          y={menu.y}
          pinned={note.pinned}
          onPin={() => {
            onPin(note.id, !note.pinned);
            setMenu(null);
          }}
          onDelete={() => {
            onDelete(note.id);
            setMenu(null);
          }}
        />
      )}
    </div>
  );
});

interface NoteContextMenuProps {
  x: number;
  y: number;
  pinned: boolean;
  onPin: () => void;
  onDelete: () => void;
}

function NoteContextMenu({
  x,
  y,
  pinned,
  onPin,
  onDelete,
  ref,
}: NoteContextMenuProps & { ref: React.Ref<HTMLDivElement> }) {
  return createPortal(
    <div
      ref={ref}
      className="fixed z-50 glass-dropdown rounded-xl py-1 min-w-[140px]"
      style={{ left: x, top: y }}
    >
      <button
        type="button"
        onClick={onPin}
        className="w-full px-3 py-1.5 text-sm text-secondary hover:bg-white/[0.06] flex items-center gap-2 text-left"
      >
        {pinned ? <PinOff className="w-3.5 h-3.5" /> : <Pin className="w-3.5 h-3.5" />}
        {pinned ? "Unpin" : "Pin"}
      </button>
      <button
        type="button"
        onClick={onDelete}
        className="w-full px-3 py-1.5 text-sm text-red-400 hover:bg-white/[0.06] flex items-center gap-2 text-left"
      >
        <Trash2 className="w-3.5 h-3.5" />
        Delete
      </button>
    </div>,
    document.body,
  );
}
