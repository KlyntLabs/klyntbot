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
import { type DragEvent, memo, useCallback, useEffect, useMemo, useRef, useState } from "react";
import { AgentFileTree } from "./AgentFileTree";
import { WorkspaceFileTree } from "./WorkspaceFileTree";

// ── Types ────────────────────────────────────────────────────────────────

interface FileTreeProps {
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
  activeWorkspaceFile: string | null;
  onSelectWorkspaceFile: (filename: string) => void;
  activeAgentName: string | null;
  activeAgentFilename: string | null;
  onSelectAgentFile: (agentName: string, filename: string) => void;
}

type ContextTarget =
  | { kind: "folder"; notebook: Notebook; x: number; y: number }
  | { kind: "note"; note: Note; x: number; y: number }
  | { kind: "blank"; x: number; y: number }
  | null;

type DragPayload = { kind: "note"; id: string } | { kind: "folder"; id: string };

// Module-level drag state — avoids DataTransfer MIME issues in WKWebView
let activeDrag: DragPayload | null = null;

function encodeDrag(payload: DragPayload, e: DragEvent) {
  activeDrag = payload;
  e.dataTransfer.effectAllowed = "move";
  // Set text/plain so the browser shows a drag ghost image
  e.dataTransfer.setData("text/plain", payload.id);
}
function decodeDrag(): DragPayload | null {
  return activeDrag;
}
function clearDrag() {
  activeDrag = null;
}

// ── Main Component ───────────────────────────────────────────────────────

