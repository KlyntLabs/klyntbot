import { ipc } from "@shared/hooks/useIpc";
import type { Note, NoteUpdateParams } from "@shared/types";
import { GitGraph, History, PenLine, Plus } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { useNavigate } from "react-router";
import { EditorContentWrapper, useEntityResolution, useNoteEditor } from "./editor/EditorCore";
import { EditorToolbar } from "./editor/EditorToolbar";
import { EntityMentionMenu } from "./editor/EntityMention";
import { SlashMenu } from "./editor/SlashCommandMenu";
import { VimCommandLine } from "./editor/VimCommandLine";
import { getVimPlugin } from "./editor/vim";
import { VIM_SAVE_EVENT } from "./editor/vim/VimPlugin";
import type { VimMode } from "./editor/vim/VimState";
import { WikiLinkMenu } from "./editor/WikiLinkNode";
import { NoteTags, type NoteTagsHandle } from "./NoteTags";
import { NoteVersionHistory } from "./NoteVersionHistory";

const VERSION_INTERVAL_MS = 5 * 60 * 1000; // Minimum interval between auto-saving version snapshots

type NotesViewMode = "editor" | "graph";

interface NoteEditorProps {
  note: Note;
  onSave: (params: NoteUpdateParams) => void;
  viewMode: NotesViewMode;
  onViewModeChange: (mode: NotesViewMode) => void;
}

