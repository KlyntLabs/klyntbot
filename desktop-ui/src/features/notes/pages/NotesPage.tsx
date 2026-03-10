import { useEvent } from "@shared/hooks/useEvent";
import { useMutation } from "@shared/hooks/useMutation";
import { useQuery } from "@shared/hooks/useQuery";
import type {
  Note,
  Notebook,
  NotebookCreateParams,
  NoteCreateParams,
  NoteUpdateParams,
} from "@shared/types";
import { FileText, GitGraph, PenLine } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useSearchParams } from "react-router";
import { FileTree } from "../components/FileTree";
import { GraphView } from "../components/GraphView";
import { NoteEditor } from "../components/NoteEditor";
import { NoteSearchBar, type NoteSearchBarHandle } from "../components/NoteSearchBar";

type NotesViewMode = "editor" | "graph";

function ViewModeToggle({
  viewMode,
  onChange,
}: {
  viewMode: NotesViewMode;
  onChange: (mode: NotesViewMode) => void;
}) {
  return (
    <div className="flex items-center gap-1">
      <button
        type="button"
        aria-label="Editor view"
        onClick={() => onChange("editor")}
        className={`p-1.5 rounded-md transition-colors ${viewMode === "editor" ? "bg-white/10 text-primary" : "text-muted hover:text-secondary"}`}
      >
        <PenLine size={16} />
      </button>
      <button
        type="button"
        aria-label="Graph view"
        onClick={() => onChange("graph")}
        className={`p-1.5 rounded-md transition-colors ${viewMode === "graph" ? "bg-white/10 text-primary" : "text-muted hover:text-secondary"}`}
      >
        <GitGraph size={16} />
      </button>
    </div>
  );
}