export function FileTree({
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
  activeWorkspaceFile,
  onSelectWorkspaceFile,
  activeAgentName,
  activeAgentFilename,
  onSelectAgentFile,
}: FileTreeProps) {
  const [contextMenu, setContextMenu] = useState<ContextTarget>(null);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [dropTarget, setDropTarget] = useState<string | null>(null); // folder id or "root"
  const menuRef = useRef<HTMLDivElement>(null);
  useClickOutside(menuRef, () => setContextMenu(null), contextMenu !== null);

  // Build folder children map: parentId -> child notebooks
  const folderChildren = useMemo(() => {
    const map = new Map<string | null, Notebook[]>();
    for (const nb of notebooks) {
      const pid = nb.parentId ?? null;
      const list = map.get(pid) ?? [];
      list.push(nb);
      map.set(pid, list);
    }
    for (const list of map.values()) {
      list.sort((a, b) => a.title.localeCompare(b.title));
    }
    return map;
  }, [notebooks]);

  // Build notes-per-folder map: notebookId -> notes
  const notesInFolder = useMemo(() => {
    const map = new Map<string | null, Note[]>();
    for (const n of notes) {
      const fid = n.notebookId ?? null;
      const list = map.get(fid) ?? [];
      list.push(n);
      map.set(fid, list);
    }
    for (const list of map.values()) {
      list.sort((a, b) => a.title.localeCompare(b.title));
    }
    return map;
  }, [notes]);

  // Collect all folder ids for "Move to" submenu
  const allFolders = useMemo(() => {
    return notebooks.map((nb) => ({ id: nb.id, title: nb.title }));
  }, [notebooks]);

  const rootFolders = folderChildren.get(null) ?? [];
  const rootNotes = notesInFolder.get(null) ?? [];

  // Drop on root area
  const handleRootDragOver = useCallback((e: DragEvent) => {
    e.preventDefault();
    e.dataTransfer.dropEffect = "move";
    setDropTarget("root");
  }, []);

  const handleRootDrop = useCallback(
    (e: DragEvent) => {
      e.preventDefault();
      setDropTarget(null);
      const payload = decodeDrag();
      if (!payload) return;
      if (payload.kind === "note") onMoveNote(payload.id, null);
      if (payload.kind === "folder") onMoveNotebook(payload.id, null);
      clearDrag();
    },
    [onMoveNote, onMoveNotebook],
  );

  const handleDragLeaveRoot = useCallback((e: DragEvent) => {
    // Only clear if leaving the container itself, not entering a child
    if (e.currentTarget === e.target) setDropTarget(null);
  }, []);

  return (
    <div className="flex-1 glass-sidebar p-3 flex flex-col gap-2 min-h-0">
      {/* Header */}
      <div className="flex items-center justify-between">
        <h2 className="text-xs font-medium text-muted uppercase tracking-wider">Notes</h2>
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
            aria-label="New folder"
          >
            <FolderPlus className="w-3.5 h-3.5" />
          </button>
        </div>
      </div>

      {/* Tree */}
      <div
        role="tree"
        className={`flex-1 overflow-y-auto flex flex-col min-h-0 rounded-lg transition-colors ${
          dropTarget === "root" ? "bg-white/[0.03]" : ""
        }`}
        style={{ contentVisibility: "auto" }}
        onDragOver={handleRootDragOver}
        onDrop={handleRootDrop}
        onDragLeave={handleDragLeaveRoot}
        onContextMenu={(e) => {
          // Only fire on blank area, not on items
          if (e.target === e.currentTarget) {
            e.preventDefault();
            setContextMenu({ kind: "blank", x: e.clientX, y: e.clientY });
          }
        }}
      >
        {rootFolders.map((nb) => (
          <FolderNode
            key={nb.id}
            notebook={nb}
            folderChildren={folderChildren}
            notesInFolder={notesInFolder}
            selectedNoteId={selectedNoteId}
            onSelectNote={onSelectNote}
            onContextMenu={setContextMenu}
            editingId={editingId}
            onStartEditing={setEditingId}
            onRenameNotebook={onRenameNotebook}
            onRenameNote={onRenameNote}
            onMoveNote={onMoveNote}
            onMoveNotebook={onMoveNotebook}
            dropTarget={dropTarget}
            onSetDropTarget={setDropTarget}
            depth={0}
          />
        ))}
        {rootNotes.map((note) => (
          <NoteNode
            key={note.id}
            note={note}
            selected={note.id === selectedNoteId}
            onSelect={onSelectNote}
            onContextMenu={setContextMenu}
            editing={editingId === note.id}
            onStartEditing={setEditingId}
            onRenameNote={onRenameNote}
            depth={0}
          />
        ))}
        {rootFolders.length === 0 && rootNotes.length === 0 && (
          <div className="text-xs text-dim text-center py-6">No notes yet</div>
        )}
      </div>

      <WorkspaceFileTree activeFile={activeWorkspaceFile} onSelectFile={onSelectWorkspaceFile} />
      <AgentFileTree
        activeAgent={activeAgentName}
        activeFilename={activeAgentFilename}
        onSelectFile={onSelectAgentFile}
      />

      {/* Context Menu */}
      {contextMenu && (
        <TreeContextMenu
          ref={menuRef}
          target={contextMenu}
          folders={allFolders}
          onCreateNote={onCreateNote}
          onCreateNotebook={onCreateNotebook}
          onDeleteNote={onDeleteNote}
          onPinNote={onPinNote}
          onDeleteNotebook={onDeleteNotebook}
          onStartEditing={setEditingId}
          onMoveNote={onMoveNote}
          onClose={() => setContextMenu(null)}
        />
      )}
    </div>
  );
}

// ── Folder Node ──────────────────────────────────────────────────────────

interface FolderNodeProps {
  notebook: Notebook;
  folderChildren: Map<string | null, Notebook[]>;
  notesInFolder: Map<string | null, Note[]>;
  selectedNoteId: string | null;
  onSelectNote: (id: string) => void;
  onContextMenu: (target: ContextTarget) => void;
  editingId: string | null;
  onStartEditing: (id: string | null) => void;
  onRenameNotebook: (id: string, title: string) => void;
  onRenameNote: (id: string, title: string) => void;
  onMoveNote: (id: string, notebookId: string | null) => void;
  onMoveNotebook: (id: string, parentId: string | null) => void;
  dropTarget: string | null;
  onSetDropTarget: (id: string | null) => void;
  depth: number;
}

