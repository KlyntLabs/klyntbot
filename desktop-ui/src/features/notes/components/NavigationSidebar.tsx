import type { Note, Notebook } from "@shared/types";
import { FileTree } from "./FileTree";
import { NoteSearchBar, type NoteSearchBarHandle } from "./NoteSearchBar";

interface NavigationSidebarProps {
  notebooks: Notebook[];
  notes: Note[];
  searchResults: Note[] | null;
  selectedNoteId: string | null;
  searchRef: React.RefObject<NoteSearchBarHandle | null>;
  onSearchResults: (results: Note[] | null) => void;
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

export function NavigationSidebar({
  notebooks,
  notes,
  searchResults,
  selectedNoteId,
  searchRef,
  onSearchResults,
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
}: NavigationSidebarProps) {
  const displayedNotes = searchResults ?? notes;

  return (
    <div className="flex flex-col gap-2 min-h-0 h-full">
      <NoteSearchBar ref={searchRef} onResults={onSearchResults} />
      <FileTree
        notebooks={searchResults ? [] : notebooks}
        notes={displayedNotes}
        selectedNoteId={selectedNoteId}
        onSelectNote={onSelectNote}
        onCreateNote={onCreateNote}
        onCreateNotebook={onCreateNotebook}
        onDeleteNote={onDeleteNote}
        onPinNote={onPinNote}
        onDeleteNotebook={onDeleteNotebook}
        onRenameNotebook={onRenameNotebook}
        onRenameNote={onRenameNote}
        onMoveNote={onMoveNote}
        onMoveNotebook={onMoveNotebook}
      />
    </div>
  );
}
