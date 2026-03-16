import { ipc } from "@shared/hooks/useIpc";
import type { Note, NoteUpdateParams } from "@shared/types";
import { useCallback, useEffect, useRef, useState } from "react";
import { useNavigate } from "react-router";
import { EditorContentWrapper, useEntityResolution, useNoteEditor } from "./editor/EditorCore";
import { EditorToolbar } from "./editor/EditorToolbar";
import { EntityMentionMenu } from "./editor/EntityMention";
import { SlashMenu } from "./editor/SlashCommandMenu";
import { VimCommandLine } from "./editor/VimCommandLine";
import type { VimMode } from "./editor/vim";
import { getVimPlugin, VIM_SAVE_EVENT } from "./editor/vim";
import { WikiLinkMenu } from "./editor/WikiLinkNode";
import { LinkInsertDialog } from "./LinkInsertDialog";
import { NoteVersionHistory } from "./NoteVersionHistory";

const VERSION_INTERVAL_MS = 5 * 60 * 1000; // Minimum interval between auto-saving version snapshots

type NotesViewMode = "editor" | "graph";

interface NoteEditorProps {
  note: Note;
  onSave: (params: NoteUpdateParams) => void;
  viewMode: NotesViewMode;
  onViewModeChange: (mode: NotesViewMode) => void;
  onToggleFocusMode?: () => void;
  focusModeActive?: boolean;
}

export function NoteEditor({
  note,
  onSave,
  viewMode,
  onViewModeChange,
  onToggleFocusMode,
  focusModeActive,
}: NoteEditorProps) {
  const navigate = useNavigate();
  const saveTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const lastNoteIdRef = useRef(note.id);
  const pendingRef = useRef<{ html: string; markdown: string } | null>(null);
  const onSaveRef = useRef(onSave);
  onSaveRef.current = onSave;
  const noteContentRef = useRef(note.bodyHtml || note.body || "");
  noteContentRef.current = note.bodyHtml || note.body || "";
  const lastVersionTimeRef = useRef(0);
  const [showHistory, setShowHistory] = useState(false);

  // Link/image insert dialog state
  const [linkDialog, setLinkDialog] = useState<{ type: "link" | "image"; isOpen: boolean }>({
    type: "link",
    isOpen: false,
  });

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

  // Listen for insert-wiki-link events from AI suggestions panel
  useEffect(() => {
    if (!editor) return;
    const handler = (e: Event) => {
      const { title, noteId: targetId } = (e as CustomEvent<{ title: string; noteId: string }>)
        .detail;
      if (!title) return;
      editor
        .chain()
        .focus()
        .insertContent({
          type: "text",
          text: title,
          marks: [{ type: "wikiLink", attrs: { noteId: targetId, title } }],
        })
        .run();
    };
    window.addEventListener("insert-wiki-link", handler);
    return () => window.removeEventListener("insert-wiki-link", handler);
  }, [editor]);

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
        <div className="px-2 pb-0 shrink-0">
          <EditorToolbar
            editor={editor}
            vimEnabled={vimEnabled}
            vimMode={vimMode}
            onToggleVim={toggleVim}
            onOpenLinkDialog={() => setLinkDialog({ type: "link", isOpen: true })}
            onOpenImageDialog={() => setLinkDialog({ type: "image", isOpen: true })}
            onToggleFocusMode={onToggleFocusMode}
            onToggleGraphMode={() => onViewModeChange(viewMode === "graph" ? "editor" : "graph")}
            onToggleVersionHistory={() => setShowHistory(!showHistory)}
            focusModeActive={focusModeActive}
            graphModeActive={viewMode === "graph"}
            versionHistoryActive={showHistory}
          />
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
        {editor && <WikiLinkMenu editor={editor} currentNoteTitle={note.title} />}
        {editor && <EntityMentionMenu editor={editor} />}
      </div>

      {showHistory && <NoteVersionHistory noteId={note.id} onRestore={handleRestore} />}

      {/* Link/Image insert dialog */}
      <LinkInsertDialog
        type={linkDialog.type}
        isOpen={linkDialog.isOpen}
        onClose={() => setLinkDialog((prev) => ({ ...prev, isOpen: false }))}
        onInsert={(url) => {
          if (!editor) return;
          if (linkDialog.type === "link") {
            editor.chain().focus().setLink({ href: url }).run();
          } else {
            editor.chain().focus().setImage({ src: url }).run();
          }
        }}
      />
    </div>
  );
}