const FolderNode = memo(function FolderNode({
  notebook,
  folderChildren,
  notesInFolder,
  selectedNoteId,
  onSelectNote,
  onContextMenu,
  editingId,
  onStartEditing,
  onRenameNotebook,
  onRenameNote,
  onMoveNote,
  onMoveNotebook,
  dropTarget,
  onSetDropTarget,
  depth,
}: FolderNodeProps) {
  const [expanded, setExpanded] = useState(true);
  const children = folderChildren.get(notebook.id) ?? [];
  const childNotes = notesInFolder.get(notebook.id) ?? [];
  const itemCount = children.length + childNotes.length;
  const isEditing = editingId === `folder:${notebook.id}`;
  const isDragOver = dropTarget === notebook.id;
  const autoExpandTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Drag source
  const handleDragStart = useCallback(
    (e: DragEvent) => {
      encodeDrag({ kind: "folder", id: notebook.id }, e);
    },
    [notebook.id],
  );

  // Drop target
  const handleDragOver = useCallback(
    (e: DragEvent) => {
      e.preventDefault();
      e.stopPropagation();
      e.dataTransfer.dropEffect = "move";
      onSetDropTarget(notebook.id);
    },
    [notebook.id, onSetDropTarget],
  );

  const handleDrop = useCallback(
    (e: DragEvent) => {
      e.preventDefault();
      e.stopPropagation();
      onSetDropTarget(null);
      const payload = decodeDrag();
      if (!payload) return;
      // Don't drop a folder into itself
      if (payload.kind === "folder" && payload.id === notebook.id) return;
      if (payload.kind === "note") onMoveNote(payload.id, notebook.id);
      if (payload.kind === "folder") onMoveNotebook(payload.id, notebook.id);
      clearDrag();
    },
    [notebook.id, onMoveNote, onMoveNotebook, onSetDropTarget],
  );

  const handleDragEnter = useCallback(() => {
    // Auto-expand folder after hovering 500ms
    if (!expanded) {
      autoExpandTimer.current = setTimeout(() => setExpanded(true), 500);
    }
  }, [expanded]);

  const handleDragLeave = useCallback(
    (e: DragEvent) => {
      // Only clear when actually leaving this folder's row
      const rect = e.currentTarget.getBoundingClientRect();
      const { clientX, clientY } = e;
      if (
        clientX < rect.left ||
        clientX > rect.right ||
        clientY < rect.top ||
        clientY > rect.bottom
      ) {
        if (autoExpandTimer.current) clearTimeout(autoExpandTimer.current);
        if (dropTarget === notebook.id) onSetDropTarget(null);
      }
    },
    [dropTarget, notebook.id, onSetDropTarget],
  );

  // Single click = instant expand/collapse, double-click = rename
  const handleClick = useCallback(() => {
    setExpanded((prev) => !prev);
  }, []);

  const handleDoubleClick = useCallback(() => {
    onStartEditing(`folder:${notebook.id}`);
  }, [notebook.id, onStartEditing]);

  // Cleanup timers
  useEffect(() => {
    return () => {
      if (autoExpandTimer.current) clearTimeout(autoExpandTimer.current);
    };
  }, []);

  return (
    <div>
      {isEditing ? (
        <InlineRenameInput
          initialValue={notebook.title}
          depth={depth}
          icon={<FolderOpen className="w-3.5 h-3.5 shrink-0 text-dim" />}
          onSubmit={(value) => {
            onRenameNotebook(notebook.id, value);
            onStartEditing(null);
          }}
          onCancel={() => onStartEditing(null)}
        />
      ) : (
        <div
          role="treeitem"
          tabIndex={0}
          draggable
          onDragStart={handleDragStart}
          onDragOver={handleDragOver}
          onDrop={handleDrop}
          onDragEnter={handleDragEnter}
          onDragLeave={handleDragLeave}
          onClick={handleClick}
          onDoubleClick={handleDoubleClick}
          onKeyDown={(e) => {
            if (e.key === "Enter") handleClick();
            if (e.key === "F2") handleDoubleClick();
          }}
          onContextMenu={(e) => {
            e.preventDefault();
            onContextMenu({ kind: "folder", notebook, x: e.clientX, y: e.clientY });
          }}
          className={`w-full flex items-center gap-1 py-1 px-1 rounded-lg text-left text-sm transition-colors cursor-default select-none outline-none focus-visible:ring-1 focus-visible:ring-brand/40 ${
            isDragOver
              ? "bg-white/[0.08] ring-1 ring-white/[0.15]"
              : "text-secondary hover:bg-white/[0.04]"
          }`}
          style={{ paddingLeft: `${depth * 16 + 4}px` }}
        >
          <ChevronRight
            className={`w-3 h-3 shrink-0 text-dim transition-transform ${expanded ? "rotate-90" : ""}`}
          />
          {expanded ? (
            <FolderOpen
              className={`w-3.5 h-3.5 shrink-0 ${itemCount > 0 ? "text-brand/60" : "text-dim/50"}`}
            />
          ) : (
            <FolderClosed
              className={`w-3.5 h-3.5 shrink-0 ${itemCount > 0 ? "text-brand/60" : "text-dim/50"}`}
            />
          )}
          <span
            className={`truncate flex-1 ${
              itemCount === 0 ? "text-dim italic" : ""
            } ${notebook.title === "New Folder" ? "text-dim italic" : ""}`}
          >
            {notebook.title}
          </span>
          {itemCount > 0 && <span className="text-[10px] text-dim shrink-0 mr-1">{itemCount}</span>}
        </div>
      )}

      {expanded && (
        <div className="relative">
          {/* Indent guide */}
          {depth < 4 && (children.length > 0 || childNotes.length > 0) && (
            <div
              className="absolute top-0 bottom-0 w-px bg-white/[0.06]"
              style={{ left: `${depth * 16 + 14}px` }}
            />
          )}
          {children.map((child) => (
            <FolderNode
              key={child.id}
              notebook={child}
              folderChildren={folderChildren}
              notesInFolder={notesInFolder}
              selectedNoteId={selectedNoteId}
              onSelectNote={onSelectNote}
              onContextMenu={onContextMenu}
              editingId={editingId}
              onStartEditing={onStartEditing}
              onRenameNotebook={onRenameNotebook}
              onRenameNote={onRenameNote}
              onMoveNote={onMoveNote}
              onMoveNotebook={onMoveNotebook}
              dropTarget={dropTarget}
              onSetDropTarget={onSetDropTarget}
              depth={depth + 1}
            />
          ))}
          {childNotes.map((note) => (
            <NoteNode
              key={note.id}
              note={note}
              selected={note.id === selectedNoteId}
              onSelect={onSelectNote}
              onContextMenu={onContextMenu}
              editing={editingId === note.id}
              onStartEditing={onStartEditing}
              onRenameNote={onRenameNote}
              depth={depth + 1}
            />
          ))}
        </div>
      )}
    </div>
  );
});

