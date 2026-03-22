import { ChevronDown, ChevronRight, GripHorizontal, StickyNote, X } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import type { AnnotationResponse } from "../hooks/useAnnotations";
import { AnnotationSidebar } from "./editor/AnnotationSidebar";
import { EditorContentWrapper, useNoteEditor } from "./editor/EditorCore";

interface AnnotationPaneProps {
  initialSideNotes?: string;
  annotations: AnnotationResponse[];
  updateAnnotation: (params: { id: string; content?: string; tags?: string }) => Promise<void>;
  deleteAnnotation: (id: string) => Promise<void>;
  onClose: () => void;
  onSideNotesChange?: (html: string, markdown: string) => void;
  sourceLang?: string;
  targetLang?: string;
}

export function AnnotationPane({
  initialSideNotes = "",
  annotations,
  updateAnnotation,
  deleteAnnotation,
  onClose,
  onSideNotesChange,
  sourceLang,
  targetLang,
}: AnnotationPaneProps) {
  const [annotationsExpanded, setAnnotationsExpanded] = useState(true);
  const [annotationSplit, setAnnotationSplit] = useState(0.45); // annotations take 45%
  const containerRef = useRef<HTMLDivElement>(null);
  const resizeCleanupRef = useRef<(() => void) | null>(null);
  const saveTimerRef = useRef<ReturnType<typeof setTimeout>>(undefined);
  const latestContentRef = useRef<{ html: string; markdown: string } | null>(null);

  const handleUpdate = useCallback(
    (html: string, markdown: string) => {
      latestContentRef.current = { html, markdown };
      clearTimeout(saveTimerRef.current);
      saveTimerRef.current = setTimeout(() => {
        onSideNotesChange?.(html, markdown);
        latestContentRef.current = null;
      }, 800);
    },
    [onSideNotesChange],
  );

  // Flush pending save + cleanup resize listeners on unmount
  useEffect(() => {
    return () => {
      clearTimeout(saveTimerRef.current);
      resizeCleanupRef.current?.();
      if (latestContentRef.current) {
        onSideNotesChange?.(latestContentRef.current.html, latestContentRef.current.markdown);
      }
    };
  }, [onSideNotesChange]);

  const sideNotesEditor = useNoteEditor({
    content: initialSideNotes,
    onUpdate: handleUpdate,
  });

  return (
    <div ref={containerRef} className="flex flex-col h-full">
      {/* Header */}
      <div className="flex items-center justify-between px-4 py-2 border-b border-border shrink-0">
        <div className="flex items-center gap-2">
          <StickyNote size={14} className="text-purple" strokeWidth={1.5} />
          <span className="text-sm font-medium text-primary">Notes & Annotations</span>
        </div>
        <button
          type="button"
          onClick={onClose}
          className="p-1 rounded text-muted-foreground hover:text-primary transition-colors"
        >
          <X size={14} strokeWidth={1.5} />
        </button>
      </div>

      {/* Rich-text notes editor */}
      <div
        className="overflow-y-auto min-h-[60px]"
        style={{ flex: `0 0 ${(1 - annotationSplit) * 100}%` }}
      >
        <EditorContentWrapper editor={sideNotesEditor} className="min-h-[60px] px-1 py-1 text-sm" />
      </div>

      {/* Resize handle */}
      {annotationsExpanded && (
        // biome-ignore lint/a11y/noStaticElementInteractions: resize handle
        <div
          className="h-1.5 shrink-0 cursor-row-resize group flex items-center justify-center hover:bg-purple/10 transition-colors border-t border-border"
          onPointerDown={(e) => {
            e.preventDefault();
            const container = containerRef.current;
            if (!container) return;
            const rect = container.getBoundingClientRect();
            const onMove = (ev: PointerEvent) => {
              const ratio = (ev.clientY - rect.top) / rect.height;
              const annRatio = 1 - Math.max(0.15, Math.min(0.85, ratio));
              setAnnotationSplit(annRatio);
            };
            const onUp = () => {
              resizeCleanupRef.current = null;
              document.removeEventListener("pointermove", onMove);
              document.removeEventListener("pointerup", onUp);
            };
            document.addEventListener("pointermove", onMove);
            document.addEventListener("pointerup", onUp);
            resizeCleanupRef.current = onUp;
          }}
          onKeyDown={() => {}}
        >
          <GripHorizontal
            size={10}
            className="text-muted-foreground/30 group-hover:text-muted-foreground/60"
          />
        </div>
      )}

      {/* Annotations — collapsible, resizable */}
      <div
        className="flex flex-col min-h-0"
        style={annotationsExpanded ? { flex: `0 0 ${annotationSplit * 100}%` } : undefined}
      >
        <button
          type="button"
          onClick={() => setAnnotationsExpanded((prev) => !prev)}
          className="w-full flex items-center gap-1.5 px-3 py-1.5 text-[10px] text-muted-foreground uppercase tracking-wider hover:bg-surface-hover transition-colors shrink-0"
        >
          {annotationsExpanded ? (
            <ChevronDown size={10} strokeWidth={1.5} />
          ) : (
            <ChevronRight size={10} strokeWidth={1.5} />
          )}
          <span>Annotations ({annotations.length})</span>
        </button>

        {annotationsExpanded && (
          <div className="flex-1 overflow-y-auto min-h-0">
            <AnnotationSidebar
              annotations={annotations}
              updateAnnotation={updateAnnotation}
              deleteAnnotation={deleteAnnotation}
              activeAnnotationId={null}
              onAnnotationClick={() => {}}
              sourceLang={sourceLang}
              targetLang={targetLang}
              hideHeader
            />
          </div>
        )}
      </div>
    </div>
  );
}
