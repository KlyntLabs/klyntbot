import { GitGraph, PenLine } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { useSearchParams } from "react-router";
import { useEvent } from "../../hooks/useEvent";
import { useMutation } from "../../hooks/useMutation";
import { useQuery } from "../../hooks/useQuery";
import type {
  Note,
  Notebook,
  NotebookCreateParams,
  NoteCreateParams,
  NoteUpdateParams,
  SidebarItem,
} from "../../lib/types";
import { Sidebar } from "../layout/Sidebar";
import { GraphView } from "./GraphView";
import { NotebookTree } from "./NotebookTree";
import { NoteEditor } from "./NoteEditor";
import { NoteList } from "./NoteList";
import { NoteSearchBar, type NoteSearchBarHandle } from "./NoteSearchBar";

type NotesViewMode = "editor" | "graph";

export default function NotesView() {
  const { data: notebooks, refetch: refetchNotebooks } = useQuery<Notebook[]>(
    "notebook_list",
    undefined,
    [],
  );
  const { data: notes, refetch: refetchNotes } = useQuery<Note[]>("note_list", undefined, []);
  const [selectedNotebookId, setSelectedNotebookId] = useState<string | null>(null);
  const [selectedNoteId, setSelectedNoteId] = useState<string | null>(null);
  const [viewMode, setNotesViewMode] = useState<NotesViewMode>("editor");
  const [searchResults, setSearchResults] = useState<Note[] | null>(null);
  const [searchParams, setSearchParams] = useSearchParams();

  // Pre-select note from URL search params (e.g. /notes?noteId=xxx)
  useEffect(() => {
    const noteId = searchParams.get("noteId");
    if (noteId) {
      setSelectedNoteId(noteId);
      setSearchParams({}, { replace: true });
    }
  }, [searchParams, setSearchParams]);

  const filteredNotes = selectedNotebookId
    ? notes.filter((n) => n.notebookId === selectedNotebookId)
    : notes;

  const displayedNotes = searchResults ?? filteredNotes;

  const selectedNote = notes.find((n) => n.id === selectedNoteId);

  // ── Mutations ──────────────────────────────────────────────────────────
  const { mutate: createNote } = useMutation<Note, NoteCreateParams>("note_create", "params");
  const { mutate: updateNote } = useMutation<Note, NoteUpdateParams>("note_update", "params");
  const { mutate: deleteNote } = useMutation<boolean, { id: string }>("note_delete");
  const { mutate: createNotebook } = useMutation<Notebook, NotebookCreateParams>(
    "notebook_create",
    "params",
  );

  // ── Handlers ───────────────────────────────────────────────────────────
  const handleCreateNote = useCallback(async () => {
    const result = await createNote({
      title: "Untitled",
      notebookId: selectedNotebookId ?? undefined,
    });
    if (result) setSelectedNoteId(result.id);
  }, [createNote, selectedNotebookId]);

  const handleCreateNotebook = useCallback(async () => {
    await createNotebook({ title: "New Notebook" });
  }, [createNotebook]);

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
    <div className="h-screen w-screen bg-background text-primary flex gap-2 p-2 overflow-hidden">
      <Sidebar active={"Notes" satisfies SidebarItem} />

      <div className="flex-1 flex gap-2 min-w-0">
        <NotebookTree
          notebooks={notebooks}
          selectedId={selectedNotebookId}
          onSelect={setSelectedNotebookId}
          onCreate={handleCreateNotebook}
        />

        <div className="w-64 flex flex-col gap-2 min-h-0">
          <NoteSearchBar ref={searchRef} onResults={setSearchResults} />
          <NoteList
            notes={displayedNotes}
            selectedId={selectedNoteId}
            onSelect={setSelectedNoteId}
            onCreate={handleCreateNote}
            onPin={handlePin}
            onDelete={handleDelete}
          />
        </div>

        <div className="flex-1 flex flex-col gap-2 min-w-0 min-h-0">
          {/* View mode toggle */}
          <div className="flex justify-end gap-1 shrink-0">
            <button
              type="button"
              onClick={() => setNotesViewMode("editor")}
              className={`p-1.5 rounded-lg transition-colors ${
                viewMode === "editor"
                  ? "bg-white/[0.1] text-primary"
                  : "text-dim hover:text-secondary hover:bg-white/[0.04]"
              }`}
              aria-label="Editor view"
            >
              <PenLine className="w-4 h-4" />
            </button>
            <button
              type="button"
              onClick={() => setNotesViewMode("graph")}
              className={`p-1.5 rounded-lg transition-colors ${
                viewMode === "graph"
                  ? "bg-white/[0.1] text-primary"
                  : "text-dim hover:text-secondary hover:bg-white/[0.04]"
              }`}
              aria-label="Graph view"
            >
              <GitGraph className="w-4 h-4" />
            </button>
          </div>

          {viewMode === "graph" ? (
            <GraphView
              notes={notes}
              activeNoteId={selectedNoteId}
              onSelectNote={setSelectedNoteId}
            />
          ) : selectedNote ? (
            <NoteEditor note={selectedNote} onSave={updateNote} />
          ) : (
            <div className="flex-1 glass-panel rounded-2xl flex items-center justify-center">
              <div className="text-muted text-sm">Select a note to view</div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
