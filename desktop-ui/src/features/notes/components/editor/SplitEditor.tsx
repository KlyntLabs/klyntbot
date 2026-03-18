import type { Note, NoteUpdateParams } from "@shared/types";
import { useCallback, useEffect, useRef, useState } from "react";
import { EditorContentWrapper, useNoteEditor } from "./EditorCore";

export type SplitMode = "translation" | "annotation" | "cornell";

interface PaneContent {
  html: string;
  markdown: string;
}

/** Per-mode storage: left pane shared, each mode has its own right pane content */
interface SplitContentStore {
  left: PaneContent;
  translation: PaneContent;
  annotation: PaneContent;
  cornell: PaneContent;
  summary: PaneContent;
}

interface SplitEditorProps {
  note: Note;
  splitMode: SplitMode;
  onSave: (params: NoteUpdateParams) => void;
}

const EMPTY: PaneContent = { html: "", markdown: "" };

function parseSplitStore(note: Note): SplitContentStore {
  const defaultLeft: PaneContent = {
    html: note.bodyHtml || note.body || "",
    markdown: note.body || "",
  };

  if (note.splitContent) {
    try {
      const raw = JSON.parse(note.splitContent);
      // Support legacy format that used a single "right" key
      const legacy = raw.right || EMPTY;
      return {
        left: raw.left || defaultLeft,
        translation: raw.translation || legacy,
        annotation: raw.annotation || EMPTY,
        cornell: raw.cornell || EMPTY,
        summary: raw.summary || EMPTY,
      };
    } catch {
      /* fall through */
    }
  }

  return {
    left: defaultLeft,
    translation: EMPTY,
    annotation: EMPTY,
    cornell: EMPTY,
    summary: EMPTY,
  };
}

// Stable no-op callbacks — avoids TipTap editor recreation on every render
const NOOP_NAV_NOTE = () => {};
const NOOP_NAV_ENTITY = () => {};

