import { ipc } from "@shared/hooks/useIpc";
import type { Note, NoteUpdateParams } from "@shared/types";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { useNavigate } from "react-router";
import { type AnnotationResponse, useAnnotations } from "../hooks/useAnnotations";
import { useAskAI } from "../hooks/useAskAI";
import { useEditorActions } from "../hooks/useEditorActions";
import { useLanguageConfig } from "../hooks/useLanguageConfig";
import { useQuickTranslate } from "../hooks/useQuickTranslate";
import { useVocabularySave } from "../hooks/useVocabularySave";
import { AnnotationPane } from "./AnnotationPane";
import { AnnotationPopover } from "./AnnotationPopover";
import { AskAIPopup } from "./editor/AskAIPopup";
import { EditorContextMenu } from "./editor/EditorContextMenu";
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
import { PracticeMode } from "./practice/PracticeMode";
import { QuickTranslatePopup } from "./QuickTranslatePopup";

const VERSION_INTERVAL_MS = 5 * 60 * 1000; // Minimum interval between auto-saving version snapshots

function parseSideNotes(splitContent: string | null | undefined): string {
  if (!splitContent) return "";
  try {
    const data = JSON.parse(splitContent);
    return data.sideNotes ?? "";
  } catch {
    return "";
  }
}

type NotesViewMode = "editor" | "graph";

