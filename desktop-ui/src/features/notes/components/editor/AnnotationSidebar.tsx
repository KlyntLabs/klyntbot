import { ipc } from "@shared/hooks/useIpc";
import { useEffect, useRef, useState } from "react";
import type { AnnotationResponse } from "../../hooks/useAnnotations";
import { EditorContentWrapper, useNoteEditor } from "./EditorCore";

interface AnnotationEnrichment {
  translation: string;
  words: Array<{
    word: string;
    reading: string | null;
    meaning: string;
    proficiencyLevel: string | null;
  }>;
}

/** Detect if text contains non-ASCII/non-Latin characters (likely foreign language). */
function hasForeignText(text: string): boolean {
  const cjkPattern = /[\u2E80-\u9FFF\uF900-\uFAFF\u3040-\u309F\u30A0-\u30FF]/;
  return cjkPattern.test(text);
}

interface AnnotationSidebarProps {
  annotations: AnnotationResponse[];
  updateAnnotation: (params: { id: string; content?: string; tags?: string }) => Promise<void>;
  deleteAnnotation: (id: string) => Promise<void>;
  /** The mark ID of the annotation currently selected in the editor */
  activeAnnotationId: string | null;
  onAnnotationClick: (markId: string) => void;
  sourceLang?: string;
  targetLang?: string;
  hideHeader?: boolean;
}

export function AnnotationSidebar({
  annotations,
  updateAnnotation,
  deleteAnnotation,
  activeAnnotationId,
  onAnnotationClick,
  sourceLang = "zh",
  targetLang = "en",
  hideHeader,
}: AnnotationSidebarProps) {
  return (
    <div className="flex h-full flex-col">
      {!hideHeader && (
        <div className="px-3 py-1.5 text-2xs text-muted-foreground uppercase tracking-wider border-b border-border shrink-0 flex items-center justify-between">
          <span>Annotations ({annotations.length})</span>
        </div>
      )}

      <div className="flex-1 overflow-y-auto">
        {annotations.length === 0 ? (
          <div className="flex items-center justify-center h-32 text-xs text-muted-foreground">
            Select text in the editor and right-click to annotate.
          </div>
        ) : (
          <div className="flex flex-col">
            {annotations.map((ann) => (
              <AnnotationCard
                key={ann.id}
                annotation={ann}
                isActive={ann.markId === activeAnnotationId || ann.id === activeAnnotationId}
                onClick={() => onAnnotationClick(ann.markId ?? ann.id)}
                onUpdate={updateAnnotation}
                onDelete={deleteAnnotation}
                sourceLang={sourceLang}
                targetLang={targetLang}
              />
            ))}
          </div>
        )}
      </div>
    </div>
  );
}