// ── Note Node ────────────────────────────────────────────────────────────

interface NoteNodeProps {
  note: Note;
  selected: boolean;
  onSelect: (id: string) => void;
  onContextMenu: (target: ContextTarget) => void;
  editing: boolean;
  onStartEditing: (id: string | null) => void;
  onRenameNote: (id: string, title: string) => void;
  depth: number;
}

const NoteNode = memo(function NoteNode({
  note,
  selected,
  onSelect,
  onContextMenu,
  editing,
  onStartEditing,
  onRenameNote,
  depth,
}: NoteNodeProps) {
  const dateStr = formatDate(note.updatedAt.slice(0, 10));

  const handleDragStart = useCallback(
    (e: DragEvent) => {
      encodeDrag({ kind: "note", id: note.id }, e);
    },
    [note.id],
  );

  if (editing) {
    return (
      <InlineRenameInput
        initialValue={note.title}
        depth={depth}
        icon={<FileText className="w-3.5 h-3.5 shrink-0 text-dim" />}
        onSubmit={(value) => {
          onRenameNote(note.id, value);
          onStartEditing(null);
        }}
        onCancel={() => onStartEditing(null)}
      />
    );
  }

  return (
    <div
      role="treeitem"
      tabIndex={0}
      draggable
      onDragStart={handleDragStart}
      onClick={() => onSelect(note.id)}
      onDoubleClick={() => onStartEditing(note.id)}
      onKeyDown={(e) => {
        if (e.key === "Enter") onSelect(note.id);
        if (e.key === "F2") onStartEditing(note.id);
      }}
      onContextMenu={(e) => {
        e.preventDefault();
        onContextMenu({ kind: "note", note, x: e.clientX, y: e.clientY });
      }}
      className={`w-full flex items-center gap-1.5 py-1 px-1 rounded-lg text-left text-sm transition-colors cursor-default select-none outline-none focus-visible:ring-1 focus-visible:ring-brand/40 ${
        selected ? "bg-white/[0.08] text-primary" : "text-secondary hover:bg-white/[0.04]"
      }`}
      style={{ paddingLeft: `${depth * 16 + 4}px` }}
    >
      {note.pinned ? (
        <Pin className="w-3 h-3 shrink-0 text-brand" />
      ) : (
        <FileText className={`w-3.5 h-3.5 shrink-0 ${selected ? "text-brand/70" : "text-dim"}`} />
      )}
      <span className={`truncate flex-1 ${note.title === "Untitled" ? "text-dim italic" : ""}`}>
        {note.title}
      </span>
      <span className="text-[10px] text-dim shrink-0 mr-1">{dateStr}</span>
    </div>
  );
});