interface NoteEditorProps {
  note: Note;
  onSave: (params: NoteUpdateParams) => void;
  viewMode: NotesViewMode;
  onViewModeChange: (mode: NotesViewMode) => void;
  onToggleFocusMode?: () => void;
  focusModeActive?: boolean;
  onGenerateCards?: (selectedText?: string) => void;
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
  editorFocusRef,
}: NoteEditorProps) {
  const [practiceActive, setPracticeActive] = useState(false);
  const [sidePaneOpen, setSidePaneOpen] = useState(false);
  const [paneSplit, setPaneSplit] = useState(0.6); // editor takes 60%
  const resizingRef = useRef(false);
  const splitContainerRef = useRef<HTMLDivElement>(null);
  const pointerCleanupRef = useRef<(() => void) | null>(null);
  const navigate = useNavigate();
  const saveTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const lastNoteIdRef = useRef(note.id);
  const pendingRef = useRef<{ html: string; markdown: string } | null>(null);
  const onSaveRef = useRef(onSave);
  onSaveRef.current = onSave;
  const noteContent = note.bodyHtml || note.body || "";
  const noteContentRef = useRef(noteContent);
  noteContentRef.current = noteContent;
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
  const askAI = useAskAI(note.id);
  const { handleAnnotate, handleFlashcard, handleAskAI } = useEditorActions(
    editor,
    note.id,
    createAnnotation,
    onGenerateCards,
    askAI.trigger,
  );

  // ── Quick-translate popup ──────────────────────────────────────────
  const { sourceLang, targetLang } = useLanguageConfig(
    note.perspectiveConfig,
    note.body ?? undefined,
  );
  const quickTranslate = useQuickTranslate(sourceLang, targetLang);
  const _vocabSave = useVocabularySave();

  // ── Annotation side pane ─────────────────────────────────────────
  const sideNotesContent = useMemo(() => parseSideNotes(note.splitContent), [note.splitContent]);

  const handleSideNotesChange = useCallback(
    (html: string, markdown: string) => {
      onSave({
        id: note.id,
        splitContent: JSON.stringify({ sideNotes: html, sideNotesMarkdown: markdown }),
      });
    },
    [note.id, onSave],
  );

  // Clean up pointer listeners on unmount (in case drag is in progress)
  useEffect(() => {
    return () => pointerCleanupRef.current?.();
  }, []);

  // Right-click "Translate" → show Quick Translate popup near the selected text
  const handleTranslate = useCallback(
    (selectedText: string, rect?: { top: number; left: number }) => {
      quickTranslate.triggerTranslateText(selectedText, rect);
    },
    [quickTranslate.triggerTranslateText],
  );

  // Annotation popover state
  const [activePopover, setActivePopover] = useState<{
    annotation: AnnotationResponse;
    position: { top: number; left: number };
  } | null>(null);

  // Use a ref so the click handler always sees the latest annotations
  const annotationsRef = useRef(annotations);
  annotationsRef.current = annotations;

  // Click outside annotation → close popover
  useEffect(() => {
    if (!editor) return;
    const handleClick = (event: MouseEvent) => {
      const target = event.target as HTMLElement;
      if (!target.closest(".annotation-highlight")) {
        setActivePopover(null);
      }
    };
    const editorEl = editor.view.dom;
    editorEl.addEventListener("click", handleClick);
    return () => editorEl.removeEventListener("click", handleClick);
  }, [editor]);

  // Hover on annotation → show lightweight tooltip with annotation content
  const [annotationTooltip, setAnnotationTooltip] = useState<{
    html: string;
    position: { top: number; left: number };
  } | null>(null);

  useEffect(() => {
    if (!editor) return;
    let hideTimer: ReturnType<typeof setTimeout> | null = null;

    const handleMouseOver = (event: MouseEvent) => {
      const target = event.target as HTMLElement;
      const highlight = target.closest(".annotation-highlight") as HTMLElement | null;
      if (!highlight) return;
      if (hideTimer) clearTimeout(hideTimer);

      const annId = highlight.getAttribute("data-annotation-id");
      if (!annId) return;
      const ann = annotationsRef.current.find((a) => a.markId === annId || a.id === annId);
      const html = ann?.content || ann?.quotedText || "";
      if (!html.replace(/<[^>]*>/g, "").trim()) return;

      const rect = highlight.getBoundingClientRect();
      setAnnotationTooltip({
        html,
        position: { top: rect.top - 4, left: rect.left + rect.width / 2 },
      });
    };

    const handleMouseOut = (event: MouseEvent) => {
      const target = event.target as HTMLElement;
      if (target.closest(".annotation-highlight")) {
        hideTimer = setTimeout(() => setAnnotationTooltip(null), 150);
      }
    };

    const editorEl = editor.view.dom;
    editorEl.addEventListener("mouseover", handleMouseOver);
    editorEl.addEventListener("mouseout", handleMouseOut);
    return () => {
      editorEl.removeEventListener("mouseover", handleMouseOver);
      editorEl.removeEventListener("mouseout", handleMouseOut);
      if (hideTimer) clearTimeout(hideTimer);
    };
  }, [editor]);

  // Remove annotation callback for context menu
  const handleRemoveAnnotation = useCallback(
    (annId: string) => {
      deleteAnnotation(annId);
    },
    [deleteAnnotation],
  );

  // Listen for editor-action events (keyboard shortcuts from AnnotationMark + vim leader keys)
  useEffect(() => {
    const handler = (e: Event) => {
      const { action } = (e as CustomEvent<{ action: string }>).detail;
      if (action === "annotate") {
        handleAnnotate();
      } else if (action === "flashcard") {
        handleFlashcard();
      } else if (action === "translate") {
        const sel = window.getSelection();
        const text = sel?.toString().trim() ?? "";
        if (!text) return;
        let rect: { top: number; left: number } | undefined;
        if (sel && sel.rangeCount > 0) {
          const r = sel.getRangeAt(0).getBoundingClientRect();
          rect = { top: r.bottom + 8, left: r.left };
        }
        handleTranslate(text, rect);
      } else if (action === "ask-ai") {
        const sel = window.getSelection();
        const text = sel?.toString().trim() ?? "";
        if (!text) return;
        let rect: { top: number; left: number } | undefined;
        if (sel && sel.rangeCount > 0) {
          const r = sel.getRangeAt(0).getBoundingClientRect();
          rect = { top: r.bottom + 8, left: r.left };
        }
        handleAskAI(text, rect);
      }
    };
    window.addEventListener("editor-action", handler);
    return () => window.removeEventListener("editor-action", handler);
  }, [handleAnnotate, handleFlashcard, handleTranslate, handleAskAI]);

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

  // Cmd+S → force save (skip when practice mode is active)
  useEffect(() => {
    if (practiceActive) return;
    const handler = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === "s") {
        e.preventDefault();
        flushSave();
      }
    };
    document.addEventListener("keydown", handler);
    return () => document.removeEventListener("keydown", handler);
  }, [flushSave, practiceActive]);

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

  // Cmd+Shift+P / Cmd+Shift+A — dispatched from KnowledgeBasePage keyboard handler
  useEffect(() => {
    const handler = (e: Event) => {
      const action = (e as CustomEvent<string>).detail;
      if (action === "toggle-practice") setPracticeActive((prev) => !prev);
      else if (action === "toggle-annotations") setSidePaneOpen((prev) => !prev);
    };
    window.addEventListener("editor-mode-toggle", handler);
    return () => window.removeEventListener("editor-mode-toggle", handler);
  }, []);

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

  const handleAnnotationClick = useCallback(
    (markId: string) => {
      if (!editor) return;
      const el = editor.view.dom.querySelector(
        `.annotation-highlight[data-annotation-id="${markId}"]`,
      );
      if (el) el.scrollIntoView({ behavior: "smooth", block: "center" });
    },
    [editor],
  );

  // Build editor content class — add vim mode class when vim is enabled
  const editorContentClass = vimEnabled
    ? `flex-1 min-h-0 overflow-y-auto vim-${vimMode}`
    : "flex-1 min-h-0 overflow-y-auto";

  return (
    <div className="flex-1 flex gap-2 min-w-0 min-h-0">
      <div className="flex-1 flex flex-col min-w-0 min-h-0">
        {/* Practice mode — full takeover */}
        {practiceActive ? (
          <PracticeMode
            noteId={note.id}
            sourceText={note.body || ""}
            sourceLang={sourceLang}
            targetLang={targetLang}
            onExit={() => setPracticeActive(false)}
          />
        ) : (
          <>
            {/* Content: body + optional side pane */}
            <div ref={splitContainerRef} className="flex-1 flex min-h-0">
              {/* Editor */}
              <EditorContextMenu
                onAnnotate={handleAnnotate}
                onFlashcard={handleFlashcard}
                onTranslate={handleTranslate}
                onAskAI={handleAskAI}
                onRemoveAnnotation={handleRemoveAnnotation}
              >
                <div
                  className="flex-1 overflow-y-auto min-h-0 min-w-0 relative"
                  style={sidePaneOpen ? { flex: `0 0 ${paneSplit * 100}%` } : undefined}
                >
                  <EditorContentWrapper editor={editor} className={editorContentClass} />
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
              </EditorContextMenu>

              {/* Resize handle + annotation pane */}
              {sidePaneOpen && (
                <>
                  {/* biome-ignore lint/a11y/noStaticElementInteractions: resize handle */}
                  <div
                    className="w-1.5 shrink-0 cursor-col-resize group flex items-center justify-center hover:bg-purple/10 transition-colors"
                    onPointerDown={(e) => {
                      e.preventDefault();
                      resizingRef.current = true;
                      const container = splitContainerRef.current;
                      if (!container) return;
                      const rect = container.getBoundingClientRect();
                      const onMove = (ev: PointerEvent) => {
                        if (!resizingRef.current) return;
                        const ratio = (ev.clientX - rect.left) / rect.width;
                        setPaneSplit(Math.max(0.3, Math.min(0.8, ratio)));
                      };
                      const onUp = () => {
                        resizingRef.current = false;
                        pointerCleanupRef.current = null;
                        document.removeEventListener("pointermove", onMove);
                        document.removeEventListener("pointerup", onUp);
                      };
                      document.addEventListener("pointermove", onMove);
                      document.addEventListener("pointerup", onUp);
                      pointerCleanupRef.current = onUp;
                    }}
                    onKeyDown={() => {}}
                  >
                    <div className="w-px h-full group-hover:bg-purple/40 transition-colors" />
                  </div>

                  <div className="flex-1 min-w-0 min-h-0 overflow-hidden border-l border-border">
                    <AnnotationPane
                      initialSideNotes={sideNotesContent}
                      annotations={annotations}
                      updateAnnotation={updateAnnotation}
                      deleteAnnotation={deleteAnnotation}
                      onClose={() => setSidePaneOpen(false)}
                      onSideNotesChange={handleSideNotesChange}
                      onAnnotationClick={handleAnnotationClick}
                      sourceLang={sourceLang}
                      targetLang={targetLang}
                    />
                  </div>
                </>
              )}
            </div>
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
                onTogglePracticeMode={() => setPracticeActive((prev) => !prev)}
                practiceActive={practiceActive}
                onToggleAnnotationPane={() => setSidePaneOpen((prev) => !prev)}
                annotationPaneActive={sidePaneOpen}
                onToggleFocusMode={onToggleFocusMode}
                onToggleGraphMode={() =>
                  onViewModeChange(viewMode === "graph" ? "editor" : "graph")
                }
                onToggleVersionHistory={() => setShowHistory(!showHistory)}
                focusModeActive={focusModeActive}
                graphModeActive={viewMode === "graph"}
                versionHistoryActive={showHistory}
              />
            </div>
          </>
        )}
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
          position={quickTranslate.position}
          loading={quickTranslate.loading}
          onDismiss={quickTranslate.dismiss}
        />
      )}

      {/* Ask AI popup */}
      {askAI.selectedText && askAI.position && (
        <AskAIPopup
          selectedText={askAI.selectedText}
          position={askAI.position}
          response={askAI.response}
          loading={askAI.loading}
          onSubmit={askAI.submit}
          onDismiss={askAI.dismiss}
        />
      )}

      {/* Annotation hover tooltip */}
      {annotationTooltip &&
        createPortal(
          <div
            className="fixed z-[60] max-w-[280px] rounded-lg pointer-events-none bg-popover text-popover-foreground border border-border shadow-md editor-content-compact"
            style={{
              top: annotationTooltip.position.top,
              left: annotationTooltip.position.left,
              transform: "translate(-50%, -100%)",
              padding: "0.5rem 0.75rem",
            }}
            // biome-ignore lint/security/noDangerouslySetInnerHtml: annotation content
            dangerouslySetInnerHTML={{ __html: annotationTooltip.html }}
          />,
          document.body,
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
