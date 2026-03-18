import { useEvent } from "@shared/hooks/useEvent";
import { useMutation } from "@shared/hooks/useMutation";
import { invalidateQueries, useQuery } from "@shared/hooks/useQuery";
import type {
  Note,
  Notebook,
  NotebookCreateParams,
  NoteCreateParams,
  NoteUpdateParams,
} from "@shared/types";
import { FileText, GitGraph, PenLine } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useSearchParams } from "react-router";
import { CardGenerationModal } from "../components/CardGenerationModal";
import { ContextPanel } from "../components/ContextPanel";
import { GraphView } from "../components/GraphView";
import { NavigationSidebar } from "../components/NavigationSidebar";
import { NoteCreationDialog } from "../components/NoteCreationDialog";
import { NoteEditorPanel } from "../components/NoteEditorPanel";
import { NoteFinder } from "../components/NoteFinder";
import { VersionHistoryOverlay } from "../components/VersionHistoryOverlay";
import { useCardGeneration } from "../hooks/useCardGeneration";
import { useInbox } from "../hooks/useInbox";
import { useInsightReview } from "../hooks/useInsightReview";

type ViewMode = "editor" | "graph";
type LayoutMode = "three-panel" | "focus";

function ViewModeToggle({
  viewMode,
  onChange,
}: {
  viewMode: ViewMode;
  onChange: (mode: ViewMode) => void;
}) {
  return (
    <div className="flex items-center gap-1">
      <button
        type="button"
        aria-label="Editor view"
        onClick={() => onChange("editor")}
        className={`p-1.5 rounded-md transition-colors ${viewMode === "editor" ? "bg-muted text-foreground" : "text-muted-foreground hover:text-foreground"}`}
      >
        <PenLine size={16} />
      </button>
      <button
        type="button"
        aria-label="Graph view"
        onClick={() => onChange("graph")}
        className={`p-1.5 rounded-md transition-colors ${viewMode === "graph" ? "bg-muted text-foreground" : "text-muted-foreground hover:text-foreground"}`}
      >
        <GitGraph size={16} />
      </button>
    </div>
  );
}

