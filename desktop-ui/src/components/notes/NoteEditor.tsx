import { useCallback, useEffect, useRef } from "react";
import type { Note, NoteUpdateParams } from "../../lib/types";
import { EditorContentWrapper, useNoteEditor } from "./editor/EditorCore";
import { EditorToolbar } from "./editor/EditorToolbar";
import { SlashMenu } from "./editor/SlashCommandMenu";
import { WikiLinkMenu } from "./editor/WikiLinkNode";

interface NoteEditorProps {
  note: Note;
  onSave: (params: NoteUpdateParams) => void;
}

export function NoteEditor({ note, onSave }: NoteEditorProps) {
  const saveTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const lastNoteIdRef = useRef(note.id);
  const pendingRef = useRef<{ html: string; text: string } | null>(null);
  const onSaveRef = useRef(onSave);
  onSaveRef.current = onSave;
  const noteContentRef = useRef(note.bodyHtml || note.body || "");
  noteContentRef.current = note.bodyHtml || note.body || "";

  const flushSave = useCallback(() => {
    if (saveTimerRef.current) {
      clearTimeout(saveTimerRef.current);
      saveTimerRef.current = null;
    }
    const pending = pendingRef.current;
    if (pending) {
      pendingRef.current = null;
      onSaveRef.current({
        id: lastNoteIdRef.current,
        body: pending.text,
        bodyHtml: pending.html,
      });
    }
  }, []);

  const handleUpdate = useCallback(
    (html: string, text: string) => {
      pendingRef.current = { html, text };
      if (saveTimerRef.current) clearTimeout(saveTimerRef.current);
      saveTimerRef.current = setTimeout(flushSave, 1000);
    },
    [flushSave],
  );

  const editor = useNoteEditor({
    content: note.bodyHtml || note.body || "",
    onUpdate: handleUpdate,
  });

  // Flush on note change and update editor content
  useEffect(() => {
    if (lastNoteIdRef.current !== note.id) {
      flushSave();
      lastNoteIdRef.current = note.id;
      if (editor) {
        editor.commands.setContent(noteContentRef.current);
      }
    }
  }, [note.id, editor, flushSave]);

  // Flush on unmount
  useEffect(() => {
    return () => flushSave();
  }, [flushSave]);

  return (
    <div className="flex-1 glass-panel rounded-2xl flex flex-col min-w-0 min-h-0">
      <div className="px-4 py-2 border-b border-border">
        <h1 className="text-lg font-semibold text-primary mb-2">{note.title}</h1>
        <EditorToolbar editor={editor} />
      </div>
      <EditorContentWrapper editor={editor} />
      {editor && <SlashMenu editor={editor} />}
      {editor && <WikiLinkMenu editor={editor} />}
    </div>
  );
}
