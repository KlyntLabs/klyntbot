import type { Note, NoteUpdateParams } from "@shared/types";
import { useCallback, useRef } from "react";
import { NoteEditor } from "./NoteEditor";
import { NoteTags, type NoteTagsHandle } from "./NoteTags";

type NotesViewMode = "editor" | "graph";

interface NoteEditorPanelProps {
  note: Note;
  onSave: (params: NoteUpdateParams) => void;
  onRenameNote: (id: string, title: string) => void;
  viewMode: NotesViewMode;
  onViewModeChange: (mode: NotesViewMode) => void;
  onToggleFocusMode?: () => void;
  focusModeActive?: boolean;
  onGenerateCards?: (selectedText?: string) => void;
}

export function NoteEditorPanel({
  note,
  onSave,
  onRenameNote,
  viewMode,
  onViewModeChange,
  onToggleFocusMode,
  focusModeActive,
  onGenerateCards,
}: NoteEditorPanelProps) {
  const titleRef = useRef<HTMLDivElement>(null);
  const tagsRef = useRef<NoteTagsHandle>(null);
  const lastTitleRef = useRef(note.title);

  // Update last known title when note changes
  if (lastTitleRef.current !== note.title && titleRef.current) {
    lastTitleRef.current = note.title;
  }

  const handleTitleBlur = useCallback(() => {
    const el = titleRef.current;
    if (!el) return;
    const newTitle = (el.textContent || "").trim();
    if (newTitle && newTitle !== note.title) {
      onRenameNote(note.id, newTitle);
      lastTitleRef.current = newTitle;
    } else if (!newTitle) {
      // Restore previous title if empty
      el.textContent = note.title;
    }
  }, [note.id, note.title, onRenameNote]);

  const handleTitleKeyDown = useCallback((e: React.KeyboardEvent) => {
    if (e.key === "Enter") {
      e.preventDefault();
      titleRef.current?.blur();
    }
  }, []);

  const handleTagsChange = useCallback(
    (tags: string[]) => {
      onSave({ id: note.id, tags });
    },
    [note.id, onSave],
  );

  return (
    <div className="flex-1 flex flex-col min-w-0 min-h-0">
      {/* Header: title + tags */}
      <div className="px-3 shrink-0">
        {/* Editable title */}
        {/* biome-ignore lint/a11y/useSemanticElements: contentEditable div used as inline title editor */}
        <div
          ref={titleRef}
          role="textbox"
          tabIndex={0}
          contentEditable
          suppressContentEditableWarning
          onBlur={handleTitleBlur}
          onKeyDown={handleTitleKeyDown}
          data-placeholder="Untitled"
          className="text-2xl font-bold text-foreground outline-none min-h-[1.5em] empty:before:content-[attr(data-placeholder)] empty:before:text-muted-foreground/50"
        >
          {note.title}
        </div>

        {/* Tags */}
        <div className="mt-2 flex items-center gap-2">
          <NoteTags ref={tagsRef} tags={note.tags} onChange={handleTagsChange} />
        </div>
      </div>

      {/* Editor */}
      <NoteEditor
        key={note.id}
        note={note}
        onSave={onSave}
        viewMode={viewMode}
        onViewModeChange={onViewModeChange}
        onToggleFocusMode={onToggleFocusMode}
        focusModeActive={focusModeActive}
        onGenerateCards={onGenerateCards}
      />
    </div>
  );
}
