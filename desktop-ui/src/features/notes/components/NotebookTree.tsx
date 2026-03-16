import { useClickOutside } from "@shared/hooks/useClickOutside";
import { formatDate } from "@shared/lib/dates";
import type { Note, Notebook } from "@shared/types";
import { ContextMenu, ContextMenuItem, ContextMenuSeparator, ContextMenuSubmenu } from "@shared/ui";
import {
  ChevronRight,
  FileText,
  FolderClosed,
  FolderInput,
  FolderOpen,
  FolderPlus,
  Palette,
  Pencil,
  Pin,
  PinOff,
  Plus,
  Smile,
  Trash2,
} from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";

// ── Types ─────────────────────────────────────────────────────────────

interface TreeItem {
  type: "notebook" | "note";
  id: string;
  title: string;
  depth: number;
  icon?: string;
  color?: string;
  hasChildren?: boolean;
  isExpanded?: boolean;
  pinned?: boolean;
  updatedAt?: string;
}

interface NotebookTreeProps {
  notebooks: Notebook[];
  notes: Note[];
  selectedNoteId: string | null;
  onSelectNote: (id: string) => void;
  onCreateNote: (notebookId?: string) => void;
  onCreateNotebook: (parentId?: string) => void;
  onDeleteNote: (id: string) => void;
  onPinNote: (id: string, pinned: boolean) => void;
  onDeleteNotebook: (id: string) => void;
  onRenameNotebook: (id: string, title: string) => void;
  onRenameNote: (id: string, title: string) => void;
  onMoveNote: (id: string, notebookId: string | null) => void;
  onMoveNotebook: (id: string, parentId: string | null) => void;
  onUpdateNotebook: (id: string, updates: { icon?: string | null; color?: string | null }) => void;
  onUpdateNote: (id: string, updates: { icon?: string | null }) => void;
}

const ITEM_ICONS = [
  // Documents
  "\u{1F4C4}", "\u{1F4DD}", "\u{1F4CB}", "\u{1F4D1}", "\u{1F4DC}",
  // Folders/Organization
  "\u{1F4C1}", "\u{1F4C2}", "\u{1F5C2}\uFE0F", "\u{1F4DA}", "\u{1F5C3}\uFE0F",
  // Work
  "\u{1F4BC}", "\u{1F3E2}", "\u{1F4CA}", "\u{1F4C8}", "\u{1F4BB}",
  // Creative
  "\u{1F3A8}", "\u270F\uFE0F", "\u{1F58A}\uFE0F", "\u{1F4D0}", "\u{1F3AD}",
  // Science/Tech
  "\u{1F52C}", "\u2697\uFE0F", "\u{1F9EA}", "\u{1F527}", "\u2699\uFE0F",
  // Ideas/Goals
  "\u{1F4A1}", "\u{1F3AF}", "\u{1F680}", "\u2B50", "\u{1F3C6}",
  // Nature/Life
  "\u{1F30D}", "\u{1F331}", "\u{1F30A}", "\u2600\uFE0F", "\u{1F319}",
  // Symbols
  "\u2764\uFE0F", "\u{1F525}", "\u26A1", "\u{1F511}", "\u{1F512}",
  // People
  "\u{1F464}", "\u{1F465}", "\u{1F9E0}", "\u{1F4AC}", "\u{1F4DE}",
];

const ITEM_COLORS = [
  null, // reset
  "#a78bfa", // violet
  "#93c5fd", // blue
  "#6ee7b7", // green
  "#fcd34d", // amber
  "#fca5a5", // red
  "#f9a8d4", // pink
  "#67e8f9", // cyan
  "#fdba74", // orange
  "#86efac", // emerald
  "#c4b5fd", // purple
  "#fde68a", // yellow
];

type ContextTarget =
  | { kind: "folder"; notebook: Notebook; x: number; y: number }
  | { kind: "note"; note: Note; x: number; y: number }
  | { kind: "blank"; x: number; y: number }
  | null;

