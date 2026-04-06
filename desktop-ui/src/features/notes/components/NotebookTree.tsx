import { useClickOutside } from "@shared/hooks/useClickOutside";
import { formatDate } from "@shared/lib/dates";
import type { Notebook, NoteListItem } from "@shared/types";
import { ContextMenu, ContextMenuItem, ContextMenuSeparator, ContextMenuSubmenu } from "@shared/ui";
import {
  Archive,
  Beaker,
  Bookmark,
  BookOpen,
  Box,
  Brain,
  Briefcase,
  Bug,
  Calendar,
  ChevronRight,
  ChevronsDownUp,
  ChevronsUpDown,
  CircleDot,
  Code,
  Coffee,
  Cpu,
  Database,
  Download,
  FileCode,
  FileText,
  Flame,
  FolderClosed,
  FolderInput,
  FolderOpen,
  FolderPlus,
  Gamepad2,
  Globe,
  GraduationCap,
  Heart,
  Home,
  Lightbulb,
  Lock,
  type LucideIcon,
  Map as MapIcon,
  MessageCircle,
  Music,
  Palette,
  Pencil,
  Pin,
  PinOff,
  Plus,
  Rocket,
  Shield,
  ShoppingCart,
  Sparkles,
  Star,
  Target,
  Trash2,
  Trophy,
  Upload,
  Users,
  Wrench,
  X,
  Zap,
} from "lucide-react";
import { memo, useCallback, useEffect, useMemo, useRef, useState } from "react";

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
  notebookId?: string;
  noteCount?: number;
  /** Whether this is the last sibling at its depth (renders └ vs ├) */
  isLastChild?: boolean;
  /** Which ancestor depths have continuing vertical lines */
  guides?: boolean[];
}

interface NotebookTreeProps {
  notebooks: Notebook[];
  notes: NoteListItem[];
  selectedNoteId: string | null;
  autoRenameId: string | null;
  onAutoRenameDone: () => void;
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
  onUpdateNote: (id: string, updates: { icon?: string | null; color?: string | null }) => void;
  onImportFiles?: (paths: string[], notebookId?: string) => void;
  onImportFromDialog?: (notebookId?: string) => void;
  onExportNote?: (noteId: string) => void;
  onExportNotebook?: (notebookId: string) => void;
}

// Icon registry: name → Lucide component. Stored as string in DB.
const ICON_MAP: Record<string, LucideIcon> = {
  // Documents & Files
  "file-text": FileText,
  "file-code": FileCode,
  "book-open": BookOpen,
  bookmark: Bookmark,
  archive: Archive,
  database: Database,
  // Work & Organization
  briefcase: Briefcase,
  calendar: Calendar,
  users: Users,
  "shopping-cart": ShoppingCart,
  box: Box,
  map: MapIcon,
  // Creative & Ideas
  palette: Palette,
  pencil: Pencil,
  lightbulb: Lightbulb,
  sparkles: Sparkles,
  music: Music,
  gamepad: Gamepad2,
  // Science & Tech
  code: Code,
  cpu: Cpu,
  beaker: Beaker,
  bug: Bug,
  wrench: Wrench,
  globe: Globe,
  // Goals & Symbols
  star: Star,
  target: Target,
  rocket: Rocket,
  trophy: Trophy,
  zap: Zap,
  flame: Flame,
  // Life & Misc
  heart: Heart,
  home: Home,
  shield: Shield,
  lock: Lock,
  brain: Brain,
  coffee: Coffee,
  "graduation-cap": GraduationCap,
  "message-circle": MessageCircle,
  "circle-dot": CircleDot,
  pin: Pin,
};

const ICON_NAMES = Object.keys(ICON_MAP);

/** Render a stored icon name as a Lucide component */
export function ItemIcon({
  name,
  className,
  style,
}: {
  name: string;
  className?: string;
  style?: React.CSSProperties;
}) {
  const Icon = ICON_MAP[name];
  if (!Icon) return <FileText className={className} style={style} />;
  return <Icon className={className} style={style} />;
}

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
  | { kind: "note"; note: NoteListItem; x: number; y: number }
  | { kind: "blank"; x: number; y: number }
  | null;