function AnnotationCard({
  annotation,
  isActive,
  onClick,
  onUpdate,
  onDelete,
  sourceLang,
  targetLang,
}: {
  annotation: AnnotationResponse;
  isActive: boolean;
  onClick: () => void;
  onUpdate: (params: { id: string; content?: string; tags?: string }) => Promise<void>;
  onDelete: (id: string) => Promise<void>;
  sourceLang: string;
  targetLang: string;
}) {
  const cardRef = useRef<HTMLDivElement>(null);
  const [enrichment, setEnrichment] = useState<AnnotationEnrichment | null>(null);
  const enrichedRef = useRef(false);
  const onUpdateRef = useRef(onUpdate);
  onUpdateRef.current = onUpdate;

  // Scroll into view when this card becomes active
  useEffect(() => {
    if (isActive && cardRef.current) {
      cardRef.current.scrollIntoView({ behavior: "smooth", block: "nearest" });
    }
  }, [isActive]);

  // Smart language enrichment: auto-enrich if quoted text has foreign characters
  useEffect(() => {
    if (enrichedRef.current || !annotation.quotedText) return;
    if (!hasForeignText(annotation.quotedText)) return;
    enrichedRef.current = true;

    ipc<AnnotationEnrichment>("language_enrich_annotation", {
      params: {
        annotationId: annotation.id,
        quotedText: annotation.quotedText,
        sourceLang,
        targetLang,
      },
    })
      .then(setEnrichment)
      .catch(() => {});
  }, [annotation.id, annotation.quotedText, sourceLang, targetLang]);

  return (
    // biome-ignore lint/a11y/useKeyWithClickEvents: annotation card click navigates to mark
    // biome-ignore lint/a11y/noStaticElementInteractions: clickable annotation card
    <div
      ref={cardRef}
      onClick={onClick}
      className={`border-b border-border px-2.5 py-2 cursor-pointer transition-colors ${
        isActive ? "bg-brand/10 border-l-2 border-l-brand" : "hover:bg-surface-hover"
      }`}
    >
      {/* Quoted text */}
      {annotation.quotedText && (
        <div className="mb-1 border-l-2 border-brand/40 pl-1.5">
          <p className="text-2xs text-muted-foreground italic leading-snug">
            &ldquo;{annotation.quotedText}&rdquo;
          </p>
        </div>
      )}

      {/* Language enrichment (Smart Detection) */}
      {enrichment && (
        <div className="mb-2 space-y-1.5">
          <div className="rounded-md bg-surface-hover/50 px-2 py-1.5 text-[11px] text-muted-foreground">
            {enrichment.translation}
          </div>
          <div className="flex flex-wrap gap-1">
            {enrichment.words.map((w) => (
              <span
                key={w.word}
                className="inline-flex items-center gap-1 rounded bg-surface-hover px-1.5 py-0.5 text-2xs text-primary"
              >
                {w.word}
                {w.reading && <span className="text-[9px] text-muted-foreground">{w.reading}</span>}
                {w.proficiencyLevel && (
                  <span className="text-[9px] text-purple-400">{w.proficiencyLevel}</span>
                )}
              </span>
            ))}
          </div>
        </div>
      )}

      {/* Inline note editor — always live, auto-saves */}
      <AnnotationInlineEditor
        annotationId={annotation.id}
        initialContent={annotation.content || ""}
        onUpdateRef={onUpdateRef}
      />

      {/* Footer: date + delete */}
      <div className="mt-1 flex items-center justify-between">
        <span className="text-[9px] text-muted-foreground">
          {new Date(annotation.createdAt).toLocaleString(undefined, {
            month: "short",
            day: "numeric",
            hour: "numeric",
            minute: "2-digit",
          })}
        </span>
        <button
          type="button"
          onClick={(e) => {
            e.stopPropagation();
            onDelete(annotation.id);
          }}
          className="text-[9px] text-muted-foreground hover:text-red-400 transition-colors"
        >
          Delete
        </button>
      </div>
    </div>
  );
}

function AnnotationInlineEditor({
  annotationId,
  initialContent,
  onUpdateRef,
}: {
  annotationId: string;
  initialContent: string;
  onUpdateRef: React.RefObject<
    (params: { id: string; content?: string; tags?: string }) => Promise<void>
  >;
}) {
  const saveTimerRef = useRef<ReturnType<typeof setTimeout>>(undefined);
  const dirtyRef = useRef(false);

  const editor = useNoteEditor({
    content: initialContent,
    editorClass: "editor-content-compact",
    onUpdate: (html) => {
      dirtyRef.current = true;
      clearTimeout(saveTimerRef.current);
      saveTimerRef.current = setTimeout(() => {
        dirtyRef.current = false;
        onUpdateRef.current({ id: annotationId, content: html });
      }, 600);
    },
  });

  // Flush only if dirty on unmount
  // biome-ignore lint/correctness/useExhaustiveDependencies: onUpdateRef is a stable ref — .current is accessed at cleanup time, not render time
  useEffect(() => {
    return () => {
      clearTimeout(saveTimerRef.current);
      if (dirtyRef.current && editor) {
        onUpdateRef.current({ id: annotationId, content: editor.getHTML() });
      }
    };
  }, [editor, annotationId]);

  return <EditorContentWrapper editor={editor} className="editor-content-compact min-h-[20px]" />;
}