export default function KnowledgeBasePage() {
  // ── Data fetching ─────────────────────────────────────────────────────
  const { data: notebooks, refetch: refetchNotebooks } = useQuery<Notebook[]>(
    "notebook_list",
    undefined,
    [],
  );
  const { data: notes, refetch: refetchNotes } = useQuery<Note[]>("note_list", undefined, []);
  const { items: inboxItems, deleteItem: deleteInboxItem } = useInbox();

  // ── Core state ────────────────────────────────────────────────────────
  const [selectedNoteId, setSelectedNoteId] = useState<string | null>(null);
  const [viewMode, setViewMode] = useState<ViewMode>("editor");
  const [layoutMode, setLayoutMode] = useState<LayoutMode>("three-panel");
  const [showCreateDialog, setShowCreateDialog] = useState(false);
  const [showVersionHistory, setShowVersionHistory] = useState(false);
  const [showNoteFinder, setShowNoteFinder] = useState(false);
  const [searchParams, setSearchParams] = useSearchParams();

  // ── Card Generation ──────────────────────────────────────────────────
  const cardGen = useCardGeneration();
  const [cardGenOpen, setCardGenOpen] = useState(false);

  // ── Insight Review ────────────────────────────────────────────────────
  const [insightState, insightActions] = useInsightReview();
  const insightStateRef = useRef(insightState);
  insightStateRef.current = insightState;
  const insightActionsRef = useRef(insightActions);
  insightActionsRef.current = insightActions;

  // ── Sync Insight Review panel when switching notes ────────────────────
  useEffect(() => {
    if (insightState.isOpen && selectedNoteId && selectedNoteId !== insightState.noteId) {
      void insightActions.open(selectedNoteId);
    }
  }, [selectedNoteId]); // eslint-disable-line react-hooks/exhaustive-deps

  // ── Sidebar widths (imperatively managed for perf) ────────────────────
  const [leftWidth, setLeftWidth] = useState(220);
  const [rightWidth, setRightWidth] = useState(260);
  const leftRef = useRef<HTMLDivElement>(null);
  const rightRef = useRef<HTMLDivElement>(null);
  const leftWidthRef = useRef(leftWidth);
  const rightWidthRef = useRef(rightWidth);
  const containerRef = useRef<HTMLDivElement>(null);

  // Pre-select note from URL search params (e.g. /notes?noteId=xxx)
  useEffect(() => {
    const noteId = searchParams.get("noteId");
    if (noteId) {
      setSelectedNoteId(noteId);
      setSearchParams({}, { replace: true });
    }
  }, [searchParams, setSearchParams]);

  const noteMap = useMemo(() => {
    const map = new Map<string, Note>();
    for (const n of notes) map.set(n.id, n);
    return map;
  }, [notes]);

  const selectedNote = selectedNoteId ? noteMap.get(selectedNoteId) : undefined;

  // ── Insight derived state ─────────────────────────────────────────────
  const insightOpen = insightState.isOpen;
  const insightPanelWidth = useMemo(() => {
    const container = containerRef.current;
    if (!container) return 480;
    // Leave at least 300px for the editor + left sidebar space
    const available = container.clientWidth - leftWidth - 20;
    return Math.max(360, Math.min(640, available * 0.65));
  }, [leftWidth, insightOpen]); // eslint-disable-line react-hooks/exhaustive-deps
  const effectiveRightWidth = insightOpen ? insightPanelWidth : rightWidth;

  // ── Mutations ─────────────────────────────────────────────────────────
  const { mutate: createNote } = useMutation<Note, NoteCreateParams>("note_create", "params");
  const { mutate: updateNote } = useMutation<Note, NoteUpdateParams>("note_update", "params");
  const { mutate: deleteNote } = useMutation<boolean, { id: string }>("note_delete");
  const { mutate: createNotebook } = useMutation<Notebook, NotebookCreateParams>(
    "notebook_create",
    "params",
  );
  const { mutate: deleteNotebook } = useMutation<boolean, { id: string }>("notebook_delete");
  const { mutate: updateNotebook } = useMutation<
    Notebook,
    { id: string; title?: string; parentId?: string | null }
  >("notebook_update", "params");

  // ── Handlers ──────────────────────────────────────────────────────────
  const [autoRenameId, setAutoRenameId] = useState<string | null>(null);

  const handleCreateNote = useCallback(
    async (notebookId?: string) => {
      const result = await createNote({
        title: "Untitled",
        notebookId,
      });
      if (result) {
        setSelectedNoteId(result.id);
        setAutoRenameId(result.id);
      }
    },
    [createNote],
  );

  const handleCreateNoteWithTitle = useCallback(
    async (title: string) => {
      const result = await createNote({ title });
      return result ? { id: result.id } : undefined;
    },
    [createNote],
  );

  const handleCreateNotebook = useCallback(
    async (parentId?: string) => {
      const result = await createNotebook({ title: "New Folder", parentId });
      if (result) setAutoRenameId(result.id);
    },
    [createNotebook],
  );

  const handlePin = useCallback(
    (id: string, pinned: boolean) => {
      updateNote({ id, pinned });
    },
    [updateNote],
  );

  const handleDelete = useCallback(
    async (id: string) => {
      const deleted = await deleteNote({ id });
      if (deleted && selectedNoteId === id) {
        setSelectedNoteId(null);
      }
    },
    [deleteNote, selectedNoteId],
  );

  const handleDeleteNotebook = useCallback(
    async (id: string) => {
      await deleteNotebook({ id });
    },
    [deleteNotebook],
  );

  const handleRenameNotebook = useCallback(
    async (id: string, title: string) => {
      await updateNotebook({ id, title });
    },
    [updateNotebook],
  );

  const handleUpdateNotebook = useCallback(
    async (id: string, updates: { icon?: string | null; color?: string | null }) => {
      await updateNotebook({ id, ...updates });
    },
    [updateNotebook],
  );

  const handleRenameNote = useCallback(
    async (id: string, title: string) => {
      await updateNote({ id, title });
    },
    [updateNote],
  );

  const handleMoveNote = useCallback(
    async (id: string, notebookId: string | null) => {
      await updateNote({ id, notebookId });
    },
    [updateNote],
  );

  const handleMoveNotebook = useCallback(
    async (id: string, parentId: string | null) => {
      await updateNotebook({ id, parentId });
    },
    [updateNotebook],
  );

  const handleUpdateNote = useCallback(
    async (id: string, updates: { icon?: string | null; color?: string | null }) => {
      await updateNote({ id, ...updates });
    },
    [updateNote],
  );

  const handleInboxCreateAsNote = useCallback(
    async (content: string) => {
      const result = await createNote({ title: content.slice(0, 60), body: content });
      if (result) setSelectedNoteId(result.id);
    },
    [createNote],
  );

  const handleInboxDiscard = useCallback(
    async (id: string) => {
      await deleteInboxItem({ id });
    },
    [deleteInboxItem],
  );

  const handleGenerateCards = useCallback(
    (selectedText?: string) => {
      if (!selectedNote) return;
      setCardGenOpen(true);
      if (selectedText) {
        cardGen.generateFromText(selectedText, selectedNote.title);
      } else {
        cardGen.generateFromNote(selectedNote.id);
      }
    },
    [selectedNote, cardGen.generateFromNote, cardGen.generateFromText],
  );

  // ── Resize logic (left sidebar) ───────────────────────────────────────
  const onLeftResizeStart = useCallback((e: React.PointerEvent) => {
    e.preventDefault();
    const startX = e.clientX;
    const startW = leftWidthRef.current;
    let raf = 0;

    containerRef.current?.classList.add("resizing");

    const onMove = (ev: globalThis.PointerEvent) => {
      cancelAnimationFrame(raf);
      raf = requestAnimationFrame(() => {
        const newW = Math.min(320, Math.max(180, startW + ev.clientX - startX));
        leftWidthRef.current = newW;
        if (leftRef.current) leftRef.current.style.width = `${newW}px`;
      });
    };
    const onUp = () => {
      cancelAnimationFrame(raf);
      setLeftWidth(leftWidthRef.current);
      containerRef.current?.classList.remove("resizing");
      document.removeEventListener("pointermove", onMove);
      document.removeEventListener("pointerup", onUp);
    };
    document.addEventListener("pointermove", onMove);
    document.addEventListener("pointerup", onUp);
  }, []);

  // ── Resize logic (right panel) ────────────────────────────────────────
  const onRightResizeStart = useCallback((e: React.PointerEvent) => {
    e.preventDefault();
    const startX = e.clientX;
    const startW = rightWidthRef.current;
    let raf = 0;

    containerRef.current?.classList.add("resizing");

    const onMove = (ev: globalThis.PointerEvent) => {
      cancelAnimationFrame(raf);
      raf = requestAnimationFrame(() => {
        // Right panel resize is inverted — dragging left increases width
        const newW = Math.min(360, Math.max(200, startW - (ev.clientX - startX)));
        rightWidthRef.current = newW;
        if (rightRef.current) rightRef.current.style.width = `${newW}px`;
      });
    };
    const onUp = () => {
      cancelAnimationFrame(raf);
      setRightWidth(rightWidthRef.current);
      containerRef.current?.classList.remove("resizing");
      document.removeEventListener("pointermove", onMove);
      document.removeEventListener("pointerup", onUp);
    };
    document.addEventListener("pointermove", onMove);
    document.addEventListener("pointerup", onUp);
  }, []);

  // ── Keyboard shortcuts ────────────────────────────────────────────────
  const handleCreateNoteRef = useRef(handleCreateNote);
  handleCreateNoteRef.current = handleCreateNote;
  const handleDeleteRef = useRef(handleDelete);
  handleDeleteRef.current = handleDelete;
  const selectedNoteIdRef = useRef(selectedNoteId);
  selectedNoteIdRef.current = selectedNoteId;

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      // Escape: close Insight Review (no mod key needed)
      if (e.key === "Escape" && insightStateRef.current.isOpen) {
        e.preventDefault();
        insightActionsRef.current.close();
        return;
      }

      const mod = e.metaKey || e.ctrlKey;
      if (!mod) return;

      if (e.key === "n" && !e.shiftKey) {
        e.preventDefault();
        setShowCreateDialog(true);
      } else if (e.key === "n" && e.shiftKey) {
        // Cmd+Shift+N: create blank note immediately
        e.preventDefault();
        handleCreateNoteRef.current();
      } else if ((e.key === "h" || e.key === "H") && e.shiftKey && selectedNoteIdRef.current) {
        // Cmd+Shift+H: toggle version history
        e.preventDefault();
        setShowVersionHistory((prev) => !prev);
      } else if (e.key === "Backspace" && selectedNoteIdRef.current) {
        e.preventDefault();
        handleDeleteRef.current(selectedNoteIdRef.current);
      } else if (e.key === "Enter" && e.shiftKey) {
        // Cmd+Shift+Enter → toggle layout mode
        e.preventDefault();
        setLayoutMode((prev) => (prev === "three-panel" ? "focus" : "three-panel"));
      } else if ((e.key === "g" || e.key === "G") && e.shiftKey) {
        // Cmd+Shift+G → toggle view mode
        e.preventDefault();
        setViewMode((prev) => (prev === "editor" ? "graph" : "editor"));
      } else if ((e.key === "i" || e.key === "I") && e.shiftKey) {
        // Cmd+Shift+I → toggle Insight Review
        e.preventDefault();
        if (insightStateRef.current.isOpen) {
          insightActionsRef.current.close();
        } else if (selectedNoteIdRef.current) {
          void insightActionsRef.current.open(selectedNoteIdRef.current);
        }
      } else if (e.key === "l" && !e.shiftKey) {
        // Cmd+L → insert top AI-suggested link at cursor
        e.preventDefault();
        window.dispatchEvent(new CustomEvent("trigger-insert-link"));
      } else if (e.key === "f" && !e.shiftKey) {
        // Cmd+F → open note finder
        e.preventDefault();
        setShowNoteFinder(true);
      }
    };
    document.addEventListener("keydown", handler);
    return () => document.removeEventListener("keydown", handler);
  }, []);

  // ── Event refresh ─────────────────────────────────────────────────────
  useEvent<{ entityKind: string }>("entity:updated", (payload) => {
    if (payload.entityKind === "note") {
      refetchNotes();
      invalidateQueries("note_backlinks");
      invalidateQueries("note_links_all");
      invalidateQueries("note_suggestions");
    }
    if (payload.entityKind === "notebook") {
      refetchNotebooks();
      refetchNotes();
    }
    if (payload.entityKind === "inbox") {
      invalidateQueries("inbox_list");
    }
  });

  // ── Derived layout flags ──────────────────────────────────────────────
  const isFocusMode = layoutMode === "focus";
  const isGraphMode = viewMode === "graph";
  const showLeftSidebar = !isFocusMode;
  const showRightPanel = !isFocusMode && !!selectedNoteId;

  // ── Render ────────────────────────────────────────────────────────────
  return (
    <div ref={containerRef} className="flex-1 flex min-w-0">
      {/* Left sidebar — NavigationSidebar */}
      {showLeftSidebar && (
        <>
          <div
            ref={leftRef}
            className="flex flex-col shrink-0 min-h-0"
            style={{ width: leftWidth }}
          >
            <NavigationSidebar
              notebooks={notebooks}
              notes={notes}
              selectedNoteId={selectedNoteId}
              autoRenameId={autoRenameId}
              onAutoRenameDone={() => setAutoRenameId(null)}
              onSelectNote={setSelectedNoteId}
              onCreateNote={handleCreateNote}
              onCreateNotebook={handleCreateNotebook}
              onDeleteNote={handleDelete}
              onPinNote={handlePin}
              onDeleteNotebook={handleDeleteNotebook}
              onRenameNotebook={handleRenameNotebook}
              onRenameNote={handleRenameNote}
              onMoveNote={handleMoveNote}
              onMoveNotebook={handleMoveNotebook}
              onUpdateNotebook={handleUpdateNotebook}
              onUpdateNote={handleUpdateNote}
              inboxItems={inboxItems}
              onInboxCreateAsNote={handleInboxCreateAsNote}
              onInboxDiscard={handleInboxDiscard}
            />
          </div>

          {/* Left resize handle */}
          <div
            onPointerDown={onLeftResizeStart}
            className="w-1 shrink-0 cursor-col-resize group flex items-center justify-center"
          >
            <div className="w-px h-full group-hover:bg-brand/40 transition-colors" />
          </div>
        </>
      )}

      {/* Center panel */}
      <div className={`flex-1 flex flex-col min-w-0 min-h-0 ${showLeftSidebar ? "pl-1" : ""}`}>
        {isGraphMode ? (
          <>
            <div className="flex justify-end shrink-0 px-3 pt-3">
              <ViewModeToggle viewMode={viewMode} onChange={setViewMode} />
            </div>
            <GraphView
              notes={notes}
              notebooks={notebooks}
              activeNoteId={selectedNoteId}
              onSelectNote={setSelectedNoteId}
              onOpenInEditor={(id) => {
                setSelectedNoteId(id);
                setViewMode("editor");
              }}
            />
          </>
        ) : selectedNote ? (
          <div className="w-full h-full flex flex-col">
            <NoteEditorPanel
              key={selectedNote.id}
              note={selectedNote}
              onSave={updateNote}
              onRenameNote={handleRenameNote}
              viewMode={viewMode}
              onViewModeChange={setViewMode}
              onToggleFocusMode={() =>
                setLayoutMode((prev) => (prev === "three-panel" ? "focus" : "three-panel"))
              }
              focusModeActive={isFocusMode}
              onGenerateCards={handleGenerateCards}
            />
          </div>
        ) : (
          <>
            <div className="flex justify-end shrink-0 px-3 pt-3">
              <ViewModeToggle viewMode={viewMode} onChange={setViewMode} />
            </div>
            <div className="flex-1 flex flex-col items-center justify-center gap-3">
              <div className="w-12 h-12 rounded-2xl bg-card flex items-center justify-center">
                <FileText className="w-6 h-6 text-dim" strokeWidth={1.5} />
              </div>
              <div className="text-center">
                <div className="text-muted-foreground text-sm">Select a note to view</div>
                <div className="text-dim text-xs mt-1">
                  or press{" "}
                  <kbd className="px-1.5 py-0.5 rounded bg-accent text-[10px] font-mono">Cmd+N</kbd>{" "}
                  to create one
                </div>
              </div>
            </div>
          </>
        )}
      </div>

      {/* Right resize handle — hidden when insight panel is open */}
      {showRightPanel && !insightOpen && (
        <div
          onPointerDown={onRightResizeStart}
          className="w-1 shrink-0 cursor-col-resize group flex items-center justify-center"
        >
          <div className="w-px h-full group-hover:bg-brand/40 transition-colors" />
        </div>
      )}

      {/* Right panel — ContextPanel */}
      {showRightPanel && (
        <div
          ref={rightRef}
          className="h-full transition-[width] duration-300 ease-in-out"
          style={{ width: effectiveRightWidth }}
        >
          <ContextPanel
            width={effectiveRightWidth}
            noteId={selectedNoteId}
            isGraphMode={isGraphMode}
            note={selectedNote ?? null}
            notes={notes}
            onSelectNote={setSelectedNoteId}
            onExpandGraph={() => setViewMode("graph")}
            insightOpen={insightOpen}
            insightState={insightState}
            insightActions={insightActions}
            onOpenInsight={() => {
              if (selectedNoteId) void insightActions.open(selectedNoteId);
            }}
          />
        </div>
      )}

      {/* Note Creation Dialog */}
      <NoteCreationDialog
        isOpen={showCreateDialog}
        onClose={() => setShowCreateDialog(false)}
        onCreate={handleCreateNoteWithTitle}
        onNavigateNote={setSelectedNoteId}
      />

      {/* Version History Overlay */}
      {showVersionHistory && selectedNote && (
        <VersionHistoryOverlay
          noteId={selectedNote.id}
          currentBody={selectedNote.body}
          onClose={() => setShowVersionHistory(false)}
          onRestore={() => {
            refetchNotes();
            setShowVersionHistory(false);
          }}
        />
      )}

      {/* Telescope-style fuzzy finder (Cmd+Shift+F) */}
      <NoteFinder
        isOpen={showNoteFinder}
        onClose={() => setShowNoteFinder(false)}
        onSelectNote={(id) => {
          setSelectedNoteId(id);
          setShowNoteFinder(false);
        }}
        notes={notes}
      />

      {/* Card Generation Modal */}
      <CardGenerationModal
        open={cardGenOpen}
        generating={cardGen.generating}
        previews={cardGen.previews}
        deckSuggestion={cardGen.deckSuggestion}
        approved={cardGen.approved}
        error={cardGen.error}
        saving={cardGen.saving}
        onToggleCard={cardGen.toggleCard}
        onEditCard={cardGen.editCard}
        onSave={(noteId, deck) => {
          cardGen.saveApproved(noteId, deck).then(() => setCardGenOpen(false));
        }}
        onClose={() => {
          cardGen.reset();
          setCardGenOpen(false);
        }}
        noteId={selectedNote?.id ?? null}
      />
    </div>
  );
}
