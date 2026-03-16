import { useClickOutside } from "@shared/hooks/useClickOutside";
import type { Note } from "@shared/types";
import { ContextMenu, ContextMenuItem, ContextMenuSeparator } from "@shared/ui";
import { Archive, Clock, Pencil, Pin, PinOff, Trash2 } from "lucide-react";
import { useCallback, useMemo, useRef, useState } from "react";

interface QuickAccessListProps {
  notes: Note[];
  selectedNoteId: string | null;
  onSelectNote: (id: string) => void;
  onPinNote: (id: string, pinned: boolean) => void;
  onRenameNote?: (id: string) => void;
  onDeleteNote?: (id: string) => void;
  onArchiveNote?: (id: string) => void;
}

type ContextTarget = { note: Note; x: number; y: number } | null;

export function QuickAccessList({
  notes,
  selectedNoteId,
  onSelectNote,
  onPinNote,
  onRenameNote,
  onDeleteNote,
  onArchiveNote,
}: QuickAccessListProps) {
  const [contextMenu, setContextMenu] = useState<ContextTarget>(null);
  const menuRef = useRef<HTMLDivElement>(null);
  useClickOutside(menuRef, () => setContextMenu(null), contextMenu !== null);

  const pinnedNotes = useMemo(() => notes.filter((n) => n.pinned), [notes]);

  const recentNotes = useMemo(() => {
    const pinnedIds = new Set(pinnedNotes.map((n) => n.id));
    return notes
      .filter((n) => !pinnedIds.has(n.id) && !n.archived)
      .sort((a, b) => new Date(b.updatedAt).getTime() - new Date(a.updatedAt).getTime())
      .slice(0, 8);
  }, [notes, pinnedNotes]);

  const handleContextMenu = useCallback((e: React.MouseEvent, note: Note) => {
    e.preventDefault();
    e.stopPropagation();
    setContextMenu({ note, x: e.clientX, y: e.clientY });
  }, []);

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
          onContextMenu={handleContextMenu}
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
          onContextMenu={handleContextMenu}
        />
      ))}

      {contextMenu && (
        <ContextMenu
          ref={menuRef}
          x={contextMenu.x}
          y={contextMenu.y}
          onClose={() => setContextMenu(null)}
        >
          <ContextMenuItem
            icon={
              contextMenu.note.pinned ? <PinOff className="w-4 h-4" /> : <Pin className="w-4 h-4" />
            }
            onClick={() => {
              onPinNote(contextMenu.note.id, !contextMenu.note.pinned);
              setContextMenu(null);
            }}
          >
            {contextMenu.note.pinned ? "Unpin" : "Pin"}
          </ContextMenuItem>
          {onRenameNote && (
            <ContextMenuItem
              icon={<Pencil className="w-4 h-4" />}
              onClick={() => {
                onRenameNote(contextMenu.note.id);
                setContextMenu(null);
              }}
            >
              Rename
            </ContextMenuItem>
          )}
          {onArchiveNote && (
            <ContextMenuItem
              icon={<Archive className="w-4 h-4" />}
              onClick={() => {
                onArchiveNote(contextMenu.note.id);
                setContextMenu(null);
              }}
            >
              Archive
            </ContextMenuItem>
          )}
          {onDeleteNote && (
            <>
              <ContextMenuSeparator />
              <ContextMenuItem
                icon={<Trash2 className="w-4 h-4" />}
                onClick={() => {
                  onDeleteNote(contextMenu.note.id);
                  setContextMenu(null);
                }}
                destructive
              >
                Delete
              </ContextMenuItem>
            </>
          )}
        </ContextMenu>
      )}
    </div>
  );
}

function NoteItem({
  note,
  icon,
  isSelected,
  onSelect,
  onUnpin,
  onContextMenu,
}: {
  note: Note;
  icon: "pin" | "recent";
  isSelected: boolean;
  onSelect: (id: string) => void;
  onUnpin?: () => void;
  onContextMenu: (e: React.MouseEvent, note: Note) => void;
}) {
  return (
    <button
      type="button"
      onClick={() => onSelect(note.id)}
      onDoubleClick={onUnpin}
      onContextMenu={(e) => onContextMenu(e, note)}
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
