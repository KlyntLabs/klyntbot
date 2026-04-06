import { ipc } from "@shared/hooks/useIpc";
import { useQuery } from "@shared/hooks/useQuery";
import type { NoteListItem, Notebook } from "@shared/types";
import { BookOpen, Search } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";

interface NoteSidebarProps {
  notebookIds: string[];
  notes: NoteListItem[];
  loading: boolean;
  selectedNoteId: string | null;
  onSelectNote: (noteId: string) => void;
}

export function NoteSidebar({
  notebookIds,
  notes,
  loading,
  selectedNoteId,
  onSelectNote,
}: NoteSidebarProps) {
  const [searchQuery, setSearchQuery] = useState("");
  const [searchResults, setSearchResults] = useState<NoteListItem[] | null>(null);
  const [searching, setSearching] = useState(false);
  const searchTimer = useRef<ReturnType<typeof setTimeout>>();

  // Clean up debounce timer on unmount
  useEffect(() => () => clearTimeout(searchTimer.current), []);

  // Fetch notebook metadata for the tree
  const { data: allNotebooks } = useQuery<Notebook[]>("notebook_list", undefined, []);

  const projectNotebooks = useMemo(
    () => allNotebooks.filter((nb) => notebookIds.includes(nb.id)),
    [allNotebooks, notebookIds],
  );

  // Recent notes — top 10 by updatedAt
  const recentNotes = useMemo(() => [...notes].slice(0, 10), [notes]);

  const handleSearch = useCallback(
    async (query: string) => {
      if (!query.trim()) {
        setSearchResults(null);
        return;
      }
      setSearching(true);
      try {
        const results = await Promise.all(
          notebookIds.map((notebookId) =>
            ipc<NoteListItem[]>("note_search_hybrid", {
              query,
              notebookId,
            }).catch(() => [] as NoteListItem[]),
          ),
        );
        const merged = results.flat();
        const seen = new Set<string>();
        const deduped = merged.filter((n) => {
          if (seen.has(n.id)) return false;
          seen.add(n.id);
          return true;
        });
        setSearchResults(deduped);
      } finally {
        setSearching(false);
      }
    },
    [notebookIds],
  );

  function handleSearchInput(value: string) {
    setSearchQuery(value);
    clearTimeout(searchTimer.current);
    searchTimer.current = setTimeout(() => handleSearch(value), 300);
  }

  const displayNotes = searchResults ?? recentNotes;

  return (
    <div className="w-56 flex flex-col border-r border-border h-full">
      {/* Search */}
      <div className="p-3">
        <div className="relative">
          <Search className="absolute left-2.5 top-1/2 -translate-y-1/2 size-3.5 text-muted-foreground" />
          <input
            type="text"
            value={searchQuery}
            onChange={(e) => handleSearchInput(e.target.value)}
            placeholder="Search notes..."
            className="glass-input w-full pl-8 pr-3 py-1.5 text-xs rounded-lg"
          />
        </div>
      </div>

      {/* Notebook tree */}
      <div className="px-3 pb-2">
        <h3 className="text-2xs font-medium text-muted-foreground uppercase tracking-wider mb-1.5">
          Notebooks
        </h3>
        {projectNotebooks.length === 0 ? (
          <p className="text-xs text-muted-foreground italic">No linked notebooks</p>
        ) : (
          <div className="space-y-0.5">
            {projectNotebooks.map((nb) => (
              <div
                key={nb.id}
                className="flex items-center gap-2 px-2 py-1 rounded text-xs text-muted-foreground"
              >
                <BookOpen className="size-3.5 flex-shrink-0" />
                <span className="truncate">{nb.title}</span>
                <span className="ml-auto text-2xs opacity-60">{nb.noteCount}</span>
              </div>
            ))}
          </div>
        )}
      </div>

      {/* Divider */}
      <div className="h-px bg-border mx-3" />

      {/* Notes list */}
      <div className="flex-1 overflow-y-auto px-3 py-2">
        <h3 className="text-2xs font-medium text-muted-foreground uppercase tracking-wider mb-1.5">
          {searchResults ? "Search Results" : "Recent Notes"}
        </h3>
        {loading || searching ? (
          <p className="text-xs text-muted-foreground italic py-2">Loading...</p>
        ) : displayNotes.length === 0 ? (
          <p className="text-xs text-muted-foreground italic py-2">
            {searchResults ? "No results found" : "No notes yet"}
          </p>
        ) : (
          <div className="space-y-0.5">
            {displayNotes.map((note) => (
              <button
                key={note.id}
                type="button"
                onClick={() => onSelectNote(note.id)}
                className={`w-full text-left px-2 py-1.5 rounded text-xs transition-colors ${
                  selectedNoteId === note.id
                    ? "bg-brand/10 text-foreground"
                    : "text-muted-foreground hover:text-foreground hover:bg-accent"
                }`}
              >
                <div className="truncate font-medium">{note.title || "Untitled"}</div>
                <div className="text-2xs opacity-60 mt-0.5">
                  {new Date(note.updatedAt).toLocaleDateString()}
                </div>
              </button>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