// ── Inline Rename Input ──────────────────────────────────────────────────

function InlineRenameInput({
  initialValue,
  depth,
  icon,
  onSubmit,
  onCancel,
}: {
  initialValue: string;
  depth: number;
  icon: React.ReactNode;
  onSubmit: (value: string) => void;
  onCancel: () => void;
}) {
  const [value, setValue] = useState(initialValue);

  const handleSubmit = useCallback(() => {
    const trimmed = value.trim();
    if (trimmed && trimmed !== initialValue) {
      onSubmit(trimmed);
    } else {
      onCancel();
    }
  }, [value, initialValue, onSubmit, onCancel]);

  return (
    <div
      className="flex items-center gap-1.5 py-0.5 px-1"
      style={{ paddingLeft: `${depth * 16 + 4}px` }}
    >
      {icon}
      <input
        ref={(el) => {
          if (el) {
            el.focus();
            el.select();
          }
        }}
        type="text"
        value={value}
        onChange={(e) => setValue(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter") handleSubmit();
          if (e.key === "Escape") onCancel();
        }}
        onBlur={handleSubmit}
        className="flex-1 min-w-0 text-sm bg-white/[0.06] border border-brand/40 rounded-md px-1.5 py-0.5 text-primary focus:outline-none"
      />
    </div>
  );
}

// ── Context Menu ─────────────────────────────────────────────────────────

interface TreeContextMenuProps {
  target: NonNullable<ContextTarget>;
  folders: { id: string; title: string }[];
  onCreateNote: (notebookId?: string) => void;
  onCreateNotebook: (parentId?: string) => void;
  onDeleteNote: (id: string) => void;
  onPinNote: (id: string, pinned: boolean) => void;
  onDeleteNotebook: (id: string) => void;
  onStartEditing: (id: string | null) => void;
  onMoveNote: (id: string, notebookId: string | null) => void;
  onClose: () => void;
}

function TreeContextMenu({
  target,
  folders,
  onCreateNote,
  onCreateNotebook,
  onDeleteNote,
  onPinNote,
  onDeleteNotebook,
  onStartEditing,
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
          New folder
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
          New subfolder
        </ContextMenuItem>
        <ContextMenuSeparator />
        <ContextMenuItem
          icon={<Pencil className="w-4 h-4" />}
          onClick={() => {
            onStartEditing(`folder:${target.notebook.id}`);
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
          onStartEditing(note.id);
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
            <span className="text-dim italic">Root</span>
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