// Drag & drop data stored in dataTransfer
interface DragData {
  type: "notebook" | "note";
  id: string;
}

const INDENT = 16;

// ── Memoized Tree Row ────────────────────────────────────────────────

interface TreeRowProps {
  item: TreeItem;
  isSelected: boolean;
  isRenaming: boolean;
  isDragging: boolean;
  isDropTarget: boolean;
  onSingleClick: (item: TreeItem) => void;
  onDoubleClick: (item: TreeItem) => void;
  onContextMenu: (e: React.MouseEvent, item: TreeItem) => void;
  onKeyDown: (e: React.KeyboardEvent, item: TreeItem) => void;
  onCommitRename: (item: TreeItem, value: string) => void;
  onCancelRename: () => void;
  onDragStart: (e: React.DragEvent, item: TreeItem) => void;
  onDragOver: (e: React.DragEvent, item: TreeItem) => void;
  onDragLeave: () => void;
  onDrop: (e: React.DragEvent, item: TreeItem) => void;
  onDragEnd: () => void;
}

const TreeRow = memo(function TreeRow({
  item,
  isSelected,
  isRenaming,
  isDragging,
  isDropTarget,
  onSingleClick,
  onDoubleClick,
  onContextMenu,
  onKeyDown,
  onCommitRename,
  onCancelRename,
  onDragStart,
  onDragOver,
  onDragLeave,
  onDrop,
  onDragEnd,
}: TreeRowProps) {
  const isFolder = item.type === "notebook";
  const isNote = item.type === "note";

  return (
    <div
      role="treeitem"
      tabIndex={0}
      draggable={!isRenaming}
      onClick={() => onSingleClick(item)}
      onDoubleClick={() => onDoubleClick(item)}
      onContextMenu={(e) => onContextMenu(e, item)}
      onKeyDown={(e) => onKeyDown(e, item)}
      onDragStart={(e) => onDragStart(e, item)}
      onDragOver={(e) => onDragOver(e, item)}
      onDragLeave={onDragLeave}
      onDrop={(e) => onDrop(e, item)}
      onDragEnd={onDragEnd}
      className={`relative flex items-center gap-2 py-1.5 pr-2 rounded-lg text-xs font-light cursor-default select-none outline-none transition-all ${
        isDragging ? "opacity-40" : ""
      } ${
        isDropTarget
          ? "bg-brand/[0.12] ring-1 ring-brand/40"
          : isSelected
            ? "bg-muted text-foreground"
            : "text-muted-foreground hover:bg-accent hover:text-foreground"
      }`}
      style={{ paddingLeft: `${item.depth * INDENT + (item.depth > 0 ? 12 : 8)}px` }}
    >
      {/* Tree guide lines */}
      {item.depth > 0 && item.guides && (
        <>
          {/* Ancestor vertical lines */}
          {item.guides.map((continues, depth) => {
            if (!continues) return null;
            const guideKey = `guide-${item.id}-d${depth}`;
            return (
              <div
                key={guideKey}
                className="absolute top-0 bottom-0 w-px bg-border/40"
                style={{ left: depth * INDENT + 14 }}
              />
            );
          })}
          {/* Own connector: vertical portion */}
          <div
            className="absolute w-px bg-border/40"
            style={{
              left: (item.depth - 1) * INDENT + 14,
              top: 0,
              height: item.isLastChild ? "50%" : "100%",
            }}
          />
          {/* Own connector: horizontal branch */}
          <div
            className="absolute h-px bg-border/40"
            style={{
              left: (item.depth - 1) * INDENT + 14,
              top: "50%",
              width: INDENT - 6,
            }}
          />
        </>
      )}
      {/* Chevron for folders */}
      {isFolder && (
        <ChevronRight
          className={`size-3 shrink-0 text-dim transition-transform duration-150 ${
            item.isExpanded ? "rotate-90" : ""
          }`}
        />
      )}

      {/* Icon */}
      {item.icon && ICON_MAP[item.icon] ? (
        <ItemIcon
          name={item.icon}
          className={`size-3.5 shrink-0 ${item.color ? "" : "text-muted-foreground"}`}
          style={item.color ? { color: item.color } : undefined}
        />
      ) : item.icon?.startsWith("#") ? (
        isFolder ? (
          item.isExpanded ? (
            <FolderOpen className="size-3.5 shrink-0" style={{ color: item.icon }} />
          ) : (
            <FolderClosed className="size-3.5 shrink-0" style={{ color: item.icon }} />
          )
        ) : (
          <FileText className="size-3.5 shrink-0" style={{ color: item.icon }} />
        )
      ) : isFolder ? (
        item.isExpanded ? (
          <FolderOpen
            className={`size-3.5 shrink-0 ${item.color ? "" : "text-muted-foreground"}`}
            style={item.color ? { color: item.color } : undefined}
          />
        ) : (
          <FolderClosed
            className={`size-3.5 shrink-0 ${item.color ? "" : "text-muted-foreground"}`}
            style={item.color ? { color: item.color } : undefined}
          />
        )
      ) : isNote && item.pinned ? (
        <Pin className="size-3 shrink-0 text-brand" />
      ) : (
        <FileText
          className={`size-3.5 shrink-0 ${item.color ? "" : "text-muted-foreground"}`}
          style={item.color ? { color: item.color } : undefined}
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
          className="flex-1 min-w-0 text-xs bg-accent border border-brand/40 rounded-md px-1.5 py-0.5 text-foreground focus:outline-none"
          onBlur={(e) => onCommitRename(item, e.currentTarget.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") e.currentTarget.blur();
            if (e.key === "Escape") onCancelRename();
            e.stopPropagation();
          }}
          onClick={(e) => e.stopPropagation()}
          onDoubleClick={(e) => e.stopPropagation()}
        />
      ) : (
        <span
          className={`truncate flex-1 ${isFolder ? "font-medium" : ""} ${
            item.title === "Untitled" || item.title === "New Folder" ? "text-dim italic" : ""
          }`}
        >
          {item.title}
        </span>
      )}

      {/* Note count for folders */}
      {isFolder && !isRenaming && item.noteCount != null && item.noteCount > 0 && (
        <span className="text-2xs text-muted-foreground shrink-0">{item.noteCount}</span>
      )}

      {/* Date badge for notes */}
      {isNote && item.updatedAt && !isRenaming && (
        <span className="text-2xs text-muted-foreground shrink-0">
          {formatDate(item.updatedAt.slice(0, 10))}
        </span>
      )}
    </div>
  );
});

