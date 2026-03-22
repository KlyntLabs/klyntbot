import { ipc } from "@shared/hooks/useIpc";
import type { Note, NoteUpdateParams } from "@shared/types";
import { useCallback, useEffect, useRef, useState } from "react";
import { useNavigate } from "react-router";
import { type AnnotationResponse, useAnnotations } from "../hooks/useAnnotations";
import { useEditorActions } from "../hooks/useEditorActions";
import { useLanguageConfig } from "../hooks/useLanguageConfig";
import { usePerspective } from "../hooks/usePerspective";
import { useQuickTranslate } from "../hooks/useQuickTranslate";
import { useVocabularySave } from "../hooks/useVocabularySave";
import { AnnotationPopover } from "./AnnotationPopover";
import { EditorContextMenu } from "./editor/EditorContextMenu";
import { EditorContentWrapper, useEntityResolution, useNoteEditor } from "./editor/EditorCore";
import { EditorToolbar } from "./editor/EditorToolbar";
import { EntityMentionMenu } from "./editor/EntityMention";
import { SlashMenu } from "./editor/SlashCommandMenu";
import { SplitEditor, type SplitMode } from "./editor/SplitEditor";
import { SplitToolbar } from "./editor/SplitToolbar";
import { VimCommandLine } from "./editor/VimCommandLine";
import type { VimMode } from "./editor/vim";
import { getVimPlugin, VIM_SAVE_EVENT } from "./editor/vim";
import { WikiLinkMenu } from "./editor/WikiLinkNode";
import { LinkInsertDialog } from "./LinkInsertDialog";
import { NoteVersionHistory } from "./NoteVersionHistory";
import { QuickTranslatePopup } from "./QuickTranslatePopup";

const VERSION_INTERVAL_MS = 5 * 60 * 1000; // Minimum interval between auto-saving version snapshots

type NotesViewMode = "editor" | "graph";

interface NoteEditorProps {
  note: Note;
  onSave: (params: NoteUpdateParams) => void;
  viewMode: NotesViewMode;
  onViewModeChange: (mode: NotesViewMode) => void;
  onToggleFocusMode?: () => void;
  focusModeActive?: boolean;
  onGenerateCards?: (selectedText?: string) => void;
  splitMode?: SplitMode | null;
  onSplitModeChange?: (mode: SplitMode | null) => void;
  editorFocusRef?: React.MutableRefObject<(() => void) | undefined>;
}

