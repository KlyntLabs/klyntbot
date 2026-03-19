import { ipc } from "@shared/hooks/useIpc";
import { invalidateQueries } from "@shared/hooks/useQuery";
import { useCallback, useEffect, useRef, useState } from "react";
import { type AnnotationResponse, useAnnotations } from "../../hooks/useAnnotations";

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
  noteId: string;
  /** The mark ID of the annotation currently selected in the editor */
  activeAnnotationId: string | null;
  onAnnotationClick: (markId: string) => void;
  sourceLang?: string;
  targetLang?: string;
}

export function AnnotationSidebar({
  noteId,
  activeAnnotationId,
  onAnnotationClick,
  sourceLang = "zh",
  targetLang = "en",
}: AnnotationSidebarProps) {
  const { annotations, updateAnnotation, deleteAnnotation } = useAnnotations(noteId, null);

  return (
    <div className="flex h-full flex-col">
      <div className="px-3 py-1.5 text-[10px] text-muted uppercase tracking-wider border-b border-border shrink-0 flex items-center justify-between">
        <span>Annotations ({annotations.length})</span>
      </div>

      <div className="flex-1 overflow-y-auto">
        {annotations.length === 0 ? (
          <div className="flex items-center justify-center h-32 text-xs text-muted">
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
  const [comment, setComment] = useState(annotation.content || "");
  const [editing, setEditing] = useState(false);
  const cardRef = useRef<HTMLDivElement>(null);
  const [enrichment, setEnrichment] = useState<AnnotationEnrichment | null>(null);
  const enrichedRef = useRef(false);

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
      .catch(() => {
        // Enrichment is non-critical; ignore failures
      });
  }, [annotation.id, annotation.quotedText]);

  const handleSaveComment = useCallback(async () => {
    await onUpdate({ id: annotation.id, content: comment });
    setEditing(false);
  }, [annotation.id, comment, onUpdate]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
        handleSaveComment();
      }
      if (e.key === "Escape") {
        setComment(annotation.content || "");
        setEditing(false);
      }
    },
    [handleSaveComment, annotation.content],
  );

  return (
    <div
      ref={cardRef}
      onClick={onClick}
      className={`border-b border-border px-3 py-3 cursor-pointer transition-colors ${
        isActive ? "bg-brand/10 border-l-2 border-l-brand" : "hover:bg-surface-hover"
      }`}
    >
      {/* Quoted text */}
      {annotation.quotedText && (
        <div className="mb-2 border-l-2 border-brand/40 pl-2">
          <p className="text-[11px] text-muted italic leading-relaxed">
            &ldquo;{annotation.quotedText}&rdquo;
          </p>
        </div>
      )}

      {/* Language enrichment (Smart Detection) */}
      {enrichment && (
        <div className="mb-2 space-y-1.5">
          <div className="rounded-md bg-surface-hover/50 px-2 py-1.5 text-[11px] text-muted">
            {enrichment.translation}
          </div>
          <div className="flex flex-wrap gap-1">
            {enrichment.words.map((w) => (
              <span
                key={w.word}
                className="inline-flex items-center gap-1 rounded bg-surface-hover px-1.5 py-0.5 text-[10px] text-primary"
              >
                {w.word}
                {w.reading && <span className="text-[9px] text-muted">{w.reading}</span>}
                {w.proficiencyLevel && (
                  <span className="text-[9px] text-purple-400">{w.proficiencyLevel}</span>
                )}
              </span>
            ))}
          </div>
        </div>
      )}

      {/* Comment / note area */}
      {editing ? (
        <div className="mt-1">
          <textarea
            value={comment}
            onChange={(e) => setComment(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder="Add a note..."
            className="w-full rounded-md bg-surface-base p-2 text-xs text-primary outline-none ring-1 ring-border focus:ring-brand resize-none"
            rows={3}
            autoFocus
          />
          <div className="mt-1 flex items-center justify-between">
            <span className="text-[9px] text-muted">⌘+Enter to save</span>
            <div className="flex gap-1">
              <button
                type="button"
                onClick={(e) => {
                  e.stopPropagation();
                  setComment(annotation.content || "");
                  setEditing(false);
                }}
                className="rounded px-2 py-0.5 text-[10px] text-muted hover:bg-surface-hover"
              >
                Cancel
              </button>
              <button
                type="button"
                onClick={(e) => {
                  e.stopPropagation();
                  handleSaveComment();
                }}
                className="rounded bg-brand/20 px-2 py-0.5 text-[10px] text-brand hover:bg-brand/30"
              >
                Save
              </button>
            </div>
          </div>
        </div>
      ) : annotation.content ? (
        <p
          className="text-xs text-primary leading-relaxed cursor-text"
          onClick={(e) => {
            e.stopPropagation();
            setEditing(true);
          }}
        >
          {annotation.content}
        </p>
      ) : (
        <button
          type="button"
          className="mt-1 text-[11px] text-muted hover:text-primary transition-colors"
          onClick={(e) => {
            e.stopPropagation();
            setEditing(true);
          }}
        >
          + Add a note...
        </button>
      )}

      {/* Footer: date + delete */}
      <div className="mt-2 flex items-center justify-between">
        <span className="text-[9px] text-muted">
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
          className="text-[9px] text-muted hover:text-red-400 transition-colors"
        >
          Delete
        </button>
      </div>
    </div>
  );
}