const INDENT = 16;

// ── Main Component ───────────────────────────────────────────────────

export function NotebookTree({
  notebooks,
  notes,
  selectedNoteId,
  onSelectNote,
  onCreateNote,
  onCreateNotebook,
  onDeleteNote,
  onPinNote,
  onDeleteNotebook,
  onRenameNotebook,
  onRenameNote,
  onMoveNote,
  onMoveNotebook: _onMoveNotebook,
  onUpdateNotebook,
  onUpdateNote,
}: NotebookTreeProps) {
  const [expandedIds, setExpandedIds] = useState<Set<string>>(
    () => new Set(notebooks.map((n) => n.id)),
  );
  const [renamingId, setRenamingId] = useState<string | null>(null);
  const [contextMenu, setContextMenu] = useState<ContextTarget>(null);
  const clickTimerRef = useRef<ReturnType<typeof setTimeout>>();
  const menuRef = useRef<HTMLDivElement>(null);

  useClickOutside(menuRef, () => setContextMenu(null), contextMenu !== null);

  // Expand newly added notebooks automatically
  useEffect(() => {
    setExpandedIds((prev) => {
      const next = new Set(prev);
      let changed = false;
      for (const nb of notebooks) {
        if (!next.has(nb.id)) {
          next.add(nb.id);
          changed = true;
        }
      }
      return changed ? next : prev;
    });
  }, [notebooks]);

  // Lookup maps
  const notebookMap = useMemo(() => {
    const m = new Map<string, Notebook>();
    for (const nb of notebooks) m.set(nb.id, nb);
    return m;
  }, [notebooks]);

  const noteMap = useMemo(() => {
    const m = new Map<string, Note>();
    for (const n of notes) m.set(n.id, n);
    return m;
  }, [notes]);

  const allFolders = useMemo(
    () => notebooks.map((nb) => ({ id: nb.id, title: nb.title })),
    [notebooks],
  );

  // Build flat item list
  const items = useMemo(() => {
    const result: TreeItem[] = [];

    const rootNotebooks = notebooks
      .filter((n) => !n.parentId)
      .sort((a, b) => a.title.localeCompare(b.title));

    const addNotebook = (nb: Notebook, depth: number) => {
      const childNotes = notes.filter((n) => n.notebookId === nb.id && !n.archived);
      const childNotebooks = notebooks.filter((n) => n.parentId === nb.id);
      result.push({
        type: "notebook",
        id: nb.id,
        title: nb.title,
        depth,
        icon: nb.icon ?? undefined,
        color: nb.color ?? undefined,
        hasChildren: childNotes.length > 0 || childNotebooks.length > 0,
        isExpanded: expandedIds.has(nb.id),
      });
      if (expandedIds.has(nb.id)) {
        childNotebooks
          .sort((a, b) => a.title.localeCompare(b.title))
          .forEach((child) => {
            addNotebook(child, depth + 1);
          });
        childNotes
          .sort((a, b) => a.title.localeCompare(b.title))
          .forEach((n) => {
            result.push({
              type: "note",
              id: n.id,
              title: n.title,
              depth: depth + 1,
              icon: n.icon ?? undefined,
              pinned: n.pinned,
              updatedAt: n.updatedAt,
            });
          });
      }
    };

    for (const nb of rootNotebooks) addNotebook(nb, 0);

    // Unfiled notes at root level
    const unfiled = notes
      .filter((n) => !n.notebookId && !n.archived)
      .sort((a, b) => a.title.localeCompare(b.title));
    for (const n of unfiled) {
      result.push({
        type: "note",
        id: n.id,
        title: n.title,
        depth: 0,
        icon: n.icon ?? undefined,
        pinned: n.pinned,
        updatedAt: n.updatedAt,
      });
    }

    return result;
  }, [notebooks, notes, expandedIds]);

  // Toggle expand/collapse
  const toggleExpand = useCallback((id: string) => {
    setExpandedIds((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }, []);

  // Click handling with timer to distinguish single vs double click
  const handleClick = useCallback(
    (item: TreeItem) => {
      if (renamingId === item.id) return;
      if (clickTimerRef.current) clearTimeout(clickTimerRef.current);
      clickTimerRef.current = setTimeout(() => {
        if (item.type === "note") onSelectNote(item.id);
        else toggleExpand(item.id);
      }, 250);
    },
    [renamingId, onSelectNote, toggleExpand],
  );

  const handleDoubleClick = useCallback((item: TreeItem) => {
    if (clickTimerRef.current) clearTimeout(clickTimerRef.current);
    setRenamingId(item.id);
  }, []);

  // Rename commit
  const commitRename = useCallback(
    (item: TreeItem, value: string) => {
      const trimmed = value.trim();
      if (trimmed && trimmed !== item.title) {
        if (item.type === "notebook") onRenameNotebook(item.id, trimmed);
        else onRenameNote(item.id, trimmed);
      }
      setRenamingId(null);
    },
    [onRenameNotebook, onRenameNote],
  );

  // Context menu handlers
  const handleContextMenu = useCallback(
    (e: React.MouseEvent, item: TreeItem) => {
      e.preventDefault();
      e.stopPropagation();
      if (item.type === "notebook") {
        const nb = notebookMap.get(item.id);
        if (nb) setContextMenu({ kind: "folder", notebook: nb, x: e.clientX, y: e.clientY });
      } else {
        const note = noteMap.get(item.id);
        if (note) setContextMenu({ kind: "note", note, x: e.clientX, y: e.clientY });
      }
    },
    [notebookMap, noteMap],
  );

  const handleContainerContextMenu = useCallback((e: React.MouseEvent) => {
    if (e.target === e.currentTarget) {
      e.preventDefault();
      setContextMenu({ kind: "blank", x: e.clientX, y: e.clientY });
    }
  }, []);

  // F2 to rename
  const handleKeyDown = useCallback((e: React.KeyboardEvent, item: TreeItem) => {
    if (e.key === "F2") {
      e.preventDefault();
      setRenamingId(item.id);
    }
  }, []);

  return (
    <div className="flex flex-col min-h-0">
      {/* Header */}
      <div className="flex items-center justify-between px-2 pb-1">
        <span className="text-[10px] uppercase tracking-wider text-dim">Notebooks</span>
        <div className="flex items-center gap-0.5">
          <button
            type="button"
            onClick={() => onCreateNote()}
            className="w-5 h-5 rounded-md flex items-center justify-center text-dim hover:text-primary hover:bg-white/[0.06] transition-colors"
            aria-label="New note"
          >
            <Plus className="w-3.5 h-3.5" />
          </button>
          <button
            type="button"
            onClick={() => onCreateNotebook()}
            className="w-5 h-5 rounded-md flex items-center justify-center text-dim hover:text-primary hover:bg-white/[0.06] transition-colors"
            aria-label="New notebook"
          >
            <FolderPlus className="w-3.5 h-3.5" />
          </button>
        </div>
      </div>

      {/* Tree */}
      <div
        role="tree"
        aria-label="Notebook tree"
        onContextMenu={handleContainerContextMenu}
        className="flex-1 overflow-y-auto flex flex-col min-h-0"
      >
        {items.map((item) => {
          const isFolder = item.type === "notebook";
          const isNote = item.type === "note";
          const isSelected = isNote && item.id === selectedNoteId;
          const isRenaming = renamingId === item.id;

          return (
            <div
              key={`${item.type}:${item.id}`}
              role="treeitem"
              tabIndex={0}
              onClick={() => handleClick(item)}
              onDoubleClick={() => handleDoubleClick(item)}
              onContextMenu={(e) => handleContextMenu(e, item)}
              onKeyDown={(e) => handleKeyDown(e, item)}
              className={`flex items-center gap-1 py-1 px-1 rounded text-sm cursor-default select-none outline-none transition-colors ${
                isSelected ? "bg-white/[0.08] text-primary" : "text-secondary hover:bg-white/[0.04]"
              }`}
              style={{ paddingLeft: `${item.depth * INDENT + 4}px` }}
            >
              {/* Chevron for folders */}
              {isFolder && (
                <ChevronRight
                  className={`w-3 h-3 shrink-0 text-dim transition-transform ${
                    item.isExpanded ? "rotate-90" : ""
                  }`}
                />
              )}

              {/* Icon */}
              {item.icon ? (
                <span className="text-sm shrink-0 w-4 text-center">{item.icon}</span>
              ) : isFolder ? (
                item.isExpanded ? (
                  <FolderOpen
                    className={`w-3.5 h-3.5 shrink-0 ${item.color ? "" : "text-brand/60"}`}
                    style={item.color ? { color: item.color } : undefined}
                  />
                ) : (
                  <FolderClosed
                    className={`w-3.5 h-3.5 shrink-0 ${item.color ? "" : "text-brand/60"}`}
                    style={item.color ? { color: item.color } : undefined}
                  />
                )
              ) : isNote && item.pinned ? (
                <Pin className="w-3 h-3 shrink-0 text-brand" />
              ) : (
                <FileText
                  className={`w-3.5 h-3.5 shrink-0 ${isSelected ? "text-brand/70" : "text-dim"}`}
                />
              )}

              {/* Title / rename input */}
              {isRenaming ? (
                <input
                  ref={(el) => {
                    if (el) {
                      el.focus();
                      el.select();
                    }
                  }}
                  defaultValue={item.title}
                  className="flex-1 min-w-0 text-sm bg-white/[0.06] border border-brand/40 rounded-md px-1.5 py-0.5 text-primary focus:outline-none"
                  onBlur={(e) => commitRename(item, e.currentTarget.value)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter") e.currentTarget.blur();
                    if (e.key === "Escape") setRenamingId(null);
                    e.stopPropagation();
                  }}
                  onClick={(e) => e.stopPropagation()}
                  onDoubleClick={(e) => e.stopPropagation()}
                />
              ) : (
                <span
                  className={`truncate flex-1 ${
                    item.title === "Untitled" || item.title === "New Folder"
                      ? "text-dim italic"
                      : ""
                  }`}
                >
                  {item.title}
                </span>
              )}

              {/* Date badge for notes */}
              {isNote && item.updatedAt && !isRenaming && (
                <span className="text-[10px] text-dim shrink-0 mr-1">
                  {formatDate(item.updatedAt.slice(0, 10))}
                </span>
              )}
            </div>
          );
        })}

        {items.length === 0 && (
          <div className="text-xs text-dim text-center py-6">No notes yet</div>
        )}
      </div>

      {/* Context Menu */}
      {contextMenu && (
        <TreeContextMenu
          ref={menuRef}
          target={contextMenu}
          folders={allFolders}
          onStartRename={setRenamingId}
          onCreateNote={onCreateNote}
          onCreateNotebook={onCreateNotebook}
          onDeleteNote={onDeleteNote}
          onPinNote={onPinNote}
          onDeleteNotebook={onDeleteNotebook}
          onMoveNote={onMoveNote}
          onUpdateNotebook={onUpdateNotebook}
          onUpdateNote={onUpdateNote}
          onClose={() => setContextMenu(null)}
        />
      )}
    </div>
  );
}

// ── Context Menu ─────────────────────────────────────────────────────

interface TreeContextMenuProps {
  target: NonNullable<ContextTarget>;
  folders: { id: string; title: string }[];
  onStartRename: (id: string) => void;
  onCreateNote: (notebookId?: string) => void;
  onCreateNotebook: (parentId?: string) => void;
  onDeleteNote: (id: string) => void;
  onPinNote: (id: string, pinned: boolean) => void;
  onDeleteNotebook: (id: string) => void;
  onMoveNote: (id: string, notebookId: string | null) => void;
  onUpdateNotebook: (id: string, updates: { icon?: string | null; color?: string | null }) => void;
  onUpdateNote: (id: string, updates: { icon?: string | null }) => void;
  onClose: () => void;
}

function TreeContextMenu({
  target,
  folders,
  onStartRename,
  onCreateNote,
  onCreateNotebook,
  onDeleteNote,
  onPinNote,
  onDeleteNotebook,
  onMoveNote,
  onUpdateNotebook,
  onUpdateNote,
  onClose,
  ref,
}: TreeContextMenuProps & { ref: React.Ref<HTMLDivElement> }) {
  const [showMoveSubmenu, setShowMoveSubmenu] = useState(false);
  const [showIconPicker, setShowIconPicker] = useState(false);
  const [showColorPicker, setShowColorPicker] = useState(false);

  if (target.kind === "blank") {
    return (
      <ContextMenu ref={ref} x={target.x} y={target.y} onClose={onClose}>
        <ContextMenuItem
          icon={<Plus className="w-4 h-4" />}
          onClick={() => {
            onCreateNote();
            onClose();
          }}
        >
          New note
        </ContextMenuItem>
        <ContextMenuItem
          icon={<FolderPlus className="w-4 h-4" />}
          onClick={() => {
            onCreateNotebook();
            onClose();
          }}
        >
          New notebook
        </ContextMenuItem>
      </ContextMenu>
    );
  }

  if (target.kind === "folder") {
    return (
      <ContextMenu ref={ref} x={target.x} y={target.y} onClose={onClose}>
        <ContextMenuItem
          icon={<Plus className="w-4 h-4" />}
          onClick={() => {
            onCreateNote(target.notebook.id);
            onClose();
          }}
        >
          New note here
        </ContextMenuItem>
        <ContextMenuItem
          icon={<FolderPlus className="w-4 h-4" />}
          onClick={() => {
            onCreateNotebook(target.notebook.id);
            onClose();
          }}
        >
          New sub-notebook
        </ContextMenuItem>
        <ContextMenuSeparator />
        <ContextMenuItem
          icon={<Pencil className="w-4 h-4" />}
          onClick={() => {
            onStartRename(target.notebook.id);
            onClose();
          }}
        >
          Rename
        </ContextMenuItem>

        {/* Icon picker */}
        <ContextMenuSubmenu
          icon={<Smile className="w-4 h-4" />}
          label="Change Icon"
          open={showIconPicker}
          onToggle={() => setShowIconPicker(!showIconPicker)}
        >
          <div className="grid grid-cols-8 gap-0.5 p-1.5 max-h-40 overflow-y-auto">
            {ITEM_ICONS.map((icon) => (
              <button
                key={icon}
                type="button"
                onClick={() => {
                  onUpdateNotebook(target.notebook.id, { icon, color: null });
                  onClose();
                }}
                className={`w-7 h-7 rounded flex items-center justify-center text-sm hover:bg-white/[0.1] transition-colors ${
                  target.notebook.icon === icon ? "bg-white/[0.12] ring-1 ring-brand/40" : ""
                }`}
              >
                {icon}
              </button>
            ))}
          </div>
          {target.notebook.icon && (
            <ContextMenuItem
              onClick={() => {
                onUpdateNotebook(target.notebook.id, { icon: null });
                onClose();
              }}
            >
              <span className="text-dim text-xs">Remove icon</span>
            </ContextMenuItem>
          )}
        </ContextMenuSubmenu>

        {/* Color picker */}
        <ContextMenuSubmenu
          icon={<Palette className="w-4 h-4" />}
          label="Change Color"
          open={showColorPicker}
          onToggle={() => setShowColorPicker(!showColorPicker)}
        >
          <div className="flex gap-1 p-2">
            {ITEM_COLORS.map((color) => (
              <button
                key={color ?? "none"}
                type="button"
                onClick={() => {
                  onUpdateNotebook(target.notebook.id, color ? { color, icon: null } : { color: null });
                  onClose();
                }}
                className={`w-5 h-5 rounded-full border transition-transform hover:scale-125 ${
                  target.notebook.color === color ? "ring-2 ring-white/60 ring-offset-1 ring-offset-black" : ""
                } ${!color ? "border-white/20 bg-transparent" : "border-transparent"}`}
                style={color ? { backgroundColor: color } : undefined}
                title={color ?? "No color"}
              >
                {!color && <span className="text-[8px] text-dim">&times;</span>}
              </button>
            ))}
          </div>
        </ContextMenuSubmenu>

        <ContextMenuSeparator />
        <ContextMenuItem
          icon={<Trash2 className="w-4 h-4" />}
          onClick={() => {
            onDeleteNotebook(target.notebook.id);
            onClose();
          }}
          destructive
        >
          Delete
        </ContextMenuItem>
      </ContextMenu>
    );
  }

  // Note context menu
  const { note } = target;
  return (
    <ContextMenu ref={ref} x={target.x} y={target.y} onClose={onClose}>
      <ContextMenuItem
        icon={<Pencil className="w-4 h-4" />}
        onClick={() => {
          onStartRename(note.id);
          onClose();
        }}
      >
        Rename
      </ContextMenuItem>
      <ContextMenuItem
        icon={note.pinned ? <PinOff className="w-4 h-4" /> : <Pin className="w-4 h-4" />}
        onClick={() => {
          onPinNote(note.id, !note.pinned);
          onClose();
        }}
      >
        {note.pinned ? "Unpin" : "Pin"}
      </ContextMenuItem>

      {/* Icon picker for notes */}
      <ContextMenuSubmenu
        icon={<Smile className="w-4 h-4" />}
        label="Change Icon"
        open={showIconPicker}
        onToggle={() => setShowIconPicker(!showIconPicker)}
      >
        <div className="grid grid-cols-8 gap-0.5 p-1.5 max-h-40 overflow-y-auto">
          {ITEM_ICONS.map((icon) => (
            <button
              key={icon}
              type="button"
              onClick={() => {
                onUpdateNote(note.id, { icon });
                onClose();
              }}
              className={`w-7 h-7 rounded flex items-center justify-center text-sm hover:bg-white/[0.1] transition-colors ${
                note.icon === icon ? "bg-white/[0.12] ring-1 ring-brand/40" : ""
              }`}
            >
              {icon}
            </button>
          ))}
        </div>
        {note.icon && (
          <ContextMenuItem
            onClick={() => {
              onUpdateNote(note.id, { icon: null });
              onClose();
            }}
          >
            <span className="text-dim text-xs">Remove icon</span>
          </ContextMenuItem>
        )}
      </ContextMenuSubmenu>

      {folders.length > 0 && (
        <ContextMenuSubmenu
          icon={<FolderInput className="w-4 h-4" />}
          label="Move to..."
          open={showMoveSubmenu}
          onToggle={() => setShowMoveSubmenu(!showMoveSubmenu)}
        >
          <ContextMenuItem
            onClick={() => {
              onMoveNote(note.id, null);
              onClose();
            }}
          >
            <span className="text-dim italic">Unfiled</span>
          </ContextMenuItem>
          {folders
            .filter((f) => f.id !== note.notebookId)
            .map((folder) => (
              <ContextMenuItem
                key={folder.id}
                icon={<FolderClosed className="w-3.5 h-3.5 text-dim" />}
                onClick={() => {
                  onMoveNote(note.id, folder.id);
                  onClose();
                }}
              >
                {folder.title}
              </ContextMenuItem>
            ))}
        </ContextMenuSubmenu>
      )}

      <ContextMenuSeparator />
      <ContextMenuItem
        icon={<Trash2 className="w-4 h-4" />}
        onClick={() => {
          onDeleteNote(note.id);
          onClose();
        }}
        destructive
      >
        Delete
      </ContextMenuItem>
    </ContextMenu>
  );
}
