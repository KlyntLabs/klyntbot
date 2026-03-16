import {
  type DragTarget,
  dragAndDropFeature,
  hotkeysCoreFeature,
  renamingFeature,
  selectionFeature,
  syncDataLoaderFeature,
} from "@headless-tree/core";
import { useTree } from "@headless-tree/react";
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
  Pencil,
  Pin,
  PinOff,
  Plus,
  Trash2,
} from "lucide-react";
import { useCallback, useMemo, useRef, useState } from "react";

// ── Types ─────────────────────────────────────────────────────────────

interface TreeNodeData {
  type: "notebook" | "note" | "root";
  id: string;
  title: string;
  icon?: string;
  color?: string;
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
}

type ContextTarget =
  | { kind: "folder"; notebook: Notebook; x: number; y: number }
  | { kind: "note"; note: Note; x: number; y: number }
  | { kind: "blank"; x: number; y: number }
  | null;

const INDENT = 16;
const ROOT_ID = "root";
const UNFILED_ID = "__unfiled__";

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
  onMoveNotebook,
}: NotebookTreeProps) {
  const [contextMenu, setContextMenu] = useState<ContextTarget>(null);
  const menuRef = useRef<HTMLDivElement>(null);
  useClickOutside(menuRef, () => setContextMenu(null), contextMenu !== null);

  // Build lookup maps
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

  const unfiledNotes = useMemo(() => notes.filter((n) => !n.notebookId && !n.archived), [notes]);
  const hasUnfiled = unfiledNotes.length > 0;

  // Data map for tree items
  const dataMap = useMemo(() => {
    const m: Record<string, TreeNodeData> = {
      [ROOT_ID]: { type: "root", id: ROOT_ID, title: "Root" },
    };
    if (hasUnfiled) {
      m[UNFILED_ID] = { type: "notebook", id: UNFILED_ID, title: "Unfiled" };
    }
    for (const nb of notebooks) {
      m[nb.id] = {
        type: "notebook",
        id: nb.id,
        title: nb.title,
        icon: nb.icon ?? undefined,
        color: nb.color ?? undefined,
      };
    }
    for (const n of notes) {
      if (!n.archived) {
        m[`note:${n.id}`] = {
          type: "note",
          id: n.id,
          title: n.title,
          pinned: n.pinned,
          updatedAt: n.updatedAt,
        };
      }
    }
    return m;
  }, [notebooks, notes, hasUnfiled]);

  // Children map for tree structure
  const childrenMap = useMemo(() => {
    const m: Record<string, string[]> = {};

    // Root children: top-level notebooks + unfiled section
    const rootChildren: string[] = [];
    for (const nb of notebooks) {
      if (!nb.parentId) rootChildren.push(nb.id);
    }
    rootChildren.sort((a, b) => {
      const na = notebookMap.get(a);
      const nb2 = notebookMap.get(b);
      return (na?.title ?? "").localeCompare(nb2?.title ?? "");
    });
    if (hasUnfiled) rootChildren.push(UNFILED_ID);
    m[ROOT_ID] = rootChildren;

    // Notebook children: sub-notebooks + notes in this notebook
    for (const nb of notebooks) {
      const children: string[] = [];
      // Sub-notebooks
      for (const child of notebooks) {
        if (child.parentId === nb.id) children.push(child.id);
      }
      children.sort((a, b) => {
        const na = notebookMap.get(a);
        const nb2 = notebookMap.get(b);
        return (na?.title ?? "").localeCompare(nb2?.title ?? "");
      });
      // Notes in this notebook
      const notesInNb = notes
        .filter((n) => n.notebookId === nb.id && !n.archived)
        .sort((a, b) => a.title.localeCompare(b.title));
      for (const n of notesInNb) children.push(`note:${n.id}`);
      m[nb.id] = children;
    }

    // Unfiled children
    if (hasUnfiled) {
      m[UNFILED_ID] = unfiledNotes
        .sort((a, b) => a.title.localeCompare(b.title))
        .map((n) => `note:${n.id}`);
    }

    return m;
  }, [notebooks, notes, notebookMap, hasUnfiled, unfiledNotes]);

  const allFolders = useMemo(
    () => notebooks.map((nb) => ({ id: nb.id, title: nb.title })),
    [notebooks],
  );

  const handleDrop = useCallback(
    (droppedItems: { getItemData: () => TreeNodeData }[], target: DragTarget<TreeNodeData>) => {
      const targetData = target.item.getItemData();

      for (const item of droppedItems) {
        const data = item.getItemData();
        if (data.type === "note") {
          const newNotebookId =
            targetData.type === "notebook"
              ? targetData.id === UNFILED_ID
                ? null
                : targetData.id
              : null;
          onMoveNote(data.id, newNotebookId);
        } else if (data.type === "notebook" && data.id !== UNFILED_ID) {
          const newParentId =
            targetData.type === "notebook" && targetData.id !== UNFILED_ID ? targetData.id : null;
          onMoveNotebook(data.id, newParentId);
        }
      }
    },
    [onMoveNote, onMoveNotebook],
  );

  const handleRename = useCallback(
    (item: { getItemData: () => TreeNodeData }, value: string) => {
      const data = item.getItemData();
      const trimmed = value.trim();
      if (!trimmed) return;
      if (data.type === "notebook" && data.id !== UNFILED_ID) {
        onRenameNotebook(data.id, trimmed);
      } else if (data.type === "note") {
        onRenameNote(data.id, trimmed);
      }
    },
    [onRenameNotebook, onRenameNote],
  );

  const tree = useTree<TreeNodeData>({
    rootItemId: ROOT_ID,
    getItemName: (item) => item.getItemData().title,
    isItemFolder: (item) =>
      item.getItemData().type === "notebook" || item.getItemData().type === "root",
    canReorder: true,
    indent: INDENT,
    dataLoader: {
      getItem: (itemId) => dataMap[itemId] ?? { type: "root", id: itemId, title: "" },
      getChildren: (itemId) => childrenMap[itemId] ?? [],
    },
    onPrimaryAction: (item) => {
      const data = item.getItemData();
      if (data.type === "note") onSelectNote(data.id);
    },
    onDrop: handleDrop,
    canDrag: (items) => {
      return items.every((item) => {
        const data = item.getItemData();
        return data.id !== ROOT_ID && data.id !== UNFILED_ID;
      });
    },
    onRename: handleRename,
    canRename: (item) => {
      const data = item.getItemData();
      return data.id !== ROOT_ID && data.id !== UNFILED_ID;
    },
    features: [
      syncDataLoaderFeature,
      selectionFeature,
      hotkeysCoreFeature,
      dragAndDropFeature,
      renamingFeature,
    ],
  });

  const handleContextMenu = useCallback(
    (e: React.MouseEvent, itemId: string) => {
      e.preventDefault();
      e.stopPropagation();
      const data = dataMap[itemId];
      if (!data) return;

      if (data.type === "notebook") {
        const nb = notebookMap.get(data.id);
        if (nb) {
          setContextMenu({ kind: "folder", notebook: nb, x: e.clientX, y: e.clientY });
        }
      } else if (data.type === "note") {
        const note = noteMap.get(data.id);
        if (note) {
          setContextMenu({ kind: "note", note, x: e.clientX, y: e.clientY });
        }
      }
    },
    [dataMap, notebookMap, noteMap],
  );

  const handleContainerContextMenu = useCallback((e: React.MouseEvent) => {
    if (e.target === e.currentTarget) {
      e.preventDefault();
      setContextMenu({ kind: "blank", x: e.clientX, y: e.clientY });
    }
  }, []);

  const items = tree.getItems();

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
        {...tree.getContainerProps("Notebook tree")}
        onContextMenu={handleContainerContextMenu}
        className="flex-1 overflow-y-auto flex flex-col min-h-0"
      >
        {items.map((item) => {
          const data = item.getItemData();
          const isFolder = item.isFolder();
          const isNote = data.type === "note";
          const isUnfiled = data.id === UNFILED_ID;
          const level = item.getItemMeta().level;
          const isSelected = isNote && data.id === selectedNoteId;
          const isRenaming = item.isRenaming();

          return (
            <div
              key={item.getId()}
              {...item.getProps()}
              ref={item.registerElement}
              onContextMenu={(e) => handleContextMenu(e, item.getId())}
              className={`flex items-center gap-1 py-1 px-1 rounded text-sm cursor-default select-none outline-none transition-colors ${
                item.isDragTarget()
                  ? "bg-white/[0.08] ring-1 ring-white/[0.15]"
                  : isSelected
                    ? "bg-white/[0.08] text-primary"
                    : "text-secondary hover:bg-white/[0.04]"
              } ${item.isFocused() ? "ring-1 ring-brand/30" : ""}`}
              style={{ paddingLeft: `${level * INDENT + 4}px` }}
            >
              {/* Chevron for folders */}
              {isFolder && (
                <ChevronRight
                  className={`w-3 h-3 shrink-0 text-dim transition-transform ${
                    item.isExpanded() ? "rotate-90" : ""
                  }`}
                />
              )}

              {/* Icon */}
              {isFolder && !isUnfiled && data.icon ? (
                <span className="text-sm shrink-0 w-4 text-center">{data.icon}</span>
              ) : isFolder ? (
                item.isExpanded() ? (
                  <FolderOpen
                    className={`w-3.5 h-3.5 shrink-0 ${isUnfiled ? "text-dim/50" : "text-brand/60"}`}
                  />
                ) : (
                  <FolderClosed
                    className={`w-3.5 h-3.5 shrink-0 ${isUnfiled ? "text-dim/50" : "text-brand/60"}`}
                  />
                )
              ) : isNote && data.pinned ? (
                <Pin className="w-3 h-3 shrink-0 text-brand" />
              ) : (
                <FileText
                  className={`w-3.5 h-3.5 shrink-0 ${isSelected ? "text-brand/70" : "text-dim"}`}
                />
              )}

              {/* Color bar for notebooks */}
              {isFolder && data.color && (
                <div
                  className="w-0.5 h-4 rounded-full shrink-0"
                  style={{ backgroundColor: data.color }}
                />
              )}

              {/* Title / rename input */}
              {isRenaming ? (
                <input
                  {...item.getRenameInputProps()}
                  className="flex-1 min-w-0 text-sm bg-white/[0.06] border border-brand/40 rounded-md px-1.5 py-0.5 text-primary focus:outline-none"
                />
              ) : (
                <span
                  className={`truncate flex-1 ${
                    isFolder && isUnfiled
                      ? "text-dim italic"
                      : data.title === "Untitled" || data.title === "New Folder"
                        ? "text-dim italic"
                        : ""
                  }`}
                >
                  {data.title}
                </span>
              )}

              {/* Date badge for notes */}
              {isNote && data.updatedAt && !isRenaming && (
                <span className="text-[10px] text-dim shrink-0 mr-1">
                  {formatDate(data.updatedAt.slice(0, 10))}
                </span>
              )}
            </div>
          );
        })}

        {items.length === 0 && (
          <div className="text-xs text-dim text-center py-6">No notes yet</div>
        )}

        {/* Drag line */}
        {tree.getDragLineData() && (
          <div
            className="h-0.5 bg-brand rounded-full absolute pointer-events-none"
            style={tree.getDragLineStyle()}
          />
        )}
      </div>

      {/* Context Menu */}
      {contextMenu && (
        <TreeContextMenu
          ref={menuRef}
          target={contextMenu}
          folders={allFolders}
          tree={tree}
          onCreateNote={onCreateNote}
          onCreateNotebook={onCreateNotebook}
          onDeleteNote={onDeleteNote}
          onPinNote={onPinNote}
          onDeleteNotebook={onDeleteNotebook}
          onMoveNote={onMoveNote}
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
  tree: ReturnType<typeof useTree<TreeNodeData>>;
  onCreateNote: (notebookId?: string) => void;
  onCreateNotebook: (parentId?: string) => void;
  onDeleteNote: (id: string) => void;
  onPinNote: (id: string, pinned: boolean) => void;
  onDeleteNotebook: (id: string) => void;
  onMoveNote: (id: string, notebookId: string | null) => void;
  onClose: () => void;
}

function TreeContextMenu({
  target,
  folders,
  tree,
  onCreateNote,
  onCreateNotebook,
  onDeleteNote,
  onPinNote,
  onDeleteNotebook,
  onMoveNote,
  onClose,
  ref,
}: TreeContextMenuProps & { ref: React.Ref<HTMLDivElement> }) {
  const [showMoveSubmenu, setShowMoveSubmenu] = useState(false);

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
            const item = tree.getItemInstance(target.notebook.id);
            item?.startRenaming();
            onClose();
          }}
        >
          Rename
        </ContextMenuItem>
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
          const item = tree.getItemInstance(`note:${note.id}`);
          item?.startRenaming();
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
