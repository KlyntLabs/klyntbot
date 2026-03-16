import { ipc } from "@shared/hooks/useIpc";
import { tagBgColor, tagColor } from "@shared/lib/tagColor";
import type { InboxItem, Note, Notebook } from "@shared/types";
import { Search, X } from "lucide-react";
import type { Ref } from "react";
import { useCallback, useEffect, useImperativeHandle, useMemo, useRef, useState } from "react";
import { InboxSection } from "./InboxSection";
import { NotebookTree } from "./NotebookTree";

import { TagsExplorer } from "./TagsExplorer";

// ── Handle for parent to focus search ─────────────────────────────────

export interface NavigationSidebarHandle {
  focus: () => void;
}

// ── Props ─────────────────────────────────────────────────────────────

interface NavigationSidebarProps {
  notebooks: Notebook[];
  notes: Note[];
  selectedNoteId: string | null;
  searchRef: Ref<NavigationSidebarHandle | null>;
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
  inboxItems: InboxItem[];
  onInboxCreateAsNote: (content: string) => void;
  onInboxDiscard: (id: string) => void;
}

export function NavigationSidebar({
  notebooks,
  notes,
  selectedNoteId,
  searchRef,
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
  inboxItems,
  onInboxCreateAsNote,
  onInboxDiscard,
}: NavigationSidebarProps) {
  // ── Search state ──────────────────────────────────────────────────
  const [searchQuery, setSearchQuery] = useState("");
  const [searchResults, setSearchResults] = useState<Note[] | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useImperativeHandle(searchRef, () => ({
    focus: () => inputRef.current?.focus(),
  }));

  // Debounced search
  useEffect(() => {
    if (debounceRef.current) clearTimeout(debounceRef.current);
    const q = searchQuery.trim();
    if (!q) {
      setSearchResults(null);
      return;
    }
    debounceRef.current = setTimeout(async () => {
      try {
        const results = await ipc<Note[]>("note_search", { query: q });
        setSearchResults(results);
      } catch {
        setSearchResults(null);
      }
    }, 200);
    return () => {
      if (debounceRef.current) clearTimeout(debounceRef.current);
    };
  }, [searchQuery]);

  const clearSearch = useCallback(() => {
    setSearchQuery("");
    setSearchResults(null);
  }, []);

  // Escape clears search
  const handleSearchKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        clearSearch();
        inputRef.current?.blur();
      }
    },
    [clearSearch],
  );

  // ── Tags state ────────────────────────────────────────────────────
  const [activeTags, setActiveTags] = useState<string[]>([]);

  const handleToggleTag = useCallback((tag: string, additive: boolean) => {
    setActiveTags((prev) => {
      if (additive) {
        // Cmd+click: toggle in multi-select
        return prev.includes(tag) ? prev.filter((t) => t !== tag) : [...prev, tag];
      }
      // Normal click: single select toggle
      return prev.length === 1 && prev[0] === tag ? [] : [tag];
    });
  }, []);

  const handleClearTags = useCallback(() => setActiveTags([]), []);

  // ── Footer stats ──────────────────────────────────────────────────
  const noteCount = notes.length;
  const notebookCount = notebooks.length;

  // ── Searching mode ────────────────────────────────────────────────
  const isSearching = searchResults !== null;

  return (
    <div className="flex flex-col gap-1 min-h-0 h-full">
      {/* 1. Search Bar */}
      <div className="relative glass-sidebar shrink-0">
        <Search className="absolute left-2.5 top-1/2 -translate-y-1/2 w-3.5 h-3.5 text-dim" />
        <input
          ref={inputRef}
          type="text"
          placeholder="Search notes..."
          value={searchQuery}
          onChange={(e) => setSearchQuery(e.target.value)}
          onKeyDown={handleSearchKeyDown}
          className="w-full text-xs bg-transparent border-none rounded-lg pl-8 pr-7 py-1.5 text-primary placeholder:text-dim focus:outline-none"
        />
        {searchQuery && (
          <button
            type="button"
            onClick={clearSearch}
            className="absolute right-2 top-1/2 -translate-y-1/2 text-dim hover:text-secondary"
            aria-label="Clear search"
          >
            <X className="w-3.5 h-3.5" />
          </button>
        )}
      </div>

      {/* Search results or normal sections */}
      {isSearching ? (
        <div className="flex-1 overflow-y-auto min-h-0 px-1">
          <div className="text-[10px] uppercase tracking-wider text-dim px-2 py-1">
            {searchResults.length} result{searchResults.length !== 1 ? "s" : ""}
          </div>
          <div className="flex flex-col gap-0.5">
            {searchResults.map((note) => (
              <SearchResultItem
                key={note.id}
                note={note}
                isSelected={note.id === selectedNoteId}
                onSelect={onSelectNote}
              />
            ))}
          </div>
        </div>
      ) : (
        <div className="flex-1 overflow-y-auto min-h-0 flex flex-col gap-2">
          {/* Tags Explorer */}
          <TagsExplorer
            notes={notes}
            activeTags={activeTags}
            selectedNoteId={selectedNoteId}
            onToggleTag={handleToggleTag}
            onClearTags={handleClearTags}
            onSelectNote={onSelectNote}
          />

          {/* 4. Notebook Tree */}
          <NotebookTree
            notebooks={notebooks}
            notes={notes}
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
            onUpdateNotebook={onUpdateNotebook}
            onUpdateNote={onUpdateNote}
          />

          {/* 5. Inbox */}
          <InboxSection
            items={inboxItems}
            onCreateAsNote={onInboxCreateAsNote}
            onDiscard={onInboxDiscard}
          />

          {/* 6. Footer */}
          <div className="mt-auto shrink-0 px-2 py-1.5 text-[10px] text-dim flex items-center gap-2">
            <span>
              {noteCount} note{noteCount !== 1 ? "s" : ""}
            </span>
            <span className="opacity-40">·</span>
            <span>
              {notebookCount} notebook{notebookCount !== 1 ? "s" : ""}
            </span>
            {inboxItems.length > 0 && (
              <>
                <span className="opacity-40">·</span>
                <span>Inbox ({inboxItems.length})</span>
              </>
            )}
          </div>
        </div>
      )}
    </div>
  );
}

