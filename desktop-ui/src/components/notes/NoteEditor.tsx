import { History } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { useNavigate } from "react-router";
import { ipc } from "../../hooks/useIpc";
import type { Note, NoteUpdateParams } from "../../lib/types";
import { EditorContentWrapper, useNoteEditor } from "./editor/EditorCore";
import { EditorToolbar } from "./editor/EditorToolbar";
import { EntityMentionMenu } from "./editor/EntityMention";
import { SlashMenu } from "./editor/SlashCommandMenu";
import { WikiLinkMenu } from "./editor/WikiLinkNode";
import { NoteTags } from "./NoteTags";
import { NoteVersionHistory } from "./NoteVersionHistory";

const VERSION_INTERVAL_MS = 5 * 60 * 1000; // 5 minutes

interface NoteEditorProps {
  note: Note;
  onSave: (params: NoteUpdateParams) => void;
}

export function NoteEditor({ note, onSave }: NoteEditorProps) {
  const navigate = useNavigate();
  const saveTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const lastNoteIdRef = useRef(note.id);
  const pendingRef = useRef<{ html: string; text: string } | null>(null);
  const onSaveRef = useRef(onSave);
  onSaveRef.current = onSave;
  const noteContentRef = useRef(note.bodyHtml || note.body || "");
  noteContentRef.current = note.bodyHtml || note.body || "";
  const lastVersionTimeRef = useRef(0);
  const [showHistory, setShowHistory] = useState(false);

  const maybeCreateVersion = useCallback(async (noteId: string) => {
    const now = Date.now();
    if (now - lastVersionTimeRef.current < VERSION_INTERVAL_MS) return;
    lastVersionTimeRef.current = now;
    try {
      await ipc("note_version_create", { noteId });
    } catch {
      // non-critical — version snapshot failure should not block saves
    }
  }, []);

  const flushSave = useCallback(() => {
    if (saveTimerRef.current) {
      clearTimeout(saveTimerRef.current);
      saveTimerRef.current = null;
    }
    const pending = pendingRef.current;
    if (pending) {
      pendingRef.current = null;
      const noteId = lastNoteIdRef.current;
      onSaveRef.current({
        id: noteId,
        body: pending.text,
        bodyHtml: pending.html,
      });
      maybeCreateVersion(noteId);
    }
  }, [maybeCreateVersion]);

  const handleUpdate = useCallback(
    (html: string, text: string) => {
      pendingRef.current = { html, text };
      if (saveTimerRef.current) clearTimeout(saveTimerRef.current);
      saveTimerRef.current = setTimeout(flushSave, 1000);
    },
    [flushSave],
  );

  const handleNavigateNote = useCallback(
    (noteId: string) => navigate(`/notes?noteId=${noteId}`),
    [navigate],
  );

  const handleNavigateEntity = useCallback(
    (entityType: string, entityId: string) => {
      if (entityType === "task") navigate(`/task/${entityId}`);
      else if (entityType === "project") navigate(`/project/${entityId}`);
    },
    [navigate],
  );

  const editor = useNoteEditor({
    content: note.bodyHtml || note.body || "",
    onUpdate: handleUpdate,
    onNavigateNote: handleNavigateNote,
    onNavigateEntity: handleNavigateEntity,
  });

  // Flush on note change and update editor content
  useEffect(() => {
    if (lastNoteIdRef.current !== note.id) {
      flushSave();
      lastNoteIdRef.current = note.id;
      lastVersionTimeRef.current = 0;
      setShowHistory(false);
      if (editor) {
        editor.commands.setContent(noteContentRef.current);
      }
    }
  }, [note.id, editor, flushSave]);

  const handleTagsChange = useCallback(
    (tags: string[]) => {
      onSaveRef.current({ id: note.id, tags });
    },
    [note.id],
  );

  const handleRestore = useCallback(
    (restored: Note) => {
      if (editor) {
        editor.commands.setContent(restored.bodyHtml || restored.body || "");
      }
    },
    [editor],
  );

  // Cmd+S → force save
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === "s") {
        e.preventDefault();
        flushSave();
      }
    };
    document.addEventListener("keydown", handler);
    return () => document.removeEventListener("keydown", handler);
  }, [flushSave]);

  // Flush on unmount
  useEffect(() => {
    return () => flushSave();
  }, [flushSave]);

  return (
    <div className="flex-1 flex gap-2 min-w-0 min-h-0">
      <div className="flex-1 glass-panel rounded-2xl flex flex-col min-w-0 min-h-0">
        <div className="px-4 py-2 border-b border-border">
          <div className="flex items-center justify-between mb-1">
            <h1 className="text-lg font-semibold text-primary">{note.title}</h1>
            <button
              type="button"
              onClick={() => setShowHistory(!showHistory)}
              className={`p-1.5 rounded-lg transition-colors ${
                showHistory
                  ? "bg-white/[0.1] text-primary"
                  : "text-dim hover:text-secondary hover:bg-white/[0.04]"
              }`}
              aria-label="Toggle version history"
            >
              <History className="w-4 h-4" />
            </button>
          </div>
          <NoteTags tags={note.tags} onChange={handleTagsChange} />
          <div className="mt-2">
            <EditorToolbar editor={editor} />
          </div>
        </div>
        <EditorContentWrapper editor={editor} />
        {editor && <SlashMenu editor={editor} />}
        {editor && <WikiLinkMenu editor={editor} />}
        {editor && <EntityMentionMenu editor={editor} />}
      </div>

      {showHistory && <NoteVersionHistory noteId={note.id} onRestore={handleRestore} />}
    </div>
  );
}