export function NoteEditor({
  note,
  onSave,
  viewMode,
  onViewModeChange,
  onToggleFocusMode,
  focusModeActive,
  onGenerateCards,
  splitMode,
  onSplitModeChange,
  editorFocusRef,
}: NoteEditorProps) {
  const activeSplitMode = splitMode ?? (note.splitMode as SplitMode | null);
  const navigate = useNavigate();
  const saveTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const lastNoteIdRef = useRef(note.id);
  const pendingRef = useRef<{ html: string; markdown: string } | null>(null);
  const onSaveRef = useRef(onSave);
  onSaveRef.current = onSave;
  // When splitContent exists, show only the left pane in single mode
  // (body contains concatenated left+right for indexing — confusing to display)
  const singlePaneContent = (() => {
    if (note.splitContent) {
      try {
        const split = JSON.parse(note.splitContent);
        return split.left?.html || split.left?.markdown || note.body || "";
      } catch {
        /* fall through */
      }
    }
    return note.bodyHtml || note.body || "";
  })();
  const noteContentRef = useRef(singlePaneContent);
  noteContentRef.current = singlePaneContent;
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

  // Expose focus function to parent (for title → editor transition)
  useEffect(() => {
    if (editorFocusRef && editor) {
      editorFocusRef.current = () => editor.commands.focus();
    }
  }, [editor, editorFocusRef]);

  // ── Annotation & Perspective hooks ────────────────────────────────
  const { annotations, createAnnotation, updateAnnotation, deleteAnnotation } = useAnnotations(
    note.id,
    editor,
  );
  const { handleAnnotate, handleFlashcard, handleAskAI } = useEditorActions(
    editor,
    note.id,
    createAnnotation,
  );

  const { activePerspective, focusedSectionId, setPerspective, setLanguagePair, languagePair } =
    usePerspective(
      note.id,
      editor,
      (note as Record<string, unknown>).perspectiveConfig as string | null | undefined,
    );

  const [translateSelection, setTranslateSelection] = useState<string | undefined>();

  // ── Quick-translate popup ──────────────────────────────────────────
  const { sourceLang, targetLang } = useLanguageConfig(
    (note as Record<string, unknown>).perspectiveConfig as string | null | undefined,
    note.body ?? undefined,
  );
  const quickTranslate = useQuickTranslate(sourceLang, targetLang);
  const vocabSave = useVocabularySave();

  // Right-click "Translate" → show Quick Translate popup near the selected text
  const handleTranslate = useCallback(
    (selectedText: string, rect?: { top: number; left: number }) => {
      quickTranslate.triggerTranslateText(selectedText, rect);
    },
    [quickTranslate.triggerTranslateText],
  );

  const handleTranslateTo = useCallback(
    (targetLang: string, selectedText?: string, rect?: { top: number; left: number }) => {
      setLanguagePair({ targetLang });
      if (selectedText) {
        quickTranslate.triggerTranslateText(selectedText, rect);
      }
    },
    [setLanguagePair, quickTranslate.triggerTranslateText],
  );

  // Annotation popover state
  const [activePopover, setActivePopover] = useState<{
    annotation: AnnotationResponse;
    position: { top: number; left: number };
  } | null>(null);

  // Use a ref so the click handler always sees the latest annotations
  const annotationsRef = useRef(annotations);
  annotationsRef.current = annotations;

  // Click handler for annotation marks
  useEffect(() => {
    if (!editor) return;
    const handleClick = (event: MouseEvent) => {
      const target = event.target as HTMLElement;
      const highlight = target.closest(".annotation-highlight") as HTMLElement | null;
      if (!highlight) {
        setActivePopover(null);
        return;
      }
      const annotationId = highlight.getAttribute("data-annotation-id");
      if (!annotationId) return;

      const ann = annotationsRef.current.find(
        (a) => a.markId === annotationId || a.id === annotationId,
      );
      if (!ann) return;

      const rect = highlight.getBoundingClientRect();
      setActivePopover({
        annotation: ann,
        position: { top: rect.bottom, left: rect.left },
      });
    };

    const editorEl = editor.view.dom;
    editorEl.addEventListener("click", handleClick);
    return () => editorEl.removeEventListener("click", handleClick);
  }, [editor]);

  // Listen for editor-action events (keyboard shortcuts from AnnotationMark)
  useEffect(() => {
    const handler = (e: Event) => {
      const { action } = (e as CustomEvent<{ action: string }>).detail;
      if (action === "annotate") handleAnnotate();
      else if (action === "flashcard") handleFlashcard();
      else if (action === "linked-view" && focusedSectionId) {
        setPerspective(focusedSectionId, "linked-view");
      }
    };
    window.addEventListener("editor-action", handler);
    return () => window.removeEventListener("editor-action", handler);
  }, [handleAnnotate, handleFlashcard, focusedSectionId, setPerspective]);

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

  // Cmd+S → force save (skip when split mode is active — SplitEditor handles its own)
  useEffect(() => {
    if (activeSplitMode) return;
    const handler = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === "s") {
        e.preventDefault();
        flushSave();
      }
    };
    document.addEventListener("keydown", handler);
    return () => document.removeEventListener("keydown", handler);
  }, [flushSave, activeSplitMode]);

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

  // Cmd+Option+P → enter practice mode
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.altKey && e.key === "p") {
        e.preventDefault();
        onSplitModeChange?.("practice");
      }
    };
    document.addEventListener("keydown", handler);
    return () => document.removeEventListener("keydown", handler);
  }, [onSplitModeChange]);

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

  const handleToggleSplitMode = useCallback(() => {
    if (activeSplitMode) {
      onSplitModeChange?.(null);
    } else {
      onSplitModeChange?.("translation");
    }
  }, [activeSplitMode, onSplitModeChange]);

  // Restore main editor content when leaving split mode — show left pane only
  const prevSplitModeRef = useRef(activeSplitMode);
  const restoreContentRef = useRef(singlePaneContent);
  restoreContentRef.current = singlePaneContent;
  useEffect(() => {
    if (prevSplitModeRef.current && !activeSplitMode && editor) {
      editor.commands.setContent(restoreContentRef.current);
    }
    prevSplitModeRef.current = activeSplitMode;
  }, [activeSplitMode, editor]);

  return (
    <div className="flex-1 flex gap-2 min-w-0 min-h-0">
      <div className="flex-1 flex flex-col min-w-0 min-h-0">
        {/* Content: body */}
        <EditorContextMenu
          onAnnotate={handleAnnotate}
          onFlashcard={handleFlashcard}
          onTranslate={handleTranslate}
          onTranslateTo={handleTranslateTo}
          onAskAI={handleAskAI}
          onLinkedView={() => {
            if (focusedSectionId) setPerspective(focusedSectionId, "linked-view");
          }}
          onApplyPerspective={(type) => {
            if (focusedSectionId)
              setPerspective(focusedSectionId, type as "linked-view" | "annotated" | "study-mode");
          }}
          noteTargetLang={languagePair?.targetLang}
        >
          {activeSplitMode ? (
            <div className="flex-1 flex flex-col min-h-0">
              <SplitToolbar
                currentMode={activeSplitMode}
                onModeChange={(mode) =>
                  onSplitModeChange?.(mode === "single" ? null : (mode as SplitMode))
                }
              />
              <SplitEditor
                note={note}
                splitMode={activeSplitMode}
                onSave={onSave}
                onModeChange={(mode) =>
                  onSplitModeChange?.(mode === "single" ? null : (mode as SplitMode))
                }
                targetLangOverride={languagePair?.targetLang}
                sourceTextOverride={translateSelection}
                onClearSourceOverride={() => setTranslateSelection(undefined)}
              />
            </div>
          ) : (
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
          )}
        </EditorContextMenu>
        {editor && <SlashMenu editor={editor} />}
        {editor && <WikiLinkMenu editor={editor} currentNoteTitle={note.title} />}
        {editor && <EntityMentionMenu editor={editor} />}

        {/* Gradient separator */}
        <div
          className="h-[2px] mx-2 mb-2 shrink-0"
          style={{
            background:
              "linear-gradient(90deg, transparent 0%, var(--brand) 30%, rgba(167, 139, 250, 0.6) 50%, var(--brand) 70%, transparent 100%)",
          }}
        />

        {/* Controls bar */}
        <div className="px-2 pt-0 shrink-0">
          <EditorToolbar
            editor={editor}
            vimEnabled={vimEnabled}
            vimMode={vimMode}
            onToggleVim={toggleVim}
            onOpenLinkDialog={() => setLinkDialog({ type: "link", isOpen: true })}
            onOpenImageDialog={() => setLinkDialog({ type: "image", isOpen: true })}
            onGenerateCards={onGenerateCards}
            onToggleSplitMode={onSplitModeChange ? handleToggleSplitMode : undefined}
            splitModeActive={!!activeSplitMode}
            onToggleFocusMode={onToggleFocusMode}
            onToggleGraphMode={() => onViewModeChange(viewMode === "graph" ? "editor" : "graph")}
            onToggleVersionHistory={() => setShowHistory(!showHistory)}
            focusModeActive={focusModeActive}
            graphModeActive={viewMode === "graph"}
            versionHistoryActive={showHistory}
          />
        </div>
      </div>

      {showHistory && <NoteVersionHistory noteId={note.id} onRestore={handleRestore} />}

      {/* Annotation popover */}
      {activePopover && (
        <AnnotationPopover
          annotation={activePopover.annotation}
          position={activePopover.position}
          onClose={() => setActivePopover(null)}
          onEdit={(id, content) => {
            updateAnnotation({ id, content });
            setActivePopover(null);
          }}
          onDelete={(id) => {
            deleteAnnotation(id);
            setActivePopover(null);
          }}
          onCreateFlashcard={(quotedText) => {
            onGenerateCards?.(quotedText);
            setActivePopover(null);
          }}
        />
      )}

      {/* Quick-translate popup */}
      {quickTranslate.selection && quickTranslate.position && (
        <QuickTranslatePopup
          translation={quickTranslate.translation}
          words={quickTranslate.words}
          position={quickTranslate.position}
          loading={quickTranslate.loading}
          onSaveWords={() => {
            vocabSave.saveWords(quickTranslate.words, note.id, "quick-translate");
            quickTranslate.dismiss();
          }}
          onPractice={() => {
            onSplitModeChange?.("practice");
            quickTranslate.dismiss();
          }}
          onDismiss={quickTranslate.dismiss}
        />
      )}

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
