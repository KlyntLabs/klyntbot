import { useState } from "react";
import { useMutation } from "../../hooks/useMutation";
import { useQuery } from "../../hooks/useQuery";
import type { Note, Notebook, NoteUpdateParams, SidebarItem } from "../../lib/types";
import { Sidebar } from "../layout/Sidebar";
import { NotebookTree } from "./NotebookTree";
import { NoteEditor } from "./NoteEditor";
import { NoteList } from "./NoteList";

export default function NotesView() {
  const { data: notebooks } = useQuery<Notebook[]>("notebook_list", undefined, []);
  const { data: notes } = useQuery<Note[]>("note_list", undefined, []);
  const [selectedNotebookId, setSelectedNotebookId] = useState<string | null>(null);
  const [selectedNoteId, setSelectedNoteId] = useState<string | null>(null);

  const filteredNotes = selectedNotebookId
    ? notes.filter((n) => n.notebookId === selectedNotebookId)
    : notes;

  const selectedNote = notes.find((n) => n.id === selectedNoteId);
  const { mutate: updateNote } = useMutation<Note, NoteUpdateParams>("note_update", "params");

  const handleCreateNote = () => {
    // Will be wired up in Task 10
  };

  const handleCreateNotebook = () => {
    // Will be wired up in Task 10
  };

  return (
    <div className="h-screen w-screen bg-background text-primary flex gap-2 p-2 overflow-hidden">
      <Sidebar active={"Notes" satisfies SidebarItem} />

      <div className="flex-1 flex gap-2 min-w-0">
        {/* Notebooks sidebar */}
        <NotebookTree
          notebooks={notebooks}
          selectedId={selectedNotebookId}
          onSelect={setSelectedNotebookId}
          onCreate={handleCreateNotebook}
        />

        {/* Note list */}
        <NoteList
          notes={filteredNotes}
          selectedId={selectedNoteId}
          onSelect={setSelectedNoteId}
          onCreate={handleCreateNote}
        />

        {/* Editor panel */}
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