export default function NotesPage() {
  const { data: notebooks, refetch: refetchNotebooks } = useQuery<Notebook[]>(
    "notebook_list",
    undefined,
    [],
  );
  const { data: notes, refetch: refetchNotes } = useQuery<Note[]>("note_list", undefined, []);
  const [selectedNoteId, setSelectedNoteId] = useState<string | null>(null);
  const [viewMode, setNotesViewMode] = useState<NotesViewMode>("editor");
  const [searchResults, setSearchResults] = useState<Note[] | null>(null);
  const [searchParams, setSearchParams] = useSearchParams();
  const sidebarRef = useRef<HTMLDivElement>(null);
  const sidebarWidthRef = useRef(260);

  const containerRef = useRef<HTMLDivElement>(null);

  const onResizeStart = useCallback((e: React.PointerEvent) => {
    e.preventDefault();
    const startX = e.clientX;
    const startW = sidebarWidthRef.current;
    let raf = 0;

    // Disable backdrop-filter during drag — it's the main perf bottleneck
    containerRef.current?.classList.add("resizing");

    const onMove = (ev: globalThis.PointerEvent) => {
      cancelAnimationFrame(raf);
      raf = requestAnimationFrame(() => {
        const newW = Math.min(480, Math.max(180, startW + ev.clientX - startX));
        sidebarWidthRef.current = newW;
        if (sidebarRef.current) sidebarRef.current.style.width = `${newW}px`;
      });
    };
    const onUp = () => {
      cancelAnimationFrame(raf);
      containerRef.current?.classList.remove("resizing");
      document.removeEventListener("pointermove", onMove);
      document.removeEventListener("pointerup", onUp);
    };
    document.addEventListener("pointermove", onMove);
    document.addEventListener("pointerup", onUp);
  }, []);

  // Pre-select note from URL search params (e.g. /notes?noteId=xxx)
  useEffect(() => {
    const noteId = searchParams.get("noteId");
    if (noteId) {
      setSelectedNoteId(noteId);
      setSearchParams({}, { replace: true });
    }
  }, [searchParams, setSearchParams]);

  const noteMap = useMemo(() => {
    const map = new Map<string, Note>();
    for (const n of notes) map.set(n.id, n);
    return map;
  }, [notes]);

  const selectedNote = selectedNoteId ? noteMap.get(selectedNoteId) : undefined;

  // When searching, display search results in a flat tree-like way
  const displayedNotes = searchResults ?? notes;

  // ── Mutations ──────────────────────────────────────────────────────────
  const { mutate: createNote } = useMutation<Note, NoteCreateParams>("note_create", "params");
  const { mutate: updateNote } = useMutation<Note, NoteUpdateParams>("note_update", "params");
  const { mutate: deleteNote } = useMutation<boolean, { id: string }>("note_delete");
  const { mutate: createNotebook } = useMutation<Notebook, NotebookCreateParams>(
    "notebook_create",
    "params",
  );
  const { mutate: deleteNotebook } = useMutation<boolean, { id: string }>("notebook_delete");
  const { mutate: updateNotebook } = useMutation<
    Notebook,
    { id: string; title?: string; parentId?: string | null }
  >("notebook_update", "params");

  // ── Handlers ───────────────────────────────────────────────────────────
  const handleCreateNote = useCallback(
    async (notebookId?: string) => {
      const result = await createNote({
        title: "Untitled",
        notebookId,
      });
      if (result) setSelectedNoteId(result.id);
    },
    [createNote],
  );

  const handleCreateNotebook = useCallback(
    async (parentId?: string) => {
      await createNotebook({ title: "New Folder", parentId });
    },
    [createNotebook],
  );

  const handlePin = useCallback(
    (id: string, pinned: boolean) => {
      updateNote({ id, pinned });
    },
    [updateNote],
  );

  const handleDelete = useCallback(
    async (id: string) => {
      const deleted = await deleteNote({ id });
      if (deleted && selectedNoteId === id) {
        setSelectedNoteId(null);
      }
    },
    [deleteNote, selectedNoteId],
  );

  const handleDeleteNotebook = useCallback(
    async (id: string) => {
      await deleteNotebook({ id });
    },
    [deleteNotebook],
  );

  const handleRenameNotebook = useCallback(
    async (id: string, title: string) => {
      await updateNotebook({ id, title });
    },
    [updateNotebook],
  );

  const handleRenameNote = useCallback(
    async (id: string, title: string) => {
      await updateNote({ id, title });
    },
    [updateNote],
  );

  const handleMoveNote = useCallback(
    async (id: string, notebookId: string | null) => {
      await updateNote({ id, notebookId });
    },
    [updateNote],
  );

  const handleMoveNotebook = useCallback(
    async (id: string, parentId: string | null) => {
      await updateNotebook({ id, parentId });
    },
    [updateNotebook],
  );

  const searchRef = useRef<NoteSearchBarHandle>(null);

  // ── Keyboard shortcuts ─────────────────────────────────────────────────
  const handleCreateNoteRef = useRef(handleCreateNote);
  handleCreateNoteRef.current = handleCreateNote;
  const handleDeleteRef = useRef(handleDelete);
  handleDeleteRef.current = handleDelete;
  const selectedNoteIdRef = useRef(selectedNoteId);
  selectedNoteIdRef.current = selectedNoteId;

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      const mod = e.metaKey || e.ctrlKey;
      if (!mod) return;

      if (e.key === "n" && !e.shiftKey) {
        e.preventDefault();
        handleCreateNoteRef.current();
      } else if (e.key === "Backspace" && selectedNoteIdRef.current) {
        e.preventDefault();
        handleDeleteRef.current(selectedNoteIdRef.current);
      } else if (e.key === "f" && e.shiftKey) {
        e.preventDefault();
        searchRef.current?.focus();
      }
    };
    document.addEventListener("keydown", handler);
    return () => document.removeEventListener("keydown", handler);
  }, []);

  // ── Event refresh ──────────────────────────────────────────────────────
  useEvent<{ entityKind: string }>("entity:updated", (payload) => {
    if (payload.entityKind === "note") refetchNotes();
    if (payload.entityKind === "notebook") {
      refetchNotebooks();
      refetchNotes();
    }
  });

  return (
    <div ref={containerRef} className="flex-1 flex min-w-0">
      {/* File tree sidebar — resizable */}
      <div ref={sidebarRef} className="flex flex-col gap-2 min-h-0 shrink-0" style={{ width: 260 }}>
        <NoteSearchBar ref={searchRef} onResults={setSearchResults} />
        <FileTree
          notebooks={searchResults ? [] : notebooks}
          notes={displayedNotes}
          selectedNoteId={selectedNoteId}
          onSelectNote={setSelectedNoteId}
          onCreateNote={handleCreateNote}
          onCreateNotebook={handleCreateNotebook}
          onDeleteNote={handleDelete}
          onPinNote={handlePin}
          onDeleteNotebook={handleDeleteNotebook}
          onRenameNotebook={handleRenameNotebook}
          onRenameNote={handleRenameNote}
          onMoveNote={handleMoveNote}
          onMoveNotebook={handleMoveNotebook}
        />
      </div>

      {/* Resize handle */}
      <div
        onPointerDown={onResizeStart}
        className="w-1 shrink-0 cursor-col-resize group flex items-center justify-center"
      >
        <div className="w-px h-full group-hover:bg-brand/40 transition-colors" />
      </div>

      {/* Editor area */}
      <div className="flex-1 flex flex-col min-w-0 min-h-0 pl-1">
        {viewMode === "graph" ? (
          <>
            {/* Minimal toggle bar for graph/empty views */}
            <div className="flex justify-end shrink-0 px-3 pt-3">
              <ViewModeToggle viewMode={viewMode} onChange={setNotesViewMode} />
            </div>
            <GraphView
              notes={notes}
              activeNoteId={selectedNoteId}
              onSelectNote={setSelectedNoteId}
            />
          </>
        ) : selectedNote ? (
          <NoteEditor
            note={selectedNote}
            onSave={updateNote}
            viewMode={viewMode}
            onViewModeChange={setNotesViewMode}
          />
        ) : (
          <>
            {/* Minimal toggle bar for empty state */}
            <div className="flex justify-end shrink-0 px-3 pt-3">
              <ViewModeToggle viewMode={viewMode} onChange={setNotesViewMode} />
            </div>
            <div className="flex-1 flex flex-col items-center justify-center gap-3">
              <div className="w-12 h-12 rounded-2xl bg-white/[0.04] flex items-center justify-center">
                <FileText className="w-6 h-6 text-dim" strokeWidth={1.5} />
              </div>
              <div className="text-center">
                <div className="text-muted text-sm">Select a note to view</div>
                <div className="text-dim text-xs mt-1">
                  or press{" "}
                  <kbd className="px-1.5 py-0.5 rounded bg-white/[0.06] text-[10px] font-mono">
                    Cmd+N
                  </kbd>{" "}
                  to create one
                </div>
              </div>
            </div>
          </>
        )}
      </div>
    </div>
  );
}
