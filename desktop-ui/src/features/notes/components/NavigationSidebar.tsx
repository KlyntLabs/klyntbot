import type { InboxItem, Note, Notebook } from "@shared/types";
import { useCallback, useState } from "react";
import { InboxSection } from "./InboxSection";
import { NotebookTree } from "./NotebookTree";
import { TagsExplorer } from "./TagsExplorer";

// ── Props ─────────────────────────────────────────────────────────────

interface NavigationSidebarProps {
  notebooks: Notebook[];
  notes: Note[];
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
  inboxItems: InboxItem[];
  onInboxCreateAsNote: (content: string) => void;
  onInboxDiscard: (id: string) => void;
}

export function NavigationSidebar({
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
  inboxItems,
  onInboxCreateAsNote,
  onInboxDiscard,
}: NavigationSidebarProps) {
  // ── Tags state ────────────────────────────────────────────────────
  const [activeTags, setActiveTags] = useState<string[]>([]);

  const handleToggleTag = useCallback((tag: string, additive: boolean) => {
    setActiveTags((prev) => {
      if (additive) {
        return prev.includes(tag) ? prev.filter((t) => t !== tag) : [...prev, tag];
      }
      return prev.length === 1 && prev[0] === tag ? [] : [tag];
    });
  }, []);

  const handleClearTags = useCallback(() => setActiveTags([]), []);

  const noteCount = notes.length;
  const notebookCount = notebooks.length;

  return (
    <div
      className="glass-sidebar flex flex-col min-h-0 h-full"
      onContextMenu={(e) => e.preventDefault()}
    >
      <div className="flex-1 overflow-y-auto min-h-0 flex flex-col">
        <TagsExplorer
          notes={notes}
          activeTags={activeTags}
          selectedNoteId={selectedNoteId}
          onToggleTag={handleToggleTag}
          onClearTags={handleClearTags}
          onSelectNote={onSelectNote}
        />

        <NotebookTree
          notebooks={notebooks}
          notes={notes}
          selectedNoteId={selectedNoteId}
          autoRenameId={autoRenameId}
          onAutoRenameDone={onAutoRenameDone}
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

        <InboxSection
          items={inboxItems}
          onCreateAsNote={onInboxCreateAsNote}
          onDiscard={onInboxDiscard}
        />

        {/* Footer */}
        <div className="mt-auto shrink-0 px-4 py-2 text-2xs text-dim flex items-center gap-2">
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
    </div>
  );
}