// ── Search result item with snippet + tag pills ───────────────────────

function SearchResultItem({
  note,
  isSelected,
  onSelect,
}: {
  note: Note;
  isSelected: boolean;
  onSelect: (id: string) => void;
}) {
  const snippet = useMemo(() => {
    if (!note.body) return "";
    // Strip markdown and take first 80 chars
    const plain = note.body
      .replace(/^#+\s*/gm, "")
      .replace(/[*_~`]/g, "")
      .replace(/\[([^\]]*)\]\([^)]*\)/g, "$1")
      .replace(/\n+/g, " ")
      .trim();
    return plain.length > 80 ? `${plain.slice(0, 80)}...` : plain;
  }, [note.body]);

  return (
    <button
      type="button"
      onClick={() => onSelect(note.id)}
      className={`flex flex-col gap-0.5 px-2 py-1.5 rounded text-left w-full transition-colors ${
        isSelected ? "bg-white/[0.08] text-primary" : "text-secondary hover:bg-white/[0.04]"
      }`}
    >
      <span className="text-sm truncate">{note.title || "Untitled"}</span>
      {note.tags.length > 0 && (
        <div className="flex flex-wrap gap-0.5">
          {note.tags.slice(0, 3).map((tag) => (
            <span
              key={tag}
              className="text-[9px] px-1 py-px rounded-full"
              style={{ color: tagColor(tag), backgroundColor: tagBgColor(tag) }}
            >
              {tag}
            </span>
          ))}
        </div>
      )}
      {snippet && <span className="text-[11px] text-muted truncate">{snippet}</span>}
    </button>
  );
}