// ── Main Component ───────────────────────────────────────────────────

export function NotebookTree({
  notebooks,
  notes,
  selectedNoteId,
  autoRenameId,
  onAutoRenameDone,
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
  onUpdateNotebook,
  onUpdateNote,
  onImportFiles,
  onImportFromDialog,
  onExportNote,
  onExportNotebook,
}: NotebookTreeProps) {
  const [expandedIds, setExpandedIds] = useState<Set<string>>(
    () => new Set(notebooks.map((n) => n.id)),
  );
  const [renamingId, setRenamingId] = useState<string | null>(null);
  const [contextMenu, setContextMenu] = useState<ContextTarget>(null);
  const doubleClickTimerRef = useRef<ReturnType<typeof setTimeout>>();
  const pendingClickRef = useRef<string | null>(null);
  const menuRef = useRef<HTMLDivElement>(null);

  // ── Drag & drop state ───────────────────────────────────────────
  const [dragItem, setDragItem] = useState<DragData | null>(null);
  const [dropTargetId, setDropTargetId] = useState<string | null>(null);
  const [dropOnRoot, setDropOnRoot] = useState(false);

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
    const m = new Map<string, NoteListItem>();
    for (const n of notes) m.set(n.id, n);
    return m;
  }, [notes]);

  const allFolders = useMemo(
    () => notebooks.map((nb) => ({ id: nb.id, title: nb.title })),
    [notebooks],
  );

  // Set of all descendant notebook IDs for a given notebook (for cycle detection)
  const getDescendantIds = useCallback(
    (rootId: string): Set<string> => {
      const descendants = new Set<string>();
      const walk = (id: string) => {
        for (const nb of notebooks) {
          if (nb.parentId === id) {
            descendants.add(nb.id);
            walk(nb.id);
          }
        }
      };
      walk(rootId);
      return descendants;
    },
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
        noteCount: childNotes.length,
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
              color: n.color ?? undefined,
              pinned: n.pinned,
              updatedAt: n.updatedAt,
              notebookId: nb.id,
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
        color: n.color ?? undefined,
        pinned: n.pinned,
        updatedAt: n.updatedAt,
      });
    }

    // Compute tree guide metadata for each item
    for (let i = 0; i < result.length; i++) {
      const item = result[i];
      if (item.depth === 0) {
        item.isLastChild = false;
        item.guides = [];
        continue;
      }

      // isLastChild: true if no later item exists at the same depth before depth drops
      let isLast = true;
      for (let j = i + 1; j < result.length; j++) {
        if (result[j].depth < item.depth) break;
        if (result[j].depth === item.depth) {
          isLast = false;
          break;
        }
      }
      item.isLastChild = isLast;

      // guides: for each ancestor depth 0..depth-1, does the vertical line continue?
      const guides: boolean[] = [];
      for (let d = 0; d < item.depth; d++) {
        let continues = false;
        for (let j = i + 1; j < result.length; j++) {
          if (result[j].depth <= d) {
            continues = result[j].depth === d;
            break;
          }
          // If we reach end of list without finding depth <= d, line doesn't continue
        }
        guides.push(continues);
      }
      item.guides = guides;
    }

    return result;
  }, [notebooks, notes, expandedIds]);

  // Auto-rename newly created items
  useEffect(() => {
    if (!autoRenameId) return;
    const exists = items.some((item) => item.id === autoRenameId);
    if (exists) {
      setRenamingId(autoRenameId);
      onAutoRenameDone();
    }
  }, [autoRenameId, items, onAutoRenameDone]);

  // Toggle expand/collapse — immediate, no delay
  const toggleExpand = useCallback((id: string) => {
    setExpandedIds((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }, []);

  // Expand / collapse all
  const expandAll = useCallback(() => {
    setExpandedIds(new Set(notebooks.map((nb) => nb.id)));
  }, [notebooks]);

  const collapseAll = useCallback(() => {
    setExpandedIds(new Set());
  }, []);

  // Single click: immediate action. Double click detected via timer to trigger rename.
  const handleSingleClick = useCallback(
    (item: TreeItem) => {
      if (renamingId === item.id) return;

      // Execute the primary action immediately (no delay!)
      if (item.type === "note") onSelectNote(item.id);
      else toggleExpand(item.id);

      // Track for potential double-click (rename)
      if (pendingClickRef.current === item.id && doubleClickTimerRef.current) {
        clearTimeout(doubleClickTimerRef.current);
        pendingClickRef.current = null;
        setRenamingId(item.id);
        return;
      }

      pendingClickRef.current = item.id;
      if (doubleClickTimerRef.current) clearTimeout(doubleClickTimerRef.current);
      doubleClickTimerRef.current = setTimeout(() => {
        pendingClickRef.current = null;
      }, 300);
    },
    [renamingId, onSelectNote, toggleExpand],
  );

  const handleDoubleClick = useCallback((item: TreeItem) => {
    if (doubleClickTimerRef.current) clearTimeout(doubleClickTimerRef.current);
    pendingClickRef.current = null;
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

  const cancelRename = useCallback(() => setRenamingId(null), []);

  // ── Drag & Drop handlers ──────────────────────────────────────────

  const handleDragStart = useCallback((e: React.DragEvent, item: TreeItem) => {
    const data: DragData = { type: item.type, id: item.id };
    e.dataTransfer.setData("application/json", JSON.stringify(data));
    e.dataTransfer.effectAllowed = "move";
    setDragItem(data);
  }, []);

  const handleDragOver = useCallback(
    (e: React.DragEvent, target: TreeItem) => {
      // Accept external file drops (from Finder/desktop)
      if (e.dataTransfer.types.includes("Files")) {
        e.preventDefault();
        e.dataTransfer.dropEffect = "copy";
        setDropTargetId(target.id);
        return;
      }

      if (!dragItem) return;
      // Can't drop on self
      if (dragItem.id === target.id) return;

      // Notebook → Notebook: check for cycles (can't drop into own descendants)
      if (dragItem.type === "notebook" && target.type === "notebook") {
        const descendants = getDescendantIds(dragItem.id);
        if (descendants.has(target.id)) return;
      }

      // Notes can only be dropped onto folders
      if (dragItem.type === "note" && target.type === "note") {
        // Drop note onto note = move to same folder as target
        e.preventDefault();
        e.dataTransfer.dropEffect = "move";
        setDropTargetId(target.id);
        return;
      }

      if (target.type === "notebook") {
        e.preventDefault();
        e.dataTransfer.dropEffect = "move";
        setDropTargetId(target.id);
      }
    },
    [dragItem, getDescendantIds],
  );

  const handleDragLeave = useCallback(() => {
    setDropTargetId(null);
  }, []);

  const handleDrop = useCallback(
    (e: React.DragEvent, target: TreeItem) => {
      e.preventDefault();
      setDropTargetId(null);
      setDropOnRoot(false);

      // External file drop (from Finder/desktop)
      if (
        e.dataTransfer.types.includes("Files") &&
        !e.dataTransfer.types.includes("application/json")
      ) {
        const files = Array.from(e.dataTransfer.files);
        const paths = files.map((f) => (f as unknown as { path: string }).path).filter(Boolean);
        if (paths.length > 0 && onImportFiles) {
          const targetNotebookId = target.type === "notebook" ? target.id : target.notebookId;
          onImportFiles(paths, targetNotebookId ?? undefined);
        }
        return;
      }

      let data: DragData;
      try {
        data = JSON.parse(e.dataTransfer.getData("application/json"));
      } catch {
        return;
      }

      if (data.id === target.id) return;

      if (data.type === "note") {
        if (target.type === "notebook") {
          // Note → Folder: move into notebook
          onMoveNote(data.id, target.id);
        } else {
          // Note → Note: move to same notebook as target note
          const targetNote = noteMap.get(target.id);
          if (targetNote) {
            onMoveNote(data.id, targetNote.notebookId ?? null);
          }
        }
      } else {
        // Notebook drag
        if (target.type === "notebook") {
          // Notebook → Notebook: reparent (cycle already checked in dragOver)
          const descendants = getDescendantIds(data.id);
          if (!descendants.has(target.id)) {
            onMoveNotebook(data.id, target.id);
          }
        }
        // Notebook → Note doesn't make sense, ignore
      }

      setDragItem(null);
    },
    [onMoveNote, onMoveNotebook, noteMap, getDescendantIds, onImportFiles],
  );

  const handleDragEnd = useCallback(() => {
    setDragItem(null);
    setDropTargetId(null);
    setDropOnRoot(false);
  }, []);

  // Root drop zone: drop onto empty space = move to root/unfiled
  const handleRootDragOver = useCallback(
    (e: React.DragEvent) => {
      // Accept external file drops
      if (e.dataTransfer.types.includes("Files")) {
        e.preventDefault();
        e.dataTransfer.dropEffect = "copy";
        setDropOnRoot(true);
        setDropTargetId(null);
        return;
      }
      if (!dragItem) return;
      e.preventDefault();
      e.dataTransfer.dropEffect = "move";
      setDropOnRoot(true);
      setDropTargetId(null);
    },
    [dragItem],
  );

  const handleRootDragLeave = useCallback(() => {
    setDropOnRoot(false);
  }, []);

  const handleRootDrop = useCallback(
    (e: React.DragEvent) => {
      e.preventDefault();
      setDropOnRoot(false);
      setDropTargetId(null);

      // External file drop (from Finder/desktop) — no notebook target
      if (
        e.dataTransfer.types.includes("Files") &&
        !e.dataTransfer.types.includes("application/json")
      ) {
        const files = Array.from(e.dataTransfer.files);
        const paths = files.map((f) => (f as unknown as { path: string }).path).filter(Boolean);
        if (paths.length > 0 && onImportFiles) {
          onImportFiles(paths); // No notebook = unfiled
        }
        return;
      }

      let data: DragData;
      try {
        data = JSON.parse(e.dataTransfer.getData("application/json"));
      } catch {
        return;
      }

      if (data.type === "note") {
        // Note → Root: unfiled
        onMoveNote(data.id, null);
      } else {
        // Notebook → Root: move to root level
        onMoveNotebook(data.id, null);
      }

      setDragItem(null);
    },
    [onMoveNote, onMoveNotebook, onImportFiles],
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

  // Blank area context menu — fires on any right-click in the tree area that isn't on an item
  const handleBlankContextMenu = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    setContextMenu({ kind: "blank", x: e.clientX, y: e.clientY });
  }, []);

  // F2 to rename
  const handleKeyDown = useCallback((e: React.KeyboardEvent, item: TreeItem) => {
    if (e.key === "F2") {
      e.preventDefault();
      setRenamingId(item.id);
    }
  }, []);

  return (
    // biome-ignore lint/a11y/noStaticElementInteractions: tree area context menu for creating notes/notebooks
    <div className="flex flex-col min-h-0 flex-1" onContextMenu={handleBlankContextMenu}>
      {/* Header */}
      <div className="flex items-center justify-between px-4 pb-1 pt-3">
        <span className="text-2xs uppercase tracking-wider text-muted-foreground font-medium">
          Notebooks
        </span>
        <div className="flex items-center gap-0.5">
          <button
            type="button"
            onClick={() => onCreateNote()}
            className="size-5 rounded-md flex items-center justify-center text-dim hover:text-foreground hover:bg-accent transition-all"
            aria-label="New note"
          >
            <Plus className="size-3.5" />
          </button>
          <button
            type="button"
            onClick={() => onCreateNotebook()}
            className="size-5 rounded-md flex items-center justify-center text-dim hover:text-foreground hover:bg-accent transition-all"
            aria-label="New notebook"
          >
            <FolderPlus className="size-3.5" />
          </button>
        </div>
      </div>

      {/* Tree */}
      <div
        role="tree"
        aria-label="Notebook tree"
        className="flex-1 overflow-y-auto flex flex-col min-h-0 px-3"
      >
        {items.map((item) => (
          <TreeRow
            key={`${item.type}:${item.id}`}
            item={item}
            isSelected={item.type === "note" && item.id === selectedNoteId}
            isRenaming={renamingId === item.id}
            isDragging={dragItem?.id === item.id}
            isDropTarget={dropTargetId === item.id}
            onSingleClick={handleSingleClick}
            onDoubleClick={handleDoubleClick}
            onContextMenu={handleContextMenu}
            onKeyDown={handleKeyDown}
            onCommitRename={commitRename}
            onCancelRename={cancelRename}
            onDragStart={handleDragStart}
            onDragOver={handleDragOver}
            onDragLeave={handleDragLeave}
            onDrop={handleDrop}
            onDragEnd={handleDragEnd}
          />
        ))}

        {/* Root drop zone — catches drops below all items */}
        {/* biome-ignore lint/a11y/noStaticElementInteractions: drag-and-drop root drop zone */}
        <div
          className={`flex-1 min-h-[40px] transition-colors ${
            dropOnRoot ? "bg-brand/[0.08] border-t border-dashed border-brand/30" : ""
          }`}
          onDragOver={handleRootDragOver}
          onDragLeave={handleRootDragLeave}
          onDrop={handleRootDrop}
        />

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
          onExpandAll={expandAll}
          onCollapseAll={collapseAll}
          onClose={() => setContextMenu(null)}
          onImportFromDialog={onImportFromDialog}
          onExportNote={onExportNote}
          onExportNotebook={onExportNotebook}
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
  onUpdateNote: (id: string, updates: { icon?: string | null; color?: string | null }) => void;
  onExpandAll: () => void;
  onCollapseAll: () => void;
  onClose: () => void;
  onImportFromDialog?: (notebookId?: string) => void;
  onExportNote?: (noteId: string) => void;
  onExportNotebook?: (notebookId: string) => void;
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
  onExpandAll,
  onCollapseAll,
  onClose,
  onImportFromDialog,
  onExportNote,
  onExportNotebook,
  ref,
}: TreeContextMenuProps & { ref: React.Ref<HTMLDivElement> }) {
  const [openSubmenu, setOpenSubmenu] = useState<string | null>(null);
  const toggleSubmenu = (name: string) => setOpenSubmenu((prev) => (prev === name ? null : name));

  // Local preview state for appearance picker
  const [previewColor, setPreviewColor] = useState<string | null | undefined>(undefined);
  const [previewIcon, setPreviewIcon] = useState<string | null | undefined>(undefined);

  // Resolve current appearance: local preview overrides target data
  const currentColor = (kind: string) => {
    if (kind === "folder" && target.kind === "folder") {
      return previewColor !== undefined ? previewColor : target.notebook.color;
    }
    if (kind === "note" && target.kind === "note") {
      return previewColor !== undefined ? previewColor : target.note.color;
    }
    return null;
  };
  const currentIcon = (kind: string) => {
    if (kind === "folder" && target.kind === "folder") {
      return previewIcon !== undefined ? previewIcon : target.notebook.icon;
    }
    if (kind === "note" && target.kind === "note") {
      return previewIcon !== undefined ? previewIcon : target.note.icon;
    }
    return null;
  };

  if (target.kind === "blank") {
    return (
      <ContextMenu ref={ref} x={target.x} y={target.y} onClose={onClose}>
        <ContextMenuItem
          icon={<Plus className="size-4" />}
          onClick={() => {
            onCreateNote();
            onClose();
          }}
        >
          New note
        </ContextMenuItem>
        <ContextMenuItem
          icon={<FolderPlus className="size-4" />}
          onClick={() => {
            onCreateNotebook();
            onClose();
          }}
        >
          New notebook
        </ContextMenuItem>
        <ContextMenuSeparator />
        <ContextMenuItem
          icon={<Upload className="size-4" />}
          onClick={() => {
            onImportFromDialog?.();
            onClose();
          }}
        >
          Import files...
        </ContextMenuItem>
        <ContextMenuSeparator />
        <ContextMenuItem
          icon={<ChevronsUpDown className="size-4" />}
          onClick={() => {
            onExpandAll();
            onClose();
          }}
        >
          Expand all
        </ContextMenuItem>
        <ContextMenuItem
          icon={<ChevronsDownUp className="size-4" />}
          onClick={() => {
            onCollapseAll();
            onClose();
          }}
        >
          Collapse all
        </ContextMenuItem>
      </ContextMenu>
    );
  }

  // Shared appearance picker — reused for both folder and note
  const renderAppearancePicker = (
    kind: "folder" | "note",
    entityId: string,
    onUpdate: (id: string, updates: { icon?: string | null; color?: string | null }) => void,
  ) => {
    const activeIcon = currentIcon(kind);
    const activeColor = currentColor(kind);

    return (
      <ContextMenuSubmenu
        icon={<Palette className="size-4" />}
        label="Appearance"
        open={openSubmenu === "appearance"}
        onToggle={() => toggleSubmenu("appearance")}
        panelClassName="absolute left-full top-0 ml-1 py-[5px] w-[280px] rounded-[10px] border border-border bg-[rgb(22,22,24)] shadow-xl animate-[menu-appear_100ms_ease-out]"
      >
        {/* Icon grid */}
        <div className="grid grid-cols-6 gap-0.5 px-1.5 py-1.5">
          {ICON_NAMES.map((name) => {
            const Icon = ICON_MAP[name];
            const isActive = activeIcon === name;
            return (
              <button
                key={name}
                type="button"
                onClick={() => {
                  if (isActive) {
                    setPreviewIcon(null);
                    onUpdate(entityId, { icon: null });
                  } else {
                    setPreviewIcon(name);
                    onUpdate(entityId, { icon: name });
                  }
                }}
                className={`size-[38px] rounded-md flex items-center justify-center transition-colors ${
                  isActive
                    ? "bg-accent ring-1 ring-ring/20"
                    : "text-muted-foreground hover:bg-muted hover:text-foreground"
                }`}
                title={isActive ? `Remove ${name} icon` : name}
              >
                <Icon className="size-4" style={activeColor ? { color: activeColor } : undefined} />
              </button>
            );
          })}
        </div>

        {/* Color row */}
        <div className="h-px bg-border mx-2.5" />
        <div className="flex items-center gap-1.5 px-2.5 py-2.5">
          {ITEM_COLORS.map((color) => (
            <button
              key={color ?? "none"}
              type="button"
              onClick={() => {
                if (!color) {
                  setPreviewColor(null);
                  onUpdate(entityId, { color: null });
                } else {
                  setPreviewColor(color);
                  onUpdate(entityId, { color });
                }
              }}
              className={`size-5 rounded-full border transition-transform hover:scale-125 ${
                activeColor === color && color !== null
                  ? "ring-2 ring-primary/60 ring-offset-1 ring-offset-surface-lowest"
                  : ""
              } ${!color ? "border-border bg-transparent" : "border-transparent"}`}
              style={color ? { backgroundColor: color } : undefined}
              title={color ?? "Reset color"}
            >
              {!color && <X className="size-2.5 text-dim mx-auto" />}
            </button>
          ))}
        </div>

        {/* Reset all button */}
        {(activeIcon || activeColor) && (
          <>
            <div className="h-px bg-border mx-2.5" />
            <button
              type="button"
              onClick={() => {
                setPreviewIcon(null);
                setPreviewColor(null);
                onUpdate(entityId, { icon: null, color: null });
              }}
              className="w-[calc(100%-10px)] mx-[5px] px-2.5 py-[5px] text-[13px] rounded-md text-muted-foreground hover:text-foreground text-left hover:bg-muted transition-colors"
            >
              Reset all
            </button>
          </>
        )}
      </ContextMenuSubmenu>
    );
  };

  if (target.kind === "folder") {
    return (
      <ContextMenu ref={ref} x={target.x} y={target.y} onClose={onClose}>
        <ContextMenuItem
          icon={<Plus className="size-4" />}
          onClick={() => {
            onCreateNote(target.notebook.id);
            onClose();
          }}
        >
          New note here
        </ContextMenuItem>
        <ContextMenuItem
          icon={<FolderPlus className="size-4" />}
          onClick={() => {
            onCreateNotebook(target.notebook.id);
            onClose();
          }}
        >
          New sub-notebook
        </ContextMenuItem>
        <ContextMenuSeparator />
        <ContextMenuItem
          icon={<Pencil className="size-4" />}
          onClick={() => {
            onStartRename(target.notebook.id);
            onClose();
          }}
        >
          Rename
        </ContextMenuItem>

        {renderAppearancePicker("folder", target.notebook.id, onUpdateNotebook)}

        <ContextMenuSeparator />
        <ContextMenuItem
          icon={<Upload className="size-4" />}
          onClick={() => {
            onImportFromDialog?.(target.notebook.id);
            onClose();
          }}
        >
          Import files...
        </ContextMenuItem>
        <ContextMenuItem
          icon={<Download className="size-4" />}
          onClick={() => {
            onExportNotebook?.(target.notebook.id);
            onClose();
          }}
        >
          Export as Markdown...
        </ContextMenuItem>
        <ContextMenuSeparator />
        <ContextMenuItem
          icon={<Trash2 className="size-4" />}
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
        icon={<Pencil className="size-4" />}
        onClick={() => {
          onStartRename(note.id);
          onClose();
        }}
      >
        Rename
      </ContextMenuItem>
      <ContextMenuItem
        icon={note.pinned ? <PinOff className="size-4" /> : <Pin className="size-4" />}
        onClick={() => {
          onPinNote(note.id, !note.pinned);
          onClose();
        }}
      >
        {note.pinned ? "Unpin" : "Pin"}
      </ContextMenuItem>

      {renderAppearancePicker("note", note.id, onUpdateNote)}

      {folders.length > 0 && (
        <ContextMenuSubmenu
          icon={<FolderInput className="size-4" />}
          label="Move to..."
          open={openSubmenu === "move"}
          onToggle={() => toggleSubmenu("move")}
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
                icon={<FolderClosed className="size-3.5 text-dim" />}
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
        icon={<Download className="size-4" />}
        onClick={() => {
          onExportNote?.(note.id);
          onClose();
        }}
      >
        Export as Markdown...
      </ContextMenuItem>
      <ContextMenuSeparator />
      <ContextMenuItem
        icon={<Trash2 className="size-4" />}
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