export function SplitEditor({ note, splitMode, onSave }: SplitEditorProps) {
  const onSaveRef = useRef(onSave);
  onSaveRef.current = onSave;
  const noteIdRef = useRef(note.id);
  const splitModeRef = useRef(splitMode);

  // Per-mode content store
  const storeRef = useRef(parseSplitStore(note));

  // Debounced save
  const saveTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const pendingLeftRef = useRef<PaneContent | null>(null);
  const pendingRightRef = useRef<PaneContent | null>(null);
  const pendingSummaryRef = useRef<PaneContent | null>(null);

  // Current content refs (latest values for the active panes)
  const leftContentRef = useRef(storeRef.current.left);
  const rightContentRef = useRef(storeRef.current[splitMode]);
  const summaryContentRef = useRef(storeRef.current.summary);

  const flushSave = useCallback(() => {
    if (saveTimerRef.current) {
      clearTimeout(saveTimerRef.current);
      saveTimerRef.current = null;
    }

    const hasLeft = pendingLeftRef.current !== null;
    const hasRight = pendingRightRef.current !== null;
    const hasSummary = pendingSummaryRef.current !== null;

    if (!hasLeft && !hasRight && !hasSummary) return;

    // Apply pending changes
    if (pendingLeftRef.current) {
      leftContentRef.current = pendingLeftRef.current;
      storeRef.current.left = pendingLeftRef.current;
      pendingLeftRef.current = null;
    }
    if (pendingRightRef.current) {
      rightContentRef.current = pendingRightRef.current;
      storeRef.current[splitModeRef.current] = pendingRightRef.current;
      pendingRightRef.current = null;
    }
    if (pendingSummaryRef.current) {
      summaryContentRef.current = pendingSummaryRef.current;
      storeRef.current.summary = pendingSummaryRef.current;
      pendingSummaryRef.current = null;
    }

    // Serialize the full store
    const store = storeRef.current;
    const splitContent: Record<string, PaneContent> = { left: store.left };
    if (store.translation.markdown) splitContent.translation = store.translation;
    if (store.annotation.markdown) splitContent.annotation = store.annotation;
    if (store.cornell.markdown) splitContent.cornell = store.cornell;
    if (store.summary.markdown) splitContent.summary = store.summary;

    // Concatenate all non-empty panes for FTS5/BookRAG body
    const allParts = [
      store.left.markdown,
      store.translation.markdown,
      store.annotation.markdown,
      store.cornell.markdown,
      store.summary.markdown,
    ].filter(Boolean);
    const body = allParts.join("\n\n---\n\n");

    const allHtml = [
      store.left.html,
      store.translation.html,
      store.annotation.html,
      store.cornell.html,
      store.summary.html,
    ].filter(Boolean);
    const bodyHtml = allHtml.join('<hr class="split-divider">');

    onSaveRef.current({
      id: noteIdRef.current,
      body,
      bodyHtml,
      splitContent: JSON.stringify(splitContent),
    });
  }, []);

  const scheduleSave = useCallback(() => {
    if (saveTimerRef.current) clearTimeout(saveTimerRef.current);
    saveTimerRef.current = setTimeout(flushSave, 1000);
  }, [flushSave]);

  // Editor update handlers
  const handleLeftUpdate = useCallback(
    (html: string, markdown: string) => {
      pendingLeftRef.current = { html, markdown };
      scheduleSave();
    },
    [scheduleSave],
  );

  const handleRightUpdate = useCallback(
    (html: string, markdown: string) => {
      pendingRightRef.current = { html, markdown };
      scheduleSave();
    },
    [scheduleSave],
  );

  // Create editors
  const leftEditor = useNoteEditor({
    content: storeRef.current.left.html || storeRef.current.left.markdown,
    onUpdate: handleLeftUpdate,
    onNavigateNote: NOOP_NAV_NOTE,
    onNavigateEntity: NOOP_NAV_ENTITY,
  });

  const rightEditor = useNoteEditor({
    content: storeRef.current[splitMode].html || storeRef.current[splitMode].markdown,
    onUpdate: handleRightUpdate,
    onNavigateNote: NOOP_NAV_NOTE,
    onNavigateEntity: NOOP_NAV_ENTITY,
  });

  // Handle mode switch: flush, save current right content, load new mode's content
  useEffect(() => {
    if (splitModeRef.current !== splitMode && rightEditor) {
      // Flush any pending changes for the old mode
      flushSave();

      // Update the mode ref
      splitModeRef.current = splitMode;

      // Load the new mode's right-pane content
      const newRight = storeRef.current[splitMode];
      rightContentRef.current = newRight;
      rightEditor.commands.setContent(newRight.html || newRight.markdown || "");
    }
  }, [splitMode, rightEditor, flushSave]);

  // Handle note switch
  useEffect(() => {
    if (noteIdRef.current !== note.id) {
      flushSave();
      noteIdRef.current = note.id;
      const newStore = parseSplitStore(note);
      storeRef.current = newStore;
      leftContentRef.current = newStore.left;
      rightContentRef.current = newStore[splitMode];
      summaryContentRef.current = newStore.summary;
      setSummaryText(newStore.summary.markdown);
      if (leftEditor) leftEditor.commands.setContent(newStore.left.html || newStore.left.markdown);
      if (rightEditor)
        rightEditor.commands.setContent(
          newStore[splitMode].html || newStore[splitMode].markdown || "",
        );
    }
  }, [note.id, splitMode, leftEditor, rightEditor, flushSave]);

  // Cmd+S + flush on unmount
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === "s") {
        e.preventDefault();
        flushSave();
      }
    };
    document.addEventListener("keydown", handler);
    return () => {
      document.removeEventListener("keydown", handler);
      flushSave();
    };
  }, [flushSave]);

  // ── Resize handle ─────────────────────────────────────
  const containerRef = useRef<HTMLDivElement>(null);
  const defaultRatio = splitMode === "annotation" ? 0.67 : 0.5;
  const [splitRatio, setSplitRatio] = useState(defaultRatio);
  const splitRatioRef = useRef(defaultRatio);

  useEffect(() => {
    const newDefault = splitMode === "annotation" ? 0.67 : 0.5;
    if (newDefault !== splitRatioRef.current) {
      splitRatioRef.current = newDefault;
      setSplitRatio(newDefault);
    }
  }, [splitMode]);

  const onResizeStart = useCallback((e: React.PointerEvent) => {
    e.preventDefault();
    const startX = e.clientX;
    const startRatio = splitRatioRef.current;
    const containerW = containerRef.current?.offsetWidth || 800;
    let raf = 0;

    containerRef.current?.classList.add("resizing");

    const onMove = (ev: globalThis.PointerEvent) => {
      cancelAnimationFrame(raf);
      raf = requestAnimationFrame(() => {
        const delta = ev.clientX - startX;
        const newRatio = Math.min(0.8, Math.max(0.2, startRatio + delta / containerW));
        splitRatioRef.current = newRatio;
        if (containerRef.current) {
          const left = containerRef.current.querySelector("[data-pane='left']") as HTMLElement;
          const right = containerRef.current.querySelector("[data-pane='right']") as HTMLElement;
          if (left) left.style.width = `${newRatio * 100}%`;
          if (right) right.style.width = `${(1 - newRatio) * 100}%`;
        }
      });
    };

    const onUp = () => {
      cancelAnimationFrame(raf);
      setSplitRatio(splitRatioRef.current);
      containerRef.current?.classList.remove("resizing");
      document.removeEventListener("pointermove", onMove);
      document.removeEventListener("pointerup", onUp);
    };

    document.addEventListener("pointermove", onMove);
    document.addEventListener("pointerup", onUp);
  }, []);

  // ── Synced scrolling (Translation mode only) ──────────
  const leftPaneRef = useRef<HTMLDivElement>(null);
  const rightPaneRef = useRef<HTMLDivElement>(null);
  const isSyncingRef = useRef(false);

  const handleSyncScroll = useCallback(
    (source: "left" | "right") => {
      if (splitMode !== "translation") return;
      if (isSyncingRef.current) return;
      isSyncingRef.current = true;

      const sourceEl = source === "left" ? leftPaneRef.current : rightPaneRef.current;
      const targetEl = source === "left" ? rightPaneRef.current : leftPaneRef.current;

      if (sourceEl && targetEl) {
        const ratio = sourceEl.scrollTop / (sourceEl.scrollHeight - sourceEl.clientHeight || 1);
        targetEl.scrollTop = ratio * (targetEl.scrollHeight - targetEl.clientHeight);
      }

      requestAnimationFrame(() => {
        isSyncingRef.current = false;
      });
    },
    [splitMode],
  );

  // ── Cornell summary ───────────────────────────────────
  const [summaryText, setSummaryText] = useState(storeRef.current.summary.markdown);

  const handleSummaryChange = useCallback(
    (text: string) => {
      setSummaryText(text);
      pendingSummaryRef.current = { html: `<p>${text}</p>`, markdown: text };
      scheduleSave();
    },
    [scheduleSave],
  );

  // Load summary when switching to Cornell
  useEffect(() => {
    if (splitMode === "cornell") {
      setSummaryText(storeRef.current.summary.markdown);
    }
  }, [splitMode]);

  if (!leftEditor || !rightEditor) return null;

  // ── Mode labels ───────────────────────────────────────
  const leftLabel =
    splitMode === "translation"
      ? "Source"
      : splitMode === "cornell"
        ? "Cues / Questions"
        : "Content";
  const rightLabel =
    splitMode === "translation" ? "Translation" : splitMode === "cornell" ? "Notes" : "Annotations";

  return (
    <div ref={containerRef} className="flex-1 flex flex-col min-h-0">
      <div className="flex-1 flex min-h-0">
        {/* Left pane */}
        <div
          data-pane="left"
          ref={leftPaneRef}
          className="flex flex-col min-h-0 overflow-y-auto"
          style={{ width: `${splitRatio * 100}%` }}
          onScroll={() => handleSyncScroll("left")}
        >
          <div className="px-3 py-1.5 text-[10px] text-muted-foreground uppercase tracking-wider border-b border-border shrink-0">
            {leftLabel}
          </div>
          <EditorContentWrapper editor={leftEditor} className="flex-1 min-h-0" />
        </div>

        {/* Resize handle */}
        <div
          className="w-1 cursor-col-resize bg-border hover:bg-brand/30 transition-colors shrink-0"
          onPointerDown={onResizeStart}
        />

        {/* Right pane */}
        <div
          data-pane="right"
          ref={rightPaneRef}
          className="flex flex-col min-h-0 overflow-y-auto"
          style={{ width: `${(1 - splitRatio) * 100}%` }}
          onScroll={() => handleSyncScroll("right")}
        >
          <div className="px-3 py-1.5 text-[10px] text-muted-foreground uppercase tracking-wider border-b border-border shrink-0">
            {rightLabel}
          </div>
          <EditorContentWrapper editor={rightEditor} className="flex-1 min-h-0" />
        </div>
      </div>

      {/* Cornell summary footer */}
      {splitMode === "cornell" && (
        <div className="border-t border-border">
          <div className="px-3 py-1.5 text-[10px] text-muted-foreground uppercase tracking-wider">
            Summary
          </div>
          <textarea
            value={summaryText}
            onChange={(e) => handleSummaryChange(e.target.value)}
            placeholder="Write a brief summary of this note..."
            className="w-full bg-transparent px-3 py-2 text-sm text-foreground placeholder:text-dim resize-none"
            rows={3}
          />
        </div>
      )}
    </div>
  );
}