export function NoteEditor({ note, onSave, viewMode, onViewModeChange }: NoteEditorProps) {
  const navigate = useNavigate();
  const tagsRef = useRef<NoteTagsHandle>(null);
  const saveTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const lastNoteIdRef = useRef(note.id);
  const pendingRef = useRef<{ html: string; markdown: string } | null>(null);
  const onSaveRef = useRef(onSave);
  onSaveRef.current = onSave;
  const noteContentRef = useRef(note.bodyHtml || note.body || "");
  noteContentRef.current = note.bodyHtml || note.body || "";
  const lastVersionTimeRef = useRef(0);
  const [showHistory, setShowHistory] = useState(false);

  // Vim state
  const [vimEnabled, setVimEnabled] = useState(
    () => localStorage.getItem("klyntbot:notes:vimMode") === "true",
  );
  const [vimMode, setVimMode] = useState<VimMode>("normal");
  const [commandLine, setCommandLine] = useState<{ prefix: string } | null>(null);

  // Stable ref for the enabled callback so the ProseMirror plugin always reads
  // the latest value without needing to recreate the editor.
  const vimEnabledRef = useRef(vimEnabled);
  vimEnabledRef.current = vimEnabled;

  const toggleVim = useCallback(() => {
    setVimEnabled((prev) => {
      const next = !prev;
      localStorage.setItem("klyntbot:notes:vimMode", String(next));
      return next;
    });
    // Reset to normal mode when toggling
    setVimMode("normal");
  }, []);

  // Stable vim callbacks — never change identity
  const vimCallbacks = useRef({
    onStateChange: (state: { mode: VimMode }) => setVimMode(state.mode),
    onOpenCommandLine: (prefix: string) => setCommandLine({ prefix }),
    enabled: () => vimEnabledRef.current,
  }).current;

  const maybeCreateVersion = useCallback(async (noteId: string) => {
    const now = Date.now();
    if (now - lastVersionTimeRef.current < VERSION_INTERVAL_MS) return;
    lastVersionTimeRef.current = now;
    try {
      await ipc("note_version_create", { noteId });
    } catch (e) {
      console.warn("Failed to create version snapshot:", e);
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
        body: pending.markdown,
        bodyHtml: pending.html,
      });
      maybeCreateVersion(noteId);
    }
  }, [maybeCreateVersion]);

  const handleUpdate = useCallback(
    (html: string, markdown: string) => {
      pendingRef.current = { html, markdown };
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
    vimOptions: vimCallbacks,
  });

  // Resolve entity mentions and wiki links — grays out non-existent references
  useEntityResolution(editor);

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

  // vim:save event → force save (dispatched by VimPlugin on :w command)
  useEffect(() => {
    const handler = () => flushSave();
    document.addEventListener(VIM_SAVE_EVENT, handler);
    return () => document.removeEventListener(VIM_SAVE_EVENT, handler);
  }, [flushSave]);

  // Flush on unmount
  useEffect(() => {
    return () => flushSave();
  }, [flushSave]);

  // Command line handlers
  const handleCommandLineSubmit = useCallback(
    (value: string) => {
      if (!commandLine) return;

      if (commandLine.prefix === "/") {
        // Search — set pattern on the vim plugin
        if (editor && value) {
          const vim = getVimPlugin(editor);
          vim?.setSearchPattern(value, "forward");
        }
      } else if (commandLine.prefix === ":") {
        // Ex command
        if (value === "w" || value === "write") {
          flushSave();
        } else if (editor) {
          const vim = getVimPlugin(editor);
          vim?.executeCommand(value);
        }
      }

      setCommandLine(null);
      // Return focus to editor
      editor?.commands.focus();
    },
    [commandLine, editor, flushSave],
  );

  const handleCommandLineCancel = useCallback(() => {
    setCommandLine(null);
    editor?.commands.focus();
  }, [editor]);

  // Build editor content class — add vim mode class when vim is enabled
  const editorContentClass = vimEnabled
    ? `flex-1 min-h-0 overflow-y-auto vim-${vimMode}`
    : "flex-1 min-h-0 overflow-y-auto";

  return (
    <div className="flex-1 flex gap-2 min-w-0 min-h-0">
      <div className="flex-1 flex flex-col min-w-0 min-h-0">
        {/* Controls bar */}
        <div className="px-2 pb-0 shrink-0 flex flex-col gap-2">
          {/* Row 1: Formatting toolbar */}
          <EditorToolbar
            editor={editor}
            vimEnabled={vimEnabled}
            vimMode={vimMode}
            onToggleVim={toggleVim}
          />

          {/* Row 2: Title + tags + toggles */}
          <div className="flex items-center gap-2.5">
            <div className="flex items-center gap-2 flex-1 min-w-0">
              <span className="text-sm font-semibold text-primary truncate shrink-0 max-w-[240px]">
                {note.title}
              </span>
              <div className="w-px h-3.5 bg-white/[0.08] shrink-0" />
              <NoteTags ref={tagsRef} tags={note.tags} onChange={handleTagsChange} />
            </div>
            <div className="flex items-center gap-0.5 shrink-0">
              <button
                type="button"
                onClick={() => tagsRef.current?.startAdding()}
                className="p-1.5 rounded-lg transition-all text-dim hover:text-secondary hover:bg-white/[0.04]"
                aria-label="Add tag"
              >
                <Plus className="w-3.5 h-3.5" />
              </button>
              <div className="w-px h-4 bg-white/[0.08] mx-0.5" />
              <button
                type="button"
                onClick={() => setShowHistory(!showHistory)}
                className={`p-1.5 rounded-lg transition-all ${
                  showHistory
                    ? "bg-white/[0.1] text-primary"
                    : "text-dim hover:text-secondary hover:bg-white/[0.04]"
                }`}
                aria-label="Toggle version history"
              >
                <History className="w-4 h-4" />
              </button>
              <button
                type="button"
                onClick={() => onViewModeChange("editor")}
                className={`p-1.5 rounded-lg transition-all ${
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
                onClick={() => onViewModeChange("graph")}
                className={`p-1.5 rounded-lg transition-all ${
                  viewMode === "graph"
                    ? "bg-white/[0.1] text-primary"
                    : "text-dim hover:text-secondary hover:bg-white/[0.04]"
                }`}
                aria-label="Graph view"
              >
                <GitGraph className="w-4 h-4" />
              </button>
            </div>
          </div>
        </div>

        {/* Gradient separator */}
        <div
          className="h-[2px] mx-2 mt-2 shrink-0"
          style={{
            background:
              "linear-gradient(90deg, transparent 0%, var(--brand) 30%, rgba(167, 139, 250, 0.6) 50%, var(--brand) 70%, transparent 100%)",
          }}
        />

        {/* Content: body */}
        <div className="flex-1 overflow-y-auto min-h-0 relative">
          <EditorContentWrapper editor={editor} className={editorContentClass} />
          {/* Vim command line at bottom of editor area */}
          {vimEnabled && commandLine && (
            <div className="absolute bottom-0 left-0 right-0">
              <VimCommandLine
                prefix={commandLine.prefix}
                onSubmit={handleCommandLineSubmit}
                onCancel={handleCommandLineCancel}
              />
            </div>
          )}
        </div>
        {editor && <SlashMenu editor={editor} />}
        {editor && <WikiLinkMenu editor={editor} />}
        {editor && <EntityMentionMenu editor={editor} />}
      </div>

      {showHistory && <NoteVersionHistory noteId={note.id} onRestore={handleRestore} />}
    </div>
  );
}
