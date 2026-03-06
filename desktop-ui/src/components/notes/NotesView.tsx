import { useCallback, useState } from "react";
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
import { NotebookTree } from "./NotebookTree";
import { NoteEditor } from "./NoteEditor";
import { NoteList } from "./NoteList";

export default function NotesView() {
  const { data: notebooks, refetch: refetchNotebooks } = useQuery<Notebook[]>(
    "notebook_list",
    undefined,
    [],
  );
  const { data: notes, refetch: refetchNotes } = useQuery<Note[]>("note_list", undefined, []);
  const [selectedNotebookId, setSelectedNotebookId] = useState<string | null>(null);
  const [selectedNoteId, setSelectedNoteId] = useState<string | null>(null);

  const filteredNotes = selectedNotebookId
    ? notes.filter((n) => n.notebookId === selectedNotebookId)
    : notes;

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

        <NoteList
          notes={filteredNotes}
          selectedId={selectedNoteId}
          onSelect={setSelectedNoteId}
          onCreate={handleCreateNote}
          onPin={handlePin}
          onDelete={handleDelete}
        />

        {selectedNote ? (
          <NoteEditor note={selectedNote} onSave={updateNote} />
        ) : (
          <div className="flex-1 glass-panel rounded-2xl flex items-center justify-center">
            <div className="text-muted text-sm">Select a note to view</div>
          </div>
        )}
      </div>
    </div>
  );
}
